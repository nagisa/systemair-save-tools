use crate::modbus::{self, ModbusTCPCodec, Request, Response};
use futures::{SinkExt, StreamExt as _};
use std::collections::{BTreeMap, VecDeque};
use std::ops::Range;
use std::path::PathBuf;
use std::pin;
use std::sync::atomic::AtomicU16;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::error::SendError;
use tokio::time::Instant;
use tokio_util::codec::Framed;
use tracing::{debug, info, trace, warn};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("lookup of `{1}` failed")]
    LookupHost(#[source] std::io::Error, String),
    #[error("could not connect to `{1}` over TCP")]
    Connect(#[source] std::io::Error, String),
    #[error("could not open {1:?} for reading and writing")]
    OpenDevice(#[source] std::io::Error, PathBuf),
    #[error("scheduling a request failed")]
    ScheduleRequest(#[source] SendError<modbus::Request>),
    #[error("could not read data from the stream")]
    Receive(#[source] std::io::Error),
    #[error("could not shut down the connection")]
    Shutdown(#[source] std::io::Error),
    #[error("could not send out the request")]
    Send(#[source] std::io::Error),
    #[error("could not flush out the request")]
    Flush(#[source] std::io::Error),
    #[error("could not construct the HTTP client")]
    CreateReqwest(#[source] reqwest::Error),
    #[error("modbus read API request failed")]
    Iam2Read(#[source] reqwest::Error),
    #[error("IAM2 response returned malformed JSON response")]
    Iam2JsonDecode(#[source] reqwest::Error),
    #[error("IAM2 response is not an object")]
    Iam2ResponseIsntObject,
    #[error("IAM2 response does not contain values for all the registers (requested {0}, got {1})")]
    Iam2ResponseIncomplete(u16, usize),
    #[error("IAM2 response is weird (found key {0}, but requested {1:?})")]
    Iam2ResponseWeirdKeys(u16, Range<u16>),
    #[error("IAM2 response contains register value that isn't numeric")]
    Iam2ValueIsntNumber,
    #[error("modbus write API request failed")]
    Iam2Write(#[source] reqwest::Error),
}

#[derive(Default)]
pub struct ResponseTracker {
    responses: Mutex<BTreeMap<u16, Option<modbus::Response>>>,
    change_notify: Notify,
}

impl ResponseTracker {
    pub fn mark_timeout(&self, transaction_id: u16) {
        let mut guard = self.responses.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(transaction_id, None);
        self.change_notify.notify_waiters();
        drop(guard);
    }

    pub fn add_response(&self, response: modbus::Response) {
        let mut guard = self.responses.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(response.transaction_id, Some(response));
        self.change_notify.notify_waiters();
        drop(guard);
    }

    pub async fn wait_for(&self, transaction_id: u16) -> Option<modbus::Response> {
        loop {
            self.change_notify.notified().await;
            let mut guard = self.responses.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(v) = guard.remove(&transaction_id) {
                return v;
            }
        }
    }
}

#[derive(clap::Parser, Clone)]
#[group(id = "connection::Args")]
pub struct Args {
    #[clap(flatten)]
    how: ConnectionGroup,

    /// The modbus device ID.
    #[arg(long, short = 'i')]
    device_id: u8,

    /// If the modbus response isn't received in this amount of time plus the expected internal
    /// read-out time, consider the request failed.
    ///
    /// Most of the commands will at that point attempt to retry the request.
    #[arg(long, default_value = "1s")]
    read_timeout: humantime::Duration,

    /// Reconnect, if the modbus request can't be sent in this amount of time.
    #[arg(long, default_value = "3s")]
    send_timeout: humantime::Duration,

    /// Reconnect to the modbus server after the specified number of reads timeout.
    #[arg(long, default_value = "3")]
    reconnect_after_timeouts: usize,

    /// The baudrate configured for the SAVE device which this tool will use to pace requests.
    ///
    /// See `registers modbus`.
    #[arg(long, default_value = "9600")]
    baudrate: u32,

    /// The amount of additional time to wait between sending requests over TCP.
    ///
    /// Interacting too fast can make some Modbus TCP interfaces behave poorly.
    #[arg(long, default_value = "100ms")]
    tcp_send_delay: humantime::Duration,

    /// The amount of additional time to wait after receiving a server busy exception.
    ///
    /// When busy, modbus proxies can respond with an exception code 6. Give the device
    /// this amount of time to finish its current work before retrying.
    #[arg(long, default_value = "25ms")]
    server_busy_retry_delay: humantime::Duration,
}

#[derive(clap::Parser, Clone)]
#[group(required = true)]
pub struct ConnectionGroup {
    /// Connect to the SystemAIR device over Modbus TCP (e.g. available via the IAM v1 module).
    #[arg(long)]
    tcp: Option<String>,
    /// Connect to the SystemAIR device over IAM v2 HTTP API.
    #[arg(long)]
    iam2: Option<reqwest::Url>,
    /// Connect to the SystemAIR device over Serial Modbus RTU.
    ///
    /// Specify the path to the serial device.
    #[arg(long)]
    rtu: Option<PathBuf>,
}

pub struct Connection {
    pub request_queue: tokio::sync::mpsc::UnboundedSender<modbus::Request>,
    pub worker: tokio::task::JoinHandle<Result<(), Error>>,
    pub response_tracker: Arc<ResponseTracker>,
    transaction_id_generator: std::sync::atomic::AtomicU16,
    args: Args,
}

impl Connection {
    pub async fn new(args: Args) -> Result<Connection, Error> {
        let (request_queue, jobs) = tokio::sync::mpsc::unbounded_channel();
        let response_tracker = Default::default();
        let worker = if args.how.tcp.is_some() {
            TcpWorker {
                reconnect_countdown: args.reconnect_after_timeouts,
                args: args.clone(),
                responses: Arc::clone(&response_tracker),
                inflight: VecDeque::with_capacity(8),
            }
            .spawn(jobs)
        } else if args.how.iam2.is_some() {
            Iam2Worker { args: args.clone(), responses: Arc::clone(&response_tracker) }.spawn(jobs)
        } else if args.how.rtu.is_some() {
            todo!("Modbus RTU over direct serial is not implemented yet");
        } else {
            panic!("both `--tcp` and `--device` are `None`?");
        };
        Ok(Self {
            request_queue,
            worker,
            response_tracker,
            transaction_id_generator: AtomicU16::new(0),
            args,
        })
    }

    // pub async fn new_rtu(device: &Path) -> Result<Connection, Error> {
    //     let device = tokio::fs::File::options()
    //         .read(true)
    //         .write(true)
    //         .create(false)
    //         .open(device)
    //         .await
    //         .map_err(|e| Error::OpenDevice(e, device.to_path_buf()))?;
    //     let (request_queue, jobs) = tokio::sync::mpsc::unbounded_channel();
    //     Ok(Self {
    //         io: Framed::new(Box::pin(device), Box::new(ModbusRTUCodec {})),
    //         request_queue,
    //         worker: tokio::task::spawn(worker(jobs)),
    //     })
    // }

    pub fn new_transaction_id(&self) -> u16 {
        self.transaction_id_generator.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn send(
        &self,
        operation: modbus::Operation,
    ) -> Result<Option<modbus::Response>, Error> {
        let transaction_id = self.new_transaction_id();
        let request = modbus::Request { device_id: self.args.device_id, transaction_id, operation };
        self.request_queue.send(request).map_err(Error::ScheduleRequest)?;
        Ok(self.response_tracker.wait_for(transaction_id).await)
    }

    /// [`Self::send`] but retries timeouts and `Server Busy` exceptions.
    pub async fn send_retrying(
        &self,
        operation: modbus::Operation,
    ) -> Result<modbus::Response, Error> {
        loop {
            let response = self.send(operation.clone()).await?;
            let Some(response) = response else {
                continue;
            };
            if response.is_server_busy() {
                self.handle_server_busy().await;
                continue;
            } else {
                break Ok(response);
            }
        }
    }

    pub async fn handle_server_busy(&self) {
        tokio::time::sleep(*self.args.server_busy_retry_delay).await;
    }
}

struct TcpWorker {
    args: Args,
    responses: Arc<ResponseTracker>,
    /// An in-order list of outstanding requests and their timeout instants.
    ///
    /// This list is expected to be sorted by the order in which the requests were sent out, but
    /// not necessarily the transaction ID or the timeout.
    ///
    /// We use this to manage timers for send slots and request timeouts. Some examples:
    ///
    /// 1. IAM can only handle requests in order. So if we have sent out requests [(1, 500ms ago),
    ///    (2, 250ms ago)], and we receive a successful response for #2, then we already know we
    ///    won't be receiving a response for #1 -- for one reason or another it got dropped.
    /// 2. At the same time the IAM is pretty slow at advancing the TCP state machine. It might have
    ///    completed the modbus communication with the device it is connected to, and has the
    ///    response ready, but is waiting for some inexplicable reason to actually send the
    ///    response. This is particularly visible when reading out single registers -- a plain
    ///    request-response flow can take hundreds of milliseconds per register. Sending out a
    ///    pipelined request at the right time, meanwhile, forces the IAM to send out a response
    ///    quickly and the time per register can drop down to as low as 10~20 milliseconds.
    /// 3. However, send the requests too early and the desired behaviour will not be achieved. The
    ///    newly sent requests either get dropped or a server exception 6 will be returned (slowly
    ///    so).
    ///
    /// Specifics of how request timeouts are computed may result in a situation where the timeout
    /// `Instant` of a request N+1 is earlier than that of a request of `N`. Regardless this logic
    /// only keeps track of the timeout of the first request sent (in a way providing a guarantee
    /// that timeouts will be reported back in the sending order.)
    ///
    /// It may seem like a fancy data structure like `BTreeMap` or `tokio_util::time::DelayQueue`
    /// would be better here, but we don't expect to have more than 2-3 concurrent in-flight
    /// requests at a time. So linear scans are plenty good.
    inflight: VecDeque<(u16, Instant)>,
    reconnect_countdown: usize,
}

type TcpIo = Framed<TcpStream, ModbusTCPCodec>;

impl TcpWorker {
    fn spawn(
        self,
        jobs: UnboundedReceiver<modbus::Request>,
    ) -> tokio::task::JoinHandle<Result<(), Error>> {
        tokio::task::spawn(self.main_loop(jobs))
    }

    async fn main_loop(
        mut self,
        mut jobs: UnboundedReceiver<modbus::Request>,
    ) -> Result<(), Error> {
        let mut pending_send: Option<modbus::Request> = None;
        'reconnect: loop {
            // If we are reconnecting and had any in-flight requests, it is only proper to report
            // them as timed out.
            for (transaction_id, _) in self.inflight.drain(..) {
                self.responses.mark_timeout(transaction_id);
            }
            if let Some(req) = pending_send.take() {
                self.responses.mark_timeout(req.transaction_id);
            }
            // FIXME: shouldn't await here, these should be part of select!
            // somehow.
            let (mut io_sink, mut io_source) = self.connect().await?.split();
            let mut send_time = pin::pin!(tokio::time::sleep_until(Instant::now()));
            let mut recv_time = pin::pin!(tokio::time::sleep_until(Instant::now()));
            loop {
                let time_to_send = send_time.is_elapsed();
                tokio::select! {
                    biased;
                    send_result = io_sink.flush(), if pending_send.is_some() => {
                        if let Err(e) = send_result {
                            warn!(
                                message="sending request failed, will reconnect",
                                error=(&e as &dyn std::error::Error)
                            );
                            continue 'reconnect;
                        }
                        let req: Request = pending_send.take().unwrap();
                        let resp_len = req.expected_response_length().into();
                        let baudrate = self.args.baudrate;
                        let response_duration = Duration::from_secs(resp_len) / (baudrate / 10);
                        let response_ready_instant = Instant::now() + response_duration;
                        let response_deadline = response_ready_instant + *self.args.read_timeout;
                        self.inflight
                            .push_back((req.transaction_id, response_deadline));
                        recv_time.as_mut().reset(self.inflight[0].1);
                        send_time.as_mut().reset(response_ready_instant + *self.args.tcp_send_delay);
                    }
                    Some(response) = io_source.next() => {
                        match response {
                            Err(e) => return Err(Error::Receive(e)),
                            Ok(response) => self.handle_response(response, send_time.as_mut()),
                        }
                    }
                    _ = &mut recv_time, if !self.inflight.is_empty() => {
                        if !self.handle_timeout(recv_time.as_mut()) {
                            continue 'reconnect;
                        }
                    }

                    // We need to have some down time between sending out subsequent modbus
                    // requests -- otherwise the IAM device gets somewhat confused and will
                    // ignore some of the requests, leading them to time out.
                    //
                    // This conditional select will make sure that we will always wait sleeping
                    // until the next available sending slot opens up.
                    _ = &mut send_time, if !time_to_send || pending_send.is_some() => {
                        if pending_send.is_some() {
                            warn!("sending a request timed out, will reconnect");
                            continue 'reconnect;
                        }
                    }
                    job = jobs.recv(), if time_to_send && pending_send.is_none() => {
                        match job {
                            None => {
                                io_sink.close().await.map_err(Error::Shutdown)?;
                                if self.inflight.is_empty() {
                                    return Ok(());
                                }
                            },
                            Some(req) => {
                                // While we're sending, use `send_time` to track send timeout.
                                send_time.as_mut().reset(Instant::now() + *self.args.send_timeout);
                                io_sink.feed(req.clone()).await.map_err(Error::Send)?;
                                assert!(pending_send.replace(req).is_none());
                            }
                        }
                    },
                }
            }
        }
    }

    async fn connect(&mut self) -> Result<TcpIo, Error> {
        let address = self.args.how.tcp.as_ref().unwrap();
        info!(message = "connecting...", address);
        let addresses = tokio::net::lookup_host(address)
            .await
            .map_err(|e| Error::LookupHost(e, address.to_string()))?
            .collect::<Vec<_>>();
        debug!(message = "resolved", ?addresses);
        let socket = TcpStream::connect(&*addresses)
            .await
            .map_err(|e| Error::Connect(e, address.to_string()))?;
        let nodelay_result = socket.set_nodelay(true);
        trace!(message = "setting nodelay", is_error = ?nodelay_result.err());
        info!(message = "connected");
        self.reconnect_countdown = self.args.reconnect_after_timeouts;
        Ok(Framed::new(socket, ModbusTCPCodec {}))
    }

    fn handle_response(
        &mut self,
        response: modbus::Response,
        send_time: pin::Pin<&mut tokio::time::Sleep>,
    ) {
        trace!(message = "decoded a response", transaction = response.transaction_id);
        let inflight_index =
            self.inflight.iter().position(|(id, _)| *id == response.transaction_id);
        let Some(inflight_index) = inflight_index else {
            debug!(
                message = "a response we were not expecting",
                transaction = response.transaction_id
            );
            return;
        };
        if response.is_server_busy() {
            // IAM can respond with the busy code on its own, and it most
            // likely means that another request is being currently processed.
            self.inflight.remove(inflight_index);
        } else {
            // Any requests sent out prior to the response we just received
            // were dropped, so lets time them out immediately.
            for (tr_id, _) in self.inflight.drain(..inflight_index) {
                self.responses.mark_timeout(tr_id);
            }
            self.inflight.pop_front();
            self.reconnect_countdown = self.args.reconnect_after_timeouts;
        };
        self.responses.add_response(response);
        if self.inflight.is_empty() {
            send_time.reset(Instant::now());
        }
    }

    fn handle_timeout(&mut self, request_timeout: pin::Pin<&mut tokio::time::Sleep>) -> bool {
        let (transaction_id, _) = self.inflight.pop_front().expect("unreachable");
        debug!(
            message = "an inflight request timed out",
            transaction_id,
            reconnect_countdown = self.reconnect_countdown
        );
        self.responses.mark_timeout(transaction_id);
        if let Some(new_count) = self.reconnect_countdown.checked_sub(1) {
            self.reconnect_countdown = new_count;
        } else {
            return false;
        };
        if let Some((_, timeout)) = self.inflight.front() {
            request_timeout.reset(*timeout);
        }
        true
    }
}

struct Iam2Worker {
    args: Args,
    responses: Arc<ResponseTracker>,
}

impl Iam2Worker {
    fn spawn(
        self,
        jobs: UnboundedReceiver<modbus::Request>,
    ) -> tokio::task::JoinHandle<Result<(), Error>> {
        tokio::task::spawn(self.main_loop(jobs))
    }

    async fn main_loop(self, mut jobs: UnboundedReceiver<modbus::Request>) -> Result<(), Error> {
        let mut url = self.args.how.iam2.clone().expect("have url when creating Iam2Worker");
        // This initial segment push
        // a) checks the usability of the url for what we're doing; and
        // b) simplifies the handling of the mread/mwrite management (we can just unconditionally
        //    pop away the last segment later)
        url.path_segments_mut().expect("TODO").push("mwrite");
        let http_client = reqwest::Client::builder()
            .read_timeout(*self.args.read_timeout)
            .timeout(self.args.read_timeout.saturating_add(*self.args.send_timeout))
            .build()
            .map_err(Error::CreateReqwest)?;
        let mut pending_reads = futures::stream::SelectAll::new();
        let mut pending_writes = futures::stream::SelectAll::new();
        loop {
            tokio::select! {
                job = jobs.recv() => {
                    let Some(req) = job else { return Ok(()) };
                    match req.operation {
                        modbus::Operation::GetHoldings { address, count } => {
                            url.path_segments_mut().expect("TODO").pop().push("mread");
                            let obj = serde_json::json!({address.to_string(): count});
                            url.set_query(Some(&serde_json::to_string(&obj).unwrap()));
                            let resp = http_client.get(url.clone()).send();
                            let expected_keys = address..address + count;
                            pending_reads.push(Box::pin(async_stream::stream! {
                                let resp = resp.await.map_err(Error::Iam2Read)?;
                                let resp = resp.json::<serde_json::Value>().await;
                                let resp = resp.map_err(Error::Iam2JsonDecode)?;
                                let obj = resp.as_object().ok_or(Error::Iam2ResponseIsntObject)?;
                                // Can we trust IAM2.0 to respond within reason?
                                let mut results = obj.iter()
                                    .filter_map(|(k, v)| {
                                        let Ok(address) = k.parse::<u16>() else {
                                            return None;
                                        };
                                        let Some(value) = v.as_i64() else {
                                            tracing::warn!(
                                                key = k,
                                                unexpected_val = ?v,
                                                "address value isn't integer"
                                            );
                                            return None;
                                        };
                                        Some((address, value as i16 as u16))
                                    }).collect::<Vec<_>>();
                                results.sort_unstable();
                                if results.len() < usize::from(count) {
                                    yield Err(Error::Iam2ResponseIncomplete(count, results.len()));
                                    return
                                }
                                let mut values = Vec::with_capacity(results.len());
                                for ((a, v), ea) in results.into_iter().zip(expected_keys.clone()) {
                                    if a != ea {
                                        yield Err(Error::Iam2ResponseWeirdKeys(a, expected_keys));
                                        return
                                    }
                                    values.extend(u16::to_be_bytes(v));
                                }
                                yield Ok(Response {
                                    device_id: req.device_id,
                                    transaction_id: req.transaction_id,
                                    kind: modbus::ResponseKind::GetHoldings { values }
                                })
                            }));
                        },
                        modbus::Operation::SetHoldings { address, values } => {
                            url.path_segments_mut().expect("TODO").pop().push("mwrite");
                            let map = (address..).zip(values.iter())
                                .map(|(a, b)| (a.to_string(), serde_json::json!(b)))
                                .collect();
                            let query = serde_json::Value::Object(map);
                            url.set_query(Some(&serde_json::to_string(&query).unwrap()));
                            let resp = http_client.get(url.clone()).send();
                            pending_writes.push(Box::pin(async_stream::stream! {
                                let resp = resp.await.map_err(Error::Iam2Read)?;
                                let resp = resp.json::<serde_json::Value>().await;
                                let resp = resp.map_err(Error::Iam2JsonDecode)?;
                                let _obj = resp.as_object().ok_or(Error::Iam2ResponseIsntObject)?;
                                // TODO: check that write succeeded?
                                yield Ok(Response {
                                    device_id: req.device_id,
                                    transaction_id: req.transaction_id,
                                    kind: modbus::ResponseKind::SetHoldings {
                                        address,
                                        words: values.len() as u16
                                    }
                                });
                            }));
                        },
                    }
                },
                Some(response) = pending_reads.next() => {
                    let response = response?;
                    self.responses.add_response(response);
                },
                Some(response) = pending_writes.next() => {
                    let response = response?;
                    self.responses.add_response(response);
                },
            }
        }
    }
}

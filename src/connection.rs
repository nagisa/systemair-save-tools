use crate::modbus::{ModbusTCPCodec, Request, Response};
use futures::{SinkExt, StreamExt as _};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Notify;
use tokio::time::Instant;
use tokio_util::codec::Framed;
use tracing::{debug, info, trace};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("lookup of `{1}` failed")]
    LookupHost(#[source] std::io::Error, String),
    #[error("could not connect to `{1}` over TCP")]
    Connect(#[source] std::io::Error, String),
    #[error("could not open {1:?} for reading and writing")]
    OpenDevice(#[source] std::io::Error, PathBuf),
    #[error("scheduling a request failed")]
    ScheduleRequest(#[source] SendError<Request>),
    #[error("could not read data from the stream")]
    Receive(#[source] std::io::Error),
    #[error("could not shut down the connection")]
    Shutdown(#[source] std::io::Error),
    #[error("could not send out the request")]
    Send(#[source] std::io::Error),
    #[error("could not flush out the request")]
    Flush(#[source] std::io::Error),
}

trait AsyncRW: AsyncRead + AsyncWrite {
    async fn shutdown_write(&mut self) -> Result<(), std::io::Error>;
}
impl AsyncRW for tokio::net::TcpStream {
    async fn shutdown_write(&mut self) -> Result<(), std::io::Error> {
        self.shutdown().await
    }
}
impl AsyncRW for tokio::fs::File {
    async fn shutdown_write(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

#[derive(Default)]
pub struct ResponseTracker {
    responses: Mutex<BTreeMap<u16, Option<Response>>>,
    change_notify: Notify,
}

impl ResponseTracker {
    pub fn mark_timeout(&self, transaction_id: u16) {
        let mut guard = self.responses.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(transaction_id, None);
        self.change_notify.notify_waiters();
        drop(guard);
    }

    pub fn add_response(&self, response: Response) {
        let mut guard = self.responses.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(response.transaction_id, Some(response));
        self.change_notify.notify_waiters();
        drop(guard);
    }

    pub async fn wait_for(&self, transaction_id: u16) -> Option<Response> {
        loop {
            self.change_notify.notified().await;
            let mut guard = self.responses.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(v) = guard.remove(&transaction_id) {
                return v;
            }
        }
    }
}

#[derive(clap::Parser)]
#[group(id = "connection::Args")]
pub struct Args {
    #[clap(flatten)]
    how: ConnectionGroup,
    /// If the response isn't received in this amount of time plus the expected internal read-out
    /// time, consider the request failed.
    ///
    /// Most of the commands will at that point attempt to retry the request.
    #[arg(long, default_value = "1s")]
    read_timeout: humantime::Duration,

    /// Reconnect to the server after the specified number of requests timeout.
    #[arg(long, default_value = "3")]
    reconnect_after_timeouts: usize,

    /// The baudrate configured for the SAVE device which this tool will use to pace requests.
    ///
    /// See `registers modbus`.
    #[arg(long, default_value = "9600")]
    baudrate: u32,

    /// The amount of additional time to wait between sending requests over TCP.
    #[arg(long, default_value = "25ms")]
    tcp_send_delay: humantime::Duration,
}

#[derive(clap::Parser)]
#[group(required = true)]
pub struct ConnectionGroup {
    /// Connect to the SystemAIR device over Modbus TCP (usually available via the IAM module).
    #[arg(long, short = 't')]
    tcp: Option<String>,
    /// Connect to the SystemAIR device over Serial Modbus RTU.
    #[arg(long, short = 'd')]
    device: Option<PathBuf>,
}

pub struct Connection {
    pub request_queue: tokio::sync::mpsc::UnboundedSender<Request>,
    pub worker: tokio::task::JoinHandle<Result<(), Error>>,
    pub response_tracker: Arc<ResponseTracker>,
}

impl Connection {
    pub async fn new(args: Args) -> Result<Connection, Error> {
        let (request_queue, jobs) = tokio::sync::mpsc::unbounded_channel();
        let response_tracker = Default::default();
        let worker = if args.how.tcp.is_some() {
            TcpWorker {
                reconnect_countdown: args.reconnect_after_timeouts,
                args,
                responses: Arc::clone(&response_tracker),
                inflight: VecDeque::with_capacity(8),
            }
            .spawn(jobs)
        } else if args.how.device.is_some() {
            todo!()
        } else {
            panic!("both `--tcp` and `--device` are `None`?");
        };
        Ok(Self {
            request_queue,
            worker,
            response_tracker,
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

    pub async fn send(&self, request: Request) -> Result<Option<Response>, Error> {
        let transaction_id = request.transaction_id;
        self.request_queue
            .send(request)
            .map_err(Error::ScheduleRequest)?;
        Ok(self.response_tracker.wait_for(transaction_id).await)
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
type PinnedSleep<'a> = pin::Pin<&'a mut tokio::time::Sleep>;

impl TcpWorker {
    fn spawn(self, jobs: UnboundedReceiver<Request>) -> tokio::task::JoinHandle<Result<(), Error>> {
        tokio::task::spawn(self.main_loop(jobs))
    }

    async fn main_loop(mut self, mut jobs: UnboundedReceiver<Request>) -> Result<(), Error> {
        'reconnect: loop {
            // If we are reconnecting and had any in-flight requests, it is only proper to report
            // them as timed out.
            for (transaction_id, _) in self.inflight.drain(..) {
                self.responses.mark_timeout(transaction_id);
            }
            let mut io = self.connect().await?;
            let mut send_time = pin::pin!(tokio::time::sleep_until(Instant::now()));
            let mut request_timeout = pin::pin!(tokio::time::sleep_until(Instant::now()));
            loop {
                let time_to_send = send_time.is_elapsed();
                tokio::select! {
                    biased;
                    Some(response) = io.next() => {
                        match response {
                            Err(e) => return Err(Error::Receive(e)),
                            Ok(response) => self.handle_response(response, send_time.as_mut()),
                        }
                    }

                    _ = &mut request_timeout, if !self.inflight.is_empty() => {
                        if !self.handle_timeout(request_timeout.as_mut()) {
                            continue 'reconnect;
                        }
                    }

                    // We need to have some down time between sending out subsequent modbus
                    // requests -- otherwise the IAM device gets somewhat confused and will
                    // ignore some of the requests, leading them to time out.
                    //
                    // This conditional select will make sure that we will always wait sleeping
                    // until the next available sending slot opens up.
                    _ = &mut send_time, if !time_to_send => {}

                    job = jobs.recv(), if time_to_send => {
                        match job {
                            None => {
                                io.get_mut().shutdown_write().await.map_err(Error::Shutdown)?;
                                if self.inflight.is_empty() {
                                    return Ok(());
                                }
                            },
                            Some(req) => {
                                self.send(
                                    req,
                                    &mut io,
                                    send_time.as_mut(),
                                    request_timeout.as_mut()
                                ).await?;
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
        Ok(Framed::new(socket, ModbusTCPCodec {}))
    }

    async fn send(
        &mut self,
        request: Request,
        io: &mut TcpIo,
        send_time: PinnedSleep<'_>,
        request_timeout: PinnedSleep<'_>,
    ) -> Result<(), Error> {
        let response_time = Duration::from_secs(request.expected_response_length().into())
            / (self.args.baudrate / 10);
        let response_ready_time = Instant::now() + response_time;
        let response_deadline = response_ready_time + *self.args.read_timeout;
        self.inflight
            .push_back((request.transaction_id, response_deadline));
        send_time.reset(response_ready_time + *self.args.tcp_send_delay);
        request_timeout.reset(self.inflight[0].1);

        // FIXME: shouldn't await here, these should be part of select!
        // somehow.
        io.send(&request).await.map_err(Error::Send)?;
        io.flush().await.map_err(Error::Flush)?;
        Ok(())
    }

    fn handle_response(
        &mut self,
        response: Response,
        send_time: pin::Pin<&mut tokio::time::Sleep>,
    ) {
        trace!(
            message = "decoded a response",
            transaction = response.transaction_id
        );
        let inflight_index = self
            .inflight
            .iter()
            .position(|(id, _)| *id == response.transaction_id);
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

use crate::modbus::{self, ModbusTCPCodec, Request};
use futures::{SinkExt, StreamExt as _};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::pin;
use std::sync::atomic::AtomicU16;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
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
    /// The reason the worker has stopped is shared between everything that was waiting on it.
    #[error("the connection worker has stopped")]
    WorkerStopped(#[source] Arc<Error>),
    #[error("the connection worker task did not complete")]
    WorkerTask(#[source] tokio::task::JoinError),
    #[error("the connection was shut down")]
    ConnectionClosed,
    #[error("could not read data from the stream")]
    Receive(#[source] std::io::Error),
    #[error("could not shut down the connection")]
    Shutdown(#[source] std::io::Error),
    #[error("could not send out the request")]
    Send(#[source] std::io::Error),
    #[error("could not flush out the request")]
    Flush(#[source] std::io::Error),
}

const MAX_WAIT_DURATION: std::time::Duration = std::time::Duration::from_secs(15);
/// The minimum amount of time between the starts of two subsequent connection attempts.
const RECONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);

/// Keeps track of the requests that are waiting for their response.
///
/// Every request gets a [`oneshot`] channel which the connection worker uses to hand over the
/// response (or a timeout). This way a response is never lost, regardless of whether it arrives
/// before or after the requester gets around to polling, and only the interested party is woken up.
#[derive(Default)]
pub struct ResponseTracker {
    state: Mutex<TrackerState>,
}

#[derive(Default)]
struct TrackerState {
    waiters: BTreeMap<u16, oneshot::Sender<Option<modbus::Response>>>,
    failure: Option<Arc<Error>>,
}

impl ResponseTracker {
    fn lock(&self) -> std::sync::MutexGuard<'_, TrackerState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Start waiting for a response to the request with the specified transaction ID.
    ///
    /// This must be called before the request is sent out.
    fn register(&self, transaction_id: u16) -> Result<Waiter<'_>, Error> {
        let mut state = self.lock();
        if let Some(reason) = &state.failure {
            return Err(Error::WorkerStopped(Arc::clone(reason)));
        }
        let (sender, receiver) = oneshot::channel();
        let previous = state.waiters.insert(transaction_id, sender);
        debug_assert!(previous.is_none(), "transaction ID {transaction_id} is already in use");
        Ok(Waiter { tracker: self, transaction_id, receiver })
    }

    pub fn mark_timeout(&self, transaction_id: u16) {
        let sender = self.lock().waiters.remove(&transaction_id);
        if let Some(sender) = sender {
            let _ = sender.send(None);
        }
    }

    pub fn add_response(&self, response: modbus::Response) {
        let sender = self.lock().waiters.remove(&response.transaction_id);
        if let Some(sender) = sender {
            let _ = sender.send(Some(response));
        }
    }

    /// Record that the worker has stopped and fail everything waiting on it.
    fn fail(&self, reason: Arc<Error>) {
        let mut state = self.lock();
        state.failure.get_or_insert(reason);
        // Dropping the senders wakes all the waiters up.
        let waiters = std::mem::take(&mut state.waiters);
        drop(state);
        drop(waiters);
    }

    fn failure(&self) -> Option<Error> {
        self.lock().failure.as_ref().map(|reason| Error::WorkerStopped(Arc::clone(reason)))
    }
}

/// A registered interest in a response. Stops being tracked when dropped.
struct Waiter<'a> {
    tracker: &'a ResponseTracker,
    transaction_id: u16,
    receiver: oneshot::Receiver<Option<modbus::Response>>,
}

impl Waiter<'_> {
    async fn wait(&mut self) -> Result<Option<modbus::Response>, Error> {
        match tokio::time::timeout(MAX_WAIT_DURATION, &mut self.receiver).await {
            Ok(Ok(response)) => Ok(response),
            // Senders are only ever dropped without sending by `ResponseTracker::fail`.
            Ok(Err(_)) => Err(self.tracker.failure().unwrap_or(Error::ConnectionClosed)),
            Err(_) => {
                tracing::warn!(
                    transaction_id = self.transaction_id,
                    "neither response nor timeout arrived within reasonable time; \
                     this is an implementation bug, timing out the request"
                );
                Ok(None)
            }
        }
    }
}

impl Drop for Waiter<'_> {
    fn drop(&mut self) {
        self.tracker.lock().waiters.remove(&self.transaction_id);
    }
}

#[derive(clap::Parser, Clone)]
#[group(id = "connection::Args")]
pub struct Args {
    #[clap(flatten)]
    how: ConnectionGroup,
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
    /// Connect to the SystemAIR device over Modbus TCP (usually available via the IAM module).
    #[arg(long, short = 't')]
    tcp: Option<String>,
    /// Connect to the SystemAIR device over Serial Modbus RTU.
    #[arg(long, short = 'd')]
    device: Option<PathBuf>,
    /// The modbus device ID.
    #[arg(long, short = 'i')]
    device_id: u8,
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
        } else if args.how.device.is_some() {
            todo!("direct Modbus RTU is not implemented yet");
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
        let request =
            modbus::Request { device_id: self.args.how.device_id, transaction_id, operation };
        let mut waiter = self.response_tracker.register(transaction_id)?;
        self.request_queue
            .send(request)
            .map_err(|e| self.response_tracker.failure().unwrap_or(Error::ScheduleRequest(e)))?;
        waiter.wait().await
    }

    /// [`Self::send`] but retries timeouts and `Server Busy` exceptions.
    pub async fn send_retrying(
        &self,
        operation: modbus::Operation,
    ) -> Result<modbus::Response, Error> {
        loop {
            let response = self.send(operation.clone()).await?;
            let Some(response) = response else { continue };
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
        let responses = Arc::clone(&self.responses);
        // The worker runs in its own task so that panics are observed too.
        let worker = tokio::task::spawn(self.main_loop(jobs));
        tokio::task::spawn(async move {
            let result = match worker.await {
                Ok(result) => result,
                Err(join_error) => Err(Error::WorkerTask(join_error)),
            };
            // Whatever the reason, nothing is going to answer the outstanding requests anymore.
            match result {
                Ok(()) => {
                    responses.fail(Arc::new(Error::ConnectionClosed));
                    Ok(())
                }
                Err(e) => {
                    let e = Arc::new(e);
                    tracing::error!(
                        message = "connection worker stopped",
                        error = (&*e as &dyn std::error::Error)
                    );
                    responses.fail(Arc::clone(&e));
                    Err(Error::WorkerStopped(e))
                }
            }
        })
    }

    async fn main_loop(
        mut self,
        mut jobs: UnboundedReceiver<modbus::Request>,
    ) -> Result<(), Error> {
        let mut pending_send: Option<modbus::Request> = None;
        let mut connected_before = false;
        // The first tick completes immediately. With `Delay` a late tick (long-lived connection)
        // schedules the next one a full interval from now, rather than catching up with a burst.
        let mut reconnect_interval = tokio::time::interval(RECONNECT_INTERVAL);
        reconnect_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        'reconnect: loop {
            // If we are reconnecting and had any in-flight requests, it is only proper to report
            // them as timed out.
            for (transaction_id, _) in self.inflight.drain(..) {
                self.responses.mark_timeout(transaction_id);
            }
            if let Some(req) = pending_send.take() {
                self.responses.mark_timeout(req.transaction_id);
            }
            // Connection attempts can fail instantly (e.g. connection refused) and connections can
            // get closed right after they were established. Make sure we're not spinning.
            reconnect_interval.tick().await;
            // FIXME: shouldn't await here, these should be part of select!
            // somehow.
            let io = match self.connect().await {
                Ok(io) => io,
                // Failing to establish the very first connection is most likely a configuration
                // problem and is reported right away. Later on the device may well be rebooting
                // or the network may be having a moment, so keep trying.
                Err(e) if connected_before => {
                    warn!(
                        message = "could not reconnect, will retry",
                        error = (&e as &dyn std::error::Error)
                    );
                    continue 'reconnect;
                }
                Err(e) => return Err(e),
            };
            connected_before = true;
            let (mut io_sink, mut io_source) = io.split();
            let mut send_time = pin::pin!(tokio::time::sleep_until(Instant::now()));
            let mut recv_time = pin::pin!(tokio::time::sleep_until(Instant::now()));
            loop {
                let time_to_send = send_time.is_elapsed();
                tokio::select! {
                    biased;
                    response = io_source.next() => {
                        let Some(response) = response else {
                            debug!(message="peer closed their connection, will reconnect");
                            continue 'reconnect;
                        };
                        match response {
                            Err(e) => {
                                warn!(
                                    message = "receiving a response failed, will reconnect",
                                    error = (&e as &dyn std::error::Error)
                                );
                                continue 'reconnect;
                            }
                            Ok(response) => self.handle_response(response),
                        }
                        // The request at the front of the queue may have changed; make sure the
                        // timeout tracks the new front rather than one that has already been
                        // answered (or dropped).
                        if let Some((_, deadline)) = self.inflight.front() {
                            recv_time.as_mut().reset(*deadline);
                        }
                        // If no other requests are in-flight, then IAM is idle and we can send a
                        // request to it right away.
                        if self.inflight.is_empty() {
                            send_time.as_mut().reset(Instant::now());
                        }
                    }
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
                    _ = &mut recv_time, if !self.inflight.is_empty() => {
                        if !self.handle_timeout(recv_time.as_mut()) {
                            continue 'reconnect;
                        }
                    }

                    // We need to have some down time between sending out subsequent modbus
                    // requests -- otherwise the IAM v1 modbus gateway gets somewhat confused and
                    // will ignore some of the requests altogether, leading them to time out.
                    //
                    // This conditional select will make sure that we pace our requests according
                    // to the expected underlying serial communication speed. There's no real
                    // reason to send more often than that even for non-IAM gateways, as the
                    // communication between the gateway and the device behind it is limited to the
                    // serial baudrate.
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
                                if let Err(e) = io_sink.feed(req.clone()).await {
                                    warn!(
                                        message = "queueing a request failed, will reconnect",
                                        error = (&e as &dyn std::error::Error)
                                    );
                                    self.responses.mark_timeout(req.transaction_id);
                                    continue 'reconnect;
                                }
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

    fn handle_response(&mut self, response: modbus::Response) {
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
    }

    fn handle_timeout(&mut self, request_timeout: pin::Pin<&mut tokio::time::Sleep>) -> bool {
        let (transaction_id, _) = self.inflight.pop_front().expect("unreachable");
        debug!(
            message = "an inflight request timed out",
            transaction_id,
            reconnect_countdown = self.reconnect_countdown
        );
        self.responses.mark_timeout(transaction_id);
        self.reconnect_countdown = self.reconnect_countdown.saturating_sub(1);
        if self.reconnect_countdown == 0 {
            return false;
        }
        if let Some((_, timeout)) = self.inflight.front() {
            request_timeout.reset(*timeout);
        }
        true
    }
}

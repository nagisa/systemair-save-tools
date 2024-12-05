use crate::modbus::{ModbusTCPCodec, Request, Response};
use futures::{SinkExt as _, StreamExt as _};
use std::collections::BTreeMap;
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
use tracing::{debug, trace};

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
            tokio::task::spawn(tcp_worker(jobs, Arc::clone(&response_tracker), args))
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

async fn tcp_worker(
    mut jobs: UnboundedReceiver<Request>,
    responses: Arc<ResponseTracker>,
    args: Args,
) -> Result<(), Error> {
    let mut inflight_keys = BTreeMap::new();
    let mut inflight = tokio_util::time::delay_queue::DelayQueue::new();
    'reconnect: loop {
        // If we are reconnecting and had any in-flight requests, it is only proper to mark them as
        // timed out.
        for transaction_id in std::mem::take(&mut inflight_keys).into_keys() {
            responses.mark_timeout(transaction_id);
        }
        inflight.clear();

        let address = args.how.tcp.as_ref().unwrap();
        debug!(message = "connecting...", address);
        let addresses = tokio::net::lookup_host(address)
            .await
            .map_err(|e| Error::LookupHost(e, address.to_string()))?
            .collect::<Vec<_>>();
        trace!(message = "resolved", ?addresses);
        let socket = TcpStream::connect(&*addresses)
            .await
            .map_err(|e| Error::Connect(e, address.to_string()))?;
        let nodelay_result = socket.set_nodelay(true);
        trace!(message = "setting nodelay", is_error = ?nodelay_result.err());
        let mut io = Framed::new(socket, ModbusTCPCodec {});
        let mut sequential_timeout_countdown = args.reconnect_after_timeouts;
        let mut send_slot_sleeper = pin::pin!(tokio::time::sleep_until(Instant::now()));
        loop {
            let send_slot_available = send_slot_sleeper.is_elapsed();
            tokio::select! {
                biased;
                Some(response) = io.next() => {
                    send_slot_sleeper.as_mut().reset(Instant::now());
                    let _ = futures::poll!(send_slot_sleeper.as_mut());
                    match response {
                        Err(e) => return Err(Error::Receive(e)),
                        Ok(response) => {
                            let Some(key) = inflight_keys.remove(&response.transaction_id) else {
                                debug!(
                                    message = "decoded a response we were not expecting",
                                    transaction=response.transaction_id
                                );
                                continue;
                            };
                            inflight.try_remove(&key);
                            trace!(
                                message = "decoded a response",
                                transaction=response.transaction_id
                            );
                            responses.add_response(response);
                            sequential_timeout_countdown = args.reconnect_after_timeouts;
                        }
                    }
                }

                Some(expired) = inflight.next() => {
                    let transaction_id: u16 = expired.into_inner();
                    inflight_keys.remove(&transaction_id);
                    debug!(message = "an inflight request timed out", transaction_id);
                    responses.mark_timeout(transaction_id);
                    if let Some(new_count) = sequential_timeout_countdown.checked_sub(1) {
                        sequential_timeout_countdown = new_count;
                    } else {
                        continue 'reconnect
                    };
                }

                // We need to have some down time between sending out subsequent modbus requests --
                // otherwise the IAM device gets somewhat confused and will ignore some of the
                // commands, leading them to time out.
                //
                // This conditional select will make sure that we will always wait sleeping until
                // the next available sending slot opens up.
                _ = &mut send_slot_sleeper, if !send_slot_available => {}

                job = jobs.recv(), if send_slot_available => {
                    match job {
                        None => {
                            io.get_mut().shutdown_write().await.map_err(Error::Shutdown)?;
                            if inflight.is_empty() {
                                return Ok(());
                            }
                        },
                        Some(req) => {
                            let response_time = Duration::from_secs(
                                req.expected_response_length().into()
                            ) / (args.baudrate / 10);
                            trace!(anticipated_response_duration = ?response_time);
                            let response_ready_time = Instant::now() + response_time;
                            send_slot_sleeper.as_mut().reset(response_ready_time + *args.tcp_send_delay);
                            let response_deadline = response_ready_time + *args.read_timeout;
                            let key = inflight.insert_at(req.transaction_id, response_deadline);
                            if let Some(prev_key) = inflight_keys.insert(req.transaction_id, key) {
                                inflight.try_remove(&prev_key);
                            };
                            io.send(&req).await.map_err(Error::Send)?;
                            io.flush().await.map_err(Error::Flush)?;
                        }
                    }
                },
            }
        }
    }
}

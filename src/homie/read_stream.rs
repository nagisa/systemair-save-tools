use super::ReadStreamError;
use crate::connection::Connection;
use crate::modbus::{Operation, Request, Response, ResponseKind};
use crate::registers::{RegisterIndex, Value, ADDRESSES};
use futures::stream::SelectAll;
use futures::{Stream, StreamExt as _};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::Instant;

macro_rules! range {
    ($($from: literal ..= $to: literal every $n: expr,)*) => {
        const {
            [$((
                RegisterIndex::from_address($from).unwrap(),
                RegisterIndex::from_address($to).unwrap(),
                $n,
            )),*]
        }
    }
}

// FIXME: may want to have read periods configurable by the user...
const READS: [(RegisterIndex, RegisterIndex, Duration); 39] = range![
    1001..=1044 every Duration::from_secs(15),
    1101..=1190 every Duration::from_secs(120), // NB: IAQ level may want frequent reads.
    1221..=1310 every Duration::from_secs(120),
    1351..=1440 every Duration::from_secs(120),
    1621..=1626 every Duration::from_secs(120),
    2001..=2122 every Duration::from_secs(120),
    2134..=2213 every Duration::from_secs(120),
    2311..=2317 every Duration::from_secs(120),
    2403..=2506 every Duration::from_secs(120),
    3101..=3117 every Duration::from_secs(120),
    4101..=4113 every Duration::from_secs(120),
    5001..=5114 every Duration::from_secs(120),
    6001..=6101 every Duration::from_secs(120),
    7001..=7007 every Duration::from_secs(120),
    9001..=9003 every Duration::from_secs(120),
    11401..=11422 every Duration::from_secs(120),
    12011..=12032 every Duration::from_secs(120),
    12101..=12166 every Duration::from_secs(120),
    12301..=12405 every Duration::from_secs(120),
    12544..=12544 every Duration::from_secs(120),
    12931..=12990 every Duration::from_secs(120),
    13201..=13315 every Duration::from_secs(120),
    13601..=13602 every Duration::from_secs(120),
    13801..=13802 every Duration::from_secs(120),
    14001..=14104 every Duration::from_secs(120),
    14201..=14304 every Duration::from_secs(120),
    14351..=14381 every Duration::from_secs(120),
    15002..=15122 every Duration::from_secs(120),
    15128..=15185 every Duration::from_secs(120),
    15502..=15549 every Duration::from_secs(120),
    15701..=15800 every Duration::from_secs(240),
    15801..=15900 every Duration::from_secs(240),
    15901..=15903 every Duration::from_secs(5),
    16001..=16004 every Duration::from_secs(240),
    16051..=16062 every Duration::from_secs(240),
    16101..=16101 every Duration::from_secs(240),
    17001..=17003 every Duration::from_secs(240),
    17001..=17003 every Duration::from_secs(240),
    30101..=30106 every Duration::from_secs(240),
];

const _ASSERT_ALL_REGISTERS_COVERED: () = const {
    let mut idx = 0;
    'next_address: while idx < ADDRESSES.len() {
        let address = ADDRESSES[idx];
        let mut read_idx = 0;
        while read_idx < READS.len() {
            let (range_start, range_end, _) = READS[read_idx];
            let count = range_end.address() - range_start.address() as u16;
            assert!(
                count <= 123,
                "read ranges of > 123 registers aren't universally supported"
            );
            if address >= range_start.address() && address <= range_end.address() {
                idx += 1;
                continue 'next_address;
            }
            read_idx += 1;
        }
        let _not_all_addresses_covered: () = [][address as usize];
    }
};

/// Produces a stream that reads **all** the registers at an appropriate-ish reading frequency.
pub(crate) fn read_device(
    modbus: Arc<Connection>,
) -> SelectAll<Pin<Box<dyn Send + Sync + Stream<Item = RegisterEvent>>>> {
    let mut read_stream: SelectAll<Pin<Box<dyn Send + Sync + Stream<Item = _>>>> = SelectAll::new();
    // FIXME: in some cases we want to read registers frequently (e.g. `1590{1,2,3}` and if they
    // change, then read all alarms) and not much reading otherwise. Similarly we may want to
    // quickly read some registers after other registers were written.
    for (from, to, period) in READS {
        let stream = modbus_stream_register_changes(&modbus, from, to, period);
        read_stream.push(Box::pin(stream));
    }
    read_stream
}

fn modbus_read_stream(
    modbus: Arc<Connection>,
    operation: Operation,
    period: Duration,
) -> impl Send + Sync + Stream<Item = Result<Response, ReadStreamError>> {
    let next_slot = Arc::new(Mutex::new(Instant::now()));
    futures::stream::repeat(modbus.new_transaction_id()).then(move |transaction_id| {
        let modbus = Arc::clone(&modbus);
        let next_slot = Arc::clone(&next_slot);
        async move {
            loop {
                {
                    let timeout = *next_slot.lock().unwrap_or_else(|e| e.into_inner());
                    tokio::time::sleep_until(timeout).await;
                }
                let outcome = modbus
                    .send(Request {
                        device_id: 1,
                        transaction_id,
                        operation,
                    })
                    .await
                    .map_err(ReadStreamError::Send)?;
                let Some(result) = outcome else {
                    continue;
                };
                let mut next_slot = next_slot.lock().unwrap_or_else(|e| e.into_inner());
                if result.is_server_busy() {
                    // IAM was busy with other requests. Give it some time…
                    // TODO: maybe add a flag to control this?
                    // TODO: configurable retry sleep time?
                    *next_slot = Instant::now() + std::time::Duration::from_millis(25);
                    continue;
                }
                *next_slot = Instant::now() + period;
                return Ok::<_, ReadStreamError>(result);
            }
        }
    })
}

#[derive(Clone)]
pub(crate) struct RegisterEvent {
    pub(crate) register: RegisterIndex,
    pub(crate) kind: RegisterEventKind,
}

#[derive(Clone, Debug)]
pub(crate) enum RegisterEventKind {
    /// Value has been read out.
    Value(Value),
    /// There was an error reading the value behind this property.
    ///
    /// This error may be transient.
    ReadError(Arc<ReadStreamError>),
    /// There was a server exception indicated in the response.
    ///
    /// This error may be transient.
    ServerException(u8),
}

fn extract_value(offset: usize, register: RegisterIndex, response: &[u8]) -> Option<Value> {
    let value_offset = 2 * offset;
    let value_data_type = register.data_type();
    value_data_type
        .from_bytes(&response[value_offset..][..value_data_type.bytes()])
        .next()
}

fn modbus_stream_register_changes(
    modbus: &Arc<Connection>,
    start_register: RegisterIndex,
    end_register: RegisterIndex,
    period: Duration,
) -> impl Stream<Item = RegisterEvent> {
    let count = (start_register.address()..=end_register.address()).len() as u16;
    debug_assert!(
        count <= 123,
        "read ranges of > 123 registers aren't universally supported"
    );
    let operation = Operation::GetHoldings {
        address: start_register.address(),
        count,
    };
    modbus_read_stream(Arc::clone(modbus), operation, period).flat_map(move |vs| {
        let vs = vs.map_err(Arc::new);
        let iter = (start_register.address()..=end_register.address())
            .enumerate()
            .filter_map(move |(idx, addr)| {
                let register = RegisterIndex::from_address(addr)?;
                let kind = match &vs {
                    Err(e) => RegisterEventKind::ReadError(Arc::clone(&e)),
                    Ok(Response {
                        kind: ResponseKind::ErrorCode(e),
                        ..
                    }) => RegisterEventKind::ServerException(*e),
                    Ok(Response {
                        kind: ResponseKind::GetHoldings { values },
                        ..
                    }) => {
                        let value = extract_value(idx, register, values)?;
                        RegisterEventKind::Value(value)
                    }
                };
                Some(RegisterEvent { register, kind })
            });
        futures::stream::iter(iter)
    })
}

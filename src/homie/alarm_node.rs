use crate::connection::Connection;
use crate::modbus::{Operation, Response, ResponseKind};
use crate::registers::{RegisterIndex, Value, ADDRESS_INDICES};
use futures::future::Lazy;
use futures::{Stream, StreamExt as _};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::BTreeMap;
use std::pin::pin;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tracing::info;

use super::{PropertyEvent, PropertyEventKind, PropertyValue, ReadStreamError};

macro_rules! registers {
    ($(($i: literal, $n: literal),)*) => {
        const {
            [$(((RegisterIndex::from_address($i).unwrap(), HomieID::new_const($n))),)*]
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum AlarmValue {
    Clear = 0,
    Firing = 1,
    Evaluating = 2,
    Acknowledged = 3,
}

impl TryFrom<u16> for AlarmValue {
    type Error = ();
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Clear,
            1 => Self::Firing,
            2 => Self::Evaluating,
            3 => Self::Acknowledged,
            _ => todo!(),
        })
    }
}

impl PropertyValue for AlarmValue {
    fn value(&self) -> String {
        match self {
            AlarmValue::Clear | AlarmValue::Evaluating => "clear",
            AlarmValue::Firing | AlarmValue::Acknowledged => "firing",
        }
        .to_string()
    }

    fn target(&self) -> String {
        match self {
            AlarmValue::Clear | AlarmValue::Acknowledged => "clear",
            AlarmValue::Firing | AlarmValue::Evaluating => "firing",
        }
        .to_string()
    }
}

static ALARM_STATE_REGISTERS_1: [(RegisterIndex, HomieID); 25] = registers![
    (15002, "supply-air-fan-control"),
    (15009, "extract-air-fan-control"),
    (15016, "frost-protection"),
    (15023, "defrosting"),
    (15030, "supply-air-fan-rpm"),
    (15037, "extract-air-fan-rpm"),
    (15058, "frost-protection-sensor"),
    (15065, "outdoor-air-temperature"),
    (15072, "supply-air-temperature"),
    (15079, "room-air-temperature"),
    (15086, "extract-air-temperature"),
    (15093, "extra-controller-temperature"),
    (15100, "eft"), // TODO: de-TLA this name
    (15107, "overheat-protection-sensor"),
    (15114, "emergency-thermostat"),
    (15121, "rotor-guard"),
    (15128, "bypass-damper-position-sensor"),
    (15135, "secondary-air"),
    (15142, "filter"),
    (15149, "extra-controller"),
    (15156, "external-stop"),
    (15163, "relative-humidity"),
    (15170, "co2"),
    (15177, "low-supply-air-temperature"),
    (15184, "byf"), // TODO: de-TLA this name: probably has something to do with bypass damper.
];
static ALARM_STATE_REGISTERS_2: [(RegisterIndex, HomieID); 7] = registers![
    (15502, "manual-override-outputs"),
    (15509, "pdm-room-humidity-sensor"), // PDM = pulse density modulation
    (15516, "pdm-extract-room-temperature"),
    (15523, "manual-fan-stop"),
    (15530, "overheat-temperature"),
    (15537, "fire"),
    (15544, "filter-warning"),
];
static ALARM_TYPE_SUMMARY_REGISTERS: [(RegisterIndex, HomieID); 3] = registers![
    (15901, "summary-type-a"),
    (15902, "summary-type-b"),
    (15903, "summary-type-c"),
];

pub fn description() -> HomieNodeDescription {
    let alarm_property_format =
        HomiePropertyFormat::Enum(vec!["clear".to_string(), "firing".to_string()]);
    let properties = ALARM_STATE_REGISTERS_1
        .iter()
        .chain(ALARM_STATE_REGISTERS_2.iter())
        .chain(ALARM_TYPE_SUMMARY_REGISTERS.iter())
        .map(|(register_index, property_id)| {
            (
                property_id.clone(),
                PropertyDescriptionBuilder::new(HomieDataType::Enum)
                    .settable(register_index.mode().is_writable())
                    .retained(true)
                    .format(alarm_property_format.clone())
                    .build(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    HomieNodeDescription {
        name: Some("Device alarm management".to_string()),
        r#type: None,
        properties,
    }
}

fn get_holdings_for(registers: &[(RegisterIndex, HomieID)]) -> Operation {
    let init = (u16::MAX, u16::MIN);
    let (start, end) = registers
        .iter()
        .fold((u16::MAX, u16::MIN), |(min, max), &(v, _)| {
            (v.address().min(min), v.address().max(max))
        });
    assert!(start != init.0);
    assert!(end != init.1);
    Operation::GetHoldings {
        address: start,
        count: 1 + (end - start),
    }
}

fn stream_one<const N: usize>(
    node_id: HomieID,
    modbus: Arc<Connection>,
    registers: &'static [(RegisterIndex, HomieID); N],
    polling_period: Duration,
) -> impl Stream<Item = PropertyEvent> {
    let r = super::modbus_read_stream(modbus, get_holdings_for(registers), polling_period)
        .scan(
            [const { None::<AlarmValue> }; N],
            move |known_values, vs| {
                let start_address = registers[0].0.address();
                let vs = vs.map_err(Arc::new);
                let node_id = node_id.clone();
                let mut results = [const { None::<PropertyEvent> }; N];
                for (value_slot, (register_index, property_id)) in registers.iter().enumerate() {
                    let kind = match &vs {
                        Err(e) => PropertyEventKind::ReadError(Arc::clone(e)),
                        Ok(Response {
                            kind: ResponseKind::ErrorCode(e),
                            ..
                        }) => PropertyEventKind::ServerException(*e),
                        Ok(Response {
                            kind: ResponseKind::GetHoldings { values },
                            ..
                        }) => {
                            let offset = usize::from(register_index.address() - start_address);
                            let data_type = register_index.data_type();
                            let Some(Value::U16(value)) = data_type
                                .from_bytes(&values[offset..][..data_type.bytes()])
                                .next()
                            else {
                                panic!("decoding alarm value should always succeed")
                            };
                            let Ok(value) = AlarmValue::try_from(value) else {
                                todo!("invalid contents from modbus for alarms, report error")
                            };
                            let old_value =
                                std::mem::replace(&mut known_values[value_slot], Some(value));
                            if Some(value) == old_value {
                                PropertyEventKind::ValueRead(Box::new(value))
                            } else {
                                PropertyEventKind::ValueChanged(Box::new(value))
                            }
                        }
                    };
                    results[value_slot] = Some(PropertyEvent {
                        node_id: node_id.clone(),
                        property_name: property_id.clone(),
                        kind,
                    });
                }
                futures::future::ready(Some(results))
            },
        )
        .flat_map(|v| futures::stream::iter(v.into_iter().map(|v| v.unwrap())));
    r
}

// TODO: when summary registers change, read the alarm states immediately.
pub fn stream(
    node_id: HomieID,
    modbus: Arc<Connection>,
) -> [std::pin::Pin<Box<dyn Stream<Item = PropertyEvent>>>; 3] {
    let summary_stream = stream_one(
        node_id.clone(),
        Arc::clone(&modbus),
        &ALARM_TYPE_SUMMARY_REGISTERS,
        Duration::from_millis(500),
    );
    let state1_stream = stream_one(
        node_id.clone(),
        Arc::clone(&modbus),
        &ALARM_STATE_REGISTERS_1,
        Duration::from_millis(30000),
    );
    let state2_stream = stream_one(
        node_id,
        Arc::clone(&modbus),
        &ALARM_STATE_REGISTERS_2,
        Duration::from_millis(30000),
    );
    [
        Box::pin(summary_stream),
        Box::pin(state1_stream),
        Box::pin(state2_stream),
    ]
}

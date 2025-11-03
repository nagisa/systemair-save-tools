//! Exposes the currently active functions as properties..
//!
//! The SystemAIR units keep track of two bits worth of information for each alarm (except
//! summaries):
//!
//! * Whether the alarm is firing or not
//! * Whether the alarm is "pending" for the other state (in case of a firing alarm, clearing it
//! does not immediately make it go away -- instead it goes into the state "firing but cleared".)
//!
//! This perfectly maps to the `$target` mechanism exposed in Homie and so most of the alarms have
//! an `$target` property associated with them. For consistency the summary alarms use the same
//! mechanism, even though they cannot be set and will never have a target that isn't equal to the
//! value.
//!
//! TODO: the summary alarms are perfect to trigger early read out of the full alarm list.

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

    fn target(&self) -> Option<String> {
        Some(match self {
            AlarmValue::Clear | AlarmValue::Acknowledged => "clear",
            AlarmValue::Firing | AlarmValue::Evaluating => "firing",
        }
        .to_string())
    }
}

macro_rules! registers {
    ($(($i: literal, $n: literal),)*) => {
        const {
            [$(((RegisterIndex::from_address($i).unwrap(), HomieID::new_const($n))),)*]
        }
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


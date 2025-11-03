//! Exposes the current device alarms as properties.
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

use super::PropertyValue;
use crate::homie::node::{Node, NodeEvent};
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum AlarmValue {
    Clear = 0,
    Firing = 1,
    Evaluating = 2,
    Acknowledged = 3,
}

impl TryFrom<Value> for AlarmValue {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(match value.into_inner() {
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
        Some(
            match self {
                AlarmValue::Clear | AlarmValue::Acknowledged => "clear",
                AlarmValue::Firing | AlarmValue::Evaluating => "firing",
            }
            .to_string(),
        )
    }
}

impl AlarmValue {
    fn firing(&self) -> bool {
        matches!(self, AlarmValue::Firing | AlarmValue::Acknowledged)
    }
    fn targetting_firing(&self) -> bool {
        matches!(self, AlarmValue::Firing | AlarmValue::Evaluating)
    }
}

macro_rules! registers {
    ($(($i: literal, $n: literal),)*) => {
        const {
            [$(((RegisterIndex::from_address($i).unwrap(), HomieID::new_const($n))),)*]
        }
    }
}

static ALARM_STATE_REGISTERS: [(RegisterIndex, HomieID); 35] = registers![
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
    (15502, "manual-override-outputs"),
    (15509, "pdm-room-humidity-sensor"), // PDM = pulse density modulation
    (15516, "pdm-extract-room-temperature"),
    (15523, "manual-fan-stop"),
    (15530, "overheat-temperature"),
    (15537, "fire"),
    (15544, "filter-warning"),
    (15901, "summary-type-a"),
    (15902, "summary-type-b"),
    (15903, "summary-type-c"),
];

pub struct AlarmNode {
    device_values: [Option<Value>; ALARM_STATE_REGISTERS.len()],
    sender: Sender<NodeEvent>,
}

impl AlarmNode {
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel::<NodeEvent>(1024);
        Self {
            device_values: [None; ALARM_STATE_REGISTERS.len()],
            sender: sender,
        }
    }
}

impl Node for AlarmNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("alarm")
    }
    fn description(&self) -> HomieNodeDescription {
        let alarm_property_format =
            HomiePropertyFormat::Enum(vec!["clear".to_string(), "firing".to_string()]);
        let properties = ALARM_STATE_REGISTERS
            .iter()
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
    fn registers(&self) -> &'static [(RegisterIndex, HomieID)] {
        &ALARM_STATE_REGISTERS
    }

    fn on_register_value(&mut self, register: RegisterIndex, value: Value) {
        let registers = self.registers();
        let Ok(idx) = registers.binary_search_by_key(&register, |v| v.0) else {
            return;
        };
        let prop_id = &registers[idx].1;
        let old_value = self.device_values[idx];
        if old_value == Some(value) {
            return;
        }
        self.device_values[idx] = Some(value);
        let old_value = old_value.map(AlarmValue::try_from);
        let new = AlarmValue::try_from(value);
        let (tgt_changed, val_changed, new) = match (old_value, new) {
            (None | Some(Err(_)) | Some(Ok(_)), Err(_)) => return,
            (None | Some(Err(_)), Ok(new)) => (true, true, new),
            (Some(Ok(old)), Ok(new)) => (
                old.targetting_firing() != new.targetting_firing(),
                old.firing() != new.firing(),
                new,
            ),
        };
        let new = Arc::new(new);
        if tgt_changed {
            let _ignore_no_receivers = self.sender.send(NodeEvent::TargetChanged {
                node_id: self.node_id(),
                prop_id: prop_id.clone(),
                new: Arc::clone(&new) as _,
            });
        }
        if val_changed {
            let _ignore_no_receivers = self.sender.send(NodeEvent::PropertyChanged {
                node_id: self.node_id(),
                prop_id: prop_id.clone(),
                new,
            });
        }
    }

    fn node_events(&self) -> tokio::sync::broadcast::Receiver<NodeEvent> {
        self.sender.subscribe()
    }

    fn values_populated(&self) -> bool {
        self.device_values.iter().all(|v| v.is_some())
    }
}

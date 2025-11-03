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

use crate::homie::common::{PropertyDescription, PropertyValue};
use crate::homie::node::{property_registers, Node, NodeEvent, PropertyRegisterEntry};
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

#[derive(Copy, Clone, PartialEq, Eq, strum::FromRepr)]
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
        let byte = u8::try_from(value.into_inner()).map_err(|_| ())?;
        Self::from_repr(byte).ok_or(())
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

impl PropertyDescription for AlarmValue {
    fn description() -> homie5::device_description::HomiePropertyDescription {
        let alarm_property_format =
            HomiePropertyFormat::Enum(vec!["clear".to_string(), "firing".to_string()]);
        PropertyDescriptionBuilder::new(HomieDataType::Enum)
            .retained(true)
            .format(alarm_property_format.clone())
            .build()
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

static REGISTERS: [PropertyRegisterEntry; 35] = property_registers![
    (15002 is "supply-air-fan-control": AlarmValue),
    (15009 is "extract-air-fan-control": AlarmValue),
    (15016 is "frost-protection": AlarmValue),
    (15023 is "defrosting": AlarmValue),
    (15030 is "supply-air-fan-rpm": AlarmValue),
    (15037 is "extract-air-fan-rpm": AlarmValue),
    (15058 is "frost-protection-sensor": AlarmValue),
    (15065 is "outdoor-air-temperature": AlarmValue),
    (15072 is "supply-air-temperature": AlarmValue),
    (15079 is "room-air-temperature": AlarmValue),
    (15086 is "extract-air-temperature": AlarmValue),
    (15093 is "extra-controller-temperature": AlarmValue),
    (15100 is "eft": AlarmValue), // TODO: de-TLA this name
    (15107 is "overheat-protection-sensor": AlarmValue),
    (15114 is "emergency-thermostat": AlarmValue),
    (15121 is "rotor-guard": AlarmValue),
    (15128 is "bypass-damper-position-sensor": AlarmValue),
    (15135 is "secondary-air": AlarmValue),
    (15142 is "filter": AlarmValue),
    (15149 is "extra-controller": AlarmValue),
    (15156 is "external-stop": AlarmValue),
    (15163 is "relative-humidity": AlarmValue),
    (15170 is "co2": AlarmValue),
    (15177 is "low-supply-air-temperature": AlarmValue),
    (15184 is "byf": AlarmValue), // TODO: de-TLA this name: probably has something to do with bypass damper.
    (15502 is "manual-override-outputs": AlarmValue),
    (15509 is "pdm-room-humidity-sensor": AlarmValue), // PDM = pulse density modulation
    (15516 is "pdm-extract-room-temperature": AlarmValue),
    (15523 is "manual-fan-stop": AlarmValue),
    (15530 is "overheat-temperature": AlarmValue),
    (15537 is "fire": AlarmValue),
    (15544 is "filter-warning": AlarmValue),
    (15901 is "summary-type-a": AlarmValue),
    (15902 is "summary-type-b": AlarmValue),
    (15903 is "summary-type-c": AlarmValue),
];

pub struct AlarmNode {
    device_values: [Option<Value>; REGISTERS.len()],
    sender: Sender<NodeEvent>,
}

impl AlarmNode {
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel::<NodeEvent>(1024);
        Self {
            device_values: [None; REGISTERS.len()],
            sender: sender,
        }
    }
}

impl Node for AlarmNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("alarm")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = REGISTERS
            .iter()
            .map(|prop| {
                let mut description = (prop.mk_description)();
                description.settable = prop.register.mode().is_writable();
                (prop.prop_id.clone(), description)
            })
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("device alarm management".to_string()),
            r#type: None,
            properties,
        }
    }

    fn node_events(&self) -> tokio::sync::broadcast::Receiver<NodeEvent> {
        self.sender.subscribe()
    }

    fn values_populated(&self) -> bool {
        self.device_values.iter().all(|v| v.is_some())
    }

    fn broadcast_node_event(&self, node_event: NodeEvent) {
        let _ignore_no_receivers = self.sender.send(node_event);
    }

    fn registers(&self) -> &'static [super::node::PropertyRegisterEntry] {
        &REGISTERS
    }

    fn record_register_value(&mut self, index: usize, value: Value) -> Option<Option<Value>> {
        let old_value = self.device_values[index];
        if old_value == Some(value) {
            return None;
        }
        self.device_values[index] = Some(value);
        return Some(old_value);
    }
}

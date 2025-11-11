//! Exposes the current device alarms as properties.
//!
//! The SystemAIR units keep track of two bits worth of information for each alarm (except
//! summaries):
//!
//! * Whether the alarm is firing or not
//! * Whether the alarm is "pending" for the other state (in case of a firing alarm, clearing it
//! does not immediately make it go away -- instead it goes into the state "firing but cleared".)
//!
//! This maps okay to the `$target` mechanism exposed in Homie and so most of the alarms have an
//! `$target` property associated with them. For consistency the summary alarms use the same
//! mechanism, even though they cannot be set and will never have a target that isn't equal to the
//! value.
//!

// TODO: the summary alarms are perfect to trigger early read out of the full alarm list.
// TODO: alarm log?
// TODO: clearing the alarms is achieved by writing to the adjacent `_CLEAR` register :(

use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{PropertyDescription, PropertyValue, RegisterPropertyValue};
use crate::registers::Value;
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
     { "supply-air-fan-control": AlarmValue = register "ALARM_SAF_CTRL_ALARM" },
     { "extract-air-fan-control": AlarmValue = register "ALARM_EAF_CTRL_ALARM" },
     { "frost-protection": AlarmValue = register "ALARM_FROST_PROT_ALARM" },
     { "defrosting": AlarmValue = register "ALARM_DEFROSTING_ALARM" },
     { "supply-air-fan-rpm": AlarmValue = register "ALARM_SAF_RPM_ALARM" },
     { "extract-air-fan-rpm": AlarmValue = register "ALARM_EAF_RPM_ALARM" },
     { "frost-protection-sensor": AlarmValue = register "ALARM_FPT_ALARM" },
     { "outdoor-air-temperature": AlarmValue = register "ALARM_OAT_ALARM" },
     { "supply-air-temperature": AlarmValue = register "ALARM_SAT_ALARM" },
     { "room-air-temperature": AlarmValue = register "ALARM_RAT_ALARM" },
     { "extract-air-temperature": AlarmValue = register "ALARM_EAT_ALARM" },
     { "extra-controller-temperature": AlarmValue = register "ALARM_ECT_ALARM" },
     // TODO: de-TLA this name
     { "eft": AlarmValue = register "ALARM_EFT_ALARM" },
     { "overheat-protection-sensor": AlarmValue = register "ALARM_OHT_ALARM" },
     { "emergency-thermostat": AlarmValue = register "ALARM_EMT_ALARM" },
     { "rotor-guard": AlarmValue = register "ALARM_RGS_ALARM" },
     { "bypass-damper-position-sensor": AlarmValue = register "ALARM_BYS_ALARM" },
     { "secondary-air": AlarmValue = register "ALARM_SECONDARY_AIR_ALARM" },
     { "filter": AlarmValue = register "ALARM_FILTER_ALARM" },
     { "extra-controller": AlarmValue = register "ALARM_EXTRA_CONTROLLER_ALARM" },
     { "external-stop": AlarmValue = register "ALARM_EXTERNAL_STOP_ALARM" },
     { "relative-humidity": AlarmValue = register "ALARM_RH_ALARM" },
     { "co2": AlarmValue = register "ALARM_CO2_ALARM" },
     { "low-supply-air-temperature": AlarmValue = register "ALARM_LOW_SAT_ALARM" },
     // TODO: de-TLA this name: probably has something to do with bypass damper.
     { "byf": AlarmValue = register "ALARM_BYF_ALARM" },
     { "manual-override-outputs": AlarmValue = register "ALARM_MANUAL_OVERRIDE_OUTPUTS_ALARM" },
     { "pdm-room-humidity-sensor": AlarmValue = register "ALARM_PDM_RHS_ALARM" }, // PDM = pulse density modulation
     { "pdm-extract-room-temperature": AlarmValue = register "ALARM_PDM_EAT_ALARM" },
     { "manual-fan-stop": AlarmValue = register "ALARM_MANUAL_FAN_STOP_ALARM" },
     { "overheat-temperature": AlarmValue = register "ALARM_OVERHEAT_TEMPERATURE_ALARM" },
     { "fire": AlarmValue = register "ALARM_FIRE_ALARM_ALARM" },
     { "filter-warning": AlarmValue = register "ALARM_FILTER_WARNING_ALARM" },
     // TODO: add filter warning duration.
     { "summary-type-a": AlarmValue = register "ALARM_TYPE_A" },
     { "summary-type-b": AlarmValue = register "ALARM_TYPE_B" },
     { "summary-type-c": AlarmValue = register "ALARM_TYPE_C" },
] }

pub struct AlarmNode {}

impl AlarmNode {
    pub fn new() -> Self {
        Self {}
    }
}

impl Node for AlarmNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("alarm")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("device alarm management".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

#[derive(Copy, Clone, PartialEq, Eq, strum::FromRepr, strum::EnumString)]
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

impl RegisterPropertyValue for AlarmValue {
    fn to_modbus(&self) -> u16 {
        // TODO: maybe actually implement this via the clear register? But then this is not a
        // `RegisterPropertyValue`.
        unreachable!("alarms should not be writable")
    }
}

impl PropertyDescription for AlarmValue {
    fn description(_: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        let alarm_property_format =
            HomiePropertyFormat::Enum(vec!["clear".to_string(), "firing".to_string()]);
        PropertyDescriptionBuilder::new(HomieDataType::Enum)
            .format(alarm_property_format.clone())
            .build()
    }
}

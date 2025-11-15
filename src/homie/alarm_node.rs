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

// TODO: alarm log?

use crate::homie::EventResult;
use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    AggregatePropertyValue, PropertyDescription, PropertyValue, string_enum,
};
use crate::modbus;
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
     { "any": AlarmOutputValue = register "OUTPUT_ALARM" },
     { "supply-air-fan-control": AlarmValue = aggregate "ALARM_SAF_CTRL_ALARM" },
     { "extract-air-fan-control": AlarmValue = aggregate "ALARM_EAF_CTRL_ALARM" },
     { "frost-protection": AlarmValue = aggregate "ALARM_FROST_PROT_ALARM" },
     { "defrosting": AlarmValue = aggregate "ALARM_DEFROSTING_ALARM" },
     { "supply-air-fan-rpm": AlarmValue = aggregate "ALARM_SAF_RPM_ALARM" },
     { "extract-air-fan-rpm": AlarmValue = aggregate "ALARM_EAF_RPM_ALARM" },
     { "frost-protection-sensor": AlarmValue = aggregate "ALARM_FPT_ALARM" },
     { "outdoor-air-temperature": AlarmValue = aggregate "ALARM_OAT_ALARM" },
     { "supply-air-temperature": AlarmValue = aggregate "ALARM_SAT_ALARM" },
     { "room-air-temperature": AlarmValue = aggregate "ALARM_RAT_ALARM" },
     { "extract-air-temperature": AlarmValue = aggregate "ALARM_EAT_ALARM" },
     { "extra-controller-temperature": AlarmValue = aggregate "ALARM_ECT_ALARM" },
     // TODO: de-TLA this name
     { "eft": AlarmValue = aggregate "ALARM_EFT_ALARM" },
     { "overheat-protection-sensor": AlarmValue = aggregate "ALARM_OHT_ALARM" },
     { "emergency-thermostat": AlarmValue = aggregate "ALARM_EMT_ALARM" },
     { "rotor-guard": AlarmValue = aggregate "ALARM_RGS_ALARM" },
     { "bypass-damper-position-sensor": AlarmValue = aggregate "ALARM_BYS_ALARM" },
     { "secondary-air": AlarmValue = aggregate "ALARM_SECONDARY_AIR_ALARM" },
     { "filter": AlarmValue = aggregate "ALARM_FILTER_ALARM" },
     { "extra-controller": AlarmValue = aggregate "ALARM_EXTRA_CONTROLLER_ALARM" },
     { "external-stop": AlarmValue = aggregate "ALARM_EXTERNAL_STOP_ALARM" },
     { "relative-humidity": AlarmValue = aggregate "ALARM_RH_ALARM" },
     { "co2": AlarmValue = aggregate "ALARM_CO2_ALARM" },
     { "low-supply-air-temperature": AlarmValue = aggregate "ALARM_LOW_SAT_ALARM" },
     { "bypass-damper-feedback": AlarmValue = aggregate "ALARM_BYF_ALARM" },
     { "manual-override-outputs": AlarmValue = aggregate "ALARM_MANUAL_OVERRIDE_OUTPUTS_ALARM" },
     { "pdm-room-humidity-sensor": AlarmValue = aggregate "ALARM_PDM_RHS_ALARM" }, // PDM = pulse density modulation
     { "pdm-extract-room-temperature": AlarmValue = aggregate "ALARM_PDM_EAT_ALARM" },
     { "manual-fan-stop": AlarmValue = aggregate "ALARM_MANUAL_FAN_STOP_ALARM" },
     { "overheat-temperature": AlarmValue = aggregate "ALARM_OVERHEAT_TEMPERATURE_ALARM" },
     { "fire": AlarmValue = aggregate "ALARM_FIRE_ALARM_ALARM" },
     { "filter-warning": AlarmValue = aggregate "ALARM_FILTER_WARNING_ALARM" },
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

    // fn on_property_change(
    //     &self,
    //     _: HomieID,
    //     prop_idx: usize,
    //     modbus: std::sync::Arc<crate::connection::Connection>,
    //     _: Box<super::value::DynPropertyValue>,
    // ) -> std::pin::Pin<Box<super::EventStream>> {
    // }
}

string_enum! {
#[impl(TryFromValue)]
#[derive(Copy, Clone, PartialEq)]
#[repr(u16)]
pub enum AlarmValue {
    Clear = 0,
    Firing = 1,
    Evaluating = 2,
    Acknowledged = 3,
}
}

impl AlarmValue {
    fn new(current: Value) -> Result<Self, ()> {
        current.try_into()
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

impl AggregatePropertyValue for AlarmValue {
    const SETTABLE: bool = true;
    fn set(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: std::sync::Arc<crate::connection::Connection>,
    ) -> std::pin::Pin<Box<super::EventStream>> {
        let is_clear = *self == AlarmValue::Clear;
        Box::pin(async_stream::stream! {
            if !is_clear {
                // If you'd like to break something, you still can: try manual override registers!
                let why = "can only clear alarms, why would you want something to be broken?";
                yield Ok(EventResult::HomieNotSet { node_id, prop_idx, why });
                return;
            }
            // All alarm clear registers are + 1 of the register itself.
            let address = PROPERTIES[prop_idx].kind.registers()[0].address() + 1;
            let operation = modbus::Operation::SetHoldings { address, values: vec![1] };
            let response = modbus.send_retrying(operation.clone()).await?;
            if response.exception_code().is_some() {
                yield Ok(EventResult::HomieSet {
                    node_id: node_id.clone(),
                    prop_idx,
                    operation,
                    response: response.kind,
                });
            }
            let address = address - 1;
            let operation = modbus::Operation::GetHoldings { address, count: 1 };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            yield Ok(EventResult::HomieSet { node_id, prop_idx, operation, response });
        })
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

string_enum! {
#[impl(TryFromValue, PropertyDescription, RegisterPropertyValue)]
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum AlarmOutputValue {
    Clear = 0,
    Firing = 1,
}
}

impl PropertyValue for AlarmOutputValue {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }
    fn on_property_change(
        &self,
        _node_id: HomieID,
        prop_idx: usize,
        modbus: std::sync::Arc<crate::connection::Connection>,
    ) -> std::pin::Pin<Box<super::EventStream>> {
        // if `alarm/any` changed, lets do an immediate read-out of all alarm registers so that
        // we report them immediately? This means that at least for alarms a valid configuration
        // would be to have a really rare global poll rate and a frequent rate for just
        // `alarm/any`.
        if PROPERTIES[prop_idx].prop_id.as_str() == "any" {
            return Box::pin(async_stream::stream! {
                let register = const {RegisterIndex::from_name("ALARM_SAF_CTRL_ALARM").unwrap()};
                let address = register.address();
                let operation = modbus::Operation::GetHoldings { address, count: 119 };
                let response = modbus.send_retrying(operation.clone()).await?.kind;
                yield Ok(EventResult::Periodic { operation, response });
                let register = const {RegisterIndex::from_name("ALARM_BYS_ALARM").unwrap()};
                let address = register.address();
                let operation = modbus::Operation::GetHoldings { address, count: 57 };
                let response = modbus.send_retrying(operation.clone()).await?.kind;
                yield Ok(EventResult::Periodic { operation, response });
                let register = const {RegisterIndex::from_name("ALARM_MANUAL_OVERRIDE_OUTPUTS_ALARM").unwrap()};
                let address = register.address();
                let operation = modbus::Operation::GetHoldings { address, count: 42 };
                let response = modbus.send_retrying(operation.clone()).await?.kind;
                yield Ok(EventResult::Periodic { operation, response });
            });
        } else {
            return Box::pin(futures::stream::empty());
        }
    }
}

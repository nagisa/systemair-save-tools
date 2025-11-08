//! Exposes DEMC group of settings as a homie node.
//!
//! The CO2 and RH setpoints are exposed as settable properties with a `$target`, although for RH
//! the `winter/summer`-specific setpoints may be more interesting.
//!
//! Everything else is bog-standard boolean/integer parameters.
use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{string_enum, BooleanValue, DynPropertyValue, UintValue};
use crate::registers::Value;
use homie5::device_description::HomieNodeDescription;
use homie5::HomieID;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "highest-rh-sensor": UintValue = register "DEMC_RH_HIGHEST" },
    { "highest-co2-sensor": UintValue = register "DEMC_CO2_HIGHEST" },
    { "current-rh-setpoint": UintValue = register "DEMC_RH_PI_SP" },
    { "current-rh": UintValue = register "DEMC_RH_PI_FEEDBACK" },
    { "current-rh-airflow-demand": UintValue = register "DEMC_RH_PI_OUTPUT" },
    { "current-co2-setpoint": UintValue = register "DEMC_CO2_PI_SP" },
    { "current-co2": UintValue = register "DEMC_CO2_PI_FEEDBACK" },
    { "current-co2-airflow-demand": UintValue = register "DEMC_CO2_PI_OUTPUT" },
    { "rh-pband": UintValue = register "DEMC_RH_SETTINGS_PBAND" },
    { "rh-summer-setpoint": UintValue = register "DEMC_RH_SETTINGS_SP_SUMMER" },
    { "rh-winter-setpoint": UintValue = register "DEMC_RH_SETTINGS_SP_WINTER" },
    { "rh-enabled": BooleanValue = register "DEMC_RH_SETTINGS_ON_OFF" },
    { "season": Season = register "SUMMER_WINTER" },
    { "co2-pband": UintValue = register "DEMC_CO2_SETTINGS_PBAND" },
    { "co2-setpoint": UintValue = register "DEMC_CO2_SETTINGS_SP" },
    { "co2-enabled": BooleanValue = register "DEMC_CO2_SETTINGS_ON_OFF" },
    { "current-indoor-air-quality-level": IaqLevel = register "IAQ_LEVEL" },
] }

pub struct DemandControlNode {
    values: [Option<Box<DynPropertyValue>>; PROPERTIES.len()],
}

impl DemandControlNode {
    pub fn new() -> Self {
        Self {
            values: [const { None }; PROPERTIES.len()],
        }
    }
}

impl Node for DemandControlNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("demand-control")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("demand control settings and status".to_string()),
            r#type: None,
            properties,
        }
    }
    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
    fn property_value(&self, property_index: usize) -> Option<&DynPropertyValue> {
        self.values[property_index].as_deref()
    }
    fn set_property_value(
        &mut self,
        property_index: usize,
        value: Box<DynPropertyValue>,
    ) -> Option<Box<DynPropertyValue>> {
        self.values[property_index].replace(value)
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum Season {
        Summer = 0,
        Winter = 1,
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum IaqLevel {
        Economic = 0,
        Good = 1,
        Improving = 2,
    }
}

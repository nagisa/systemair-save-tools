use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{BooleanValue, CelsiusValue, StopDelay, UintValue};
use homie5::device_description::HomieNodeDescription;
use homie5::HomieID;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "active": BooleanValue = register "FUNCTION_ACTIVE_COOLING" },
    { "demand": UintValue = register "COOLER_FROM_SATC" },
    // NOTE: although this is a generic output register, the modbus register documentation
    // specifies that this is specifically a cooler AO value.
    { "current-speed": UintValue = register "OUTPUT_Y3_ANALOG" },
    { "circulation-pump-start-temperature": CelsiusValue = register "COOLER_CIRC_PUMP_START_T" },
    { "outdoor-air-temperature-interlock": CelsiusValue = register "COOLER_OAT_INTERLOCK_T" },
    { "circulation-pump-stop-delay": StopDelay = register "COOLER_CIRC_PUMP_STOP_DELAY" },
] }

pub struct CoolerNode {}

impl CoolerNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for CoolerNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("cooler")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("the cooler, its status and configuration".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{BooleanValue, CelsiusValue, SpcHumidityValue, UintValue};
use homie5::HomieID;
use homie5::device_description::HomieNodeDescription;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "active": BooleanValue = register "FUNCTION_ACTIVE_HEAT_RECOVERY" },
    // FIXME: is this the universal register for this? 2143..2145 seem like they would likely be
    // the {SP, FEEDBACK, OUTPUT}, basically the computed thing...
    { "current-speed": UintValue = register "OUTPUT_Y2_ANALOG" },
    { "defrosting-active": BooleanValue = register "FUNCTION_ACTIVE_DEFROSTING" },
    { "enable-cooling-recovery": BooleanValue = register "HEAT_EXCHANGER_COOLING_RECOVERY_ON_OFF" },
    { "cooling-recovery-active": BooleanValue = register "FUNCTION_ACTIVE_COOLING_RECOVERY" },
    { "enable-humidity-transfer": BooleanValue = register "ROTOR_RH_TRANSFER_CTRL_ON_OFF" },
    { "humidity-transfer-active": BooleanValue = register "FUNCTION_ACTIVE_MOISTURE_TRANSFER" },
    { "humidity-transfer-setpoint": CelsiusValue = register "ROTOR_RH_TRANSFER_CTRL_SETPOINT" },
    { "humidity-transfer-pband": UintValue = register "ROTOR_RH_TRANSFER_CTRL_PBAND" },
    { "humidity-transfer-itime": UintValue = register "ROTOR_RH_TRANSFER_CTRL_ITIME" },
    { "speed-limit-for-humidity-transfer": UintValue = register "HEAT_EXCHANGER_SPEED_LIMIT_RH_TRANSFER" },
    { "current-extract-air-humidity": SpcHumidityValue = register "ROTOR_EA_SPEC_HUMIDITY" },
    { "current-outdoor-air-humidity": SpcHumidityValue = register "ROTOR_OA_SPEC_HUMIDITY" },
    { "current-extract-air-humidity-setpoint": SpcHumidityValue = register "ROTOR_EA_SPEC_HUMIDITY_SETPOINT" },
    { "current-frost-protection-sensor-temperature": CelsiusValue = register "SENSOR_FPT" },
] }

pub struct HeatExchangerNode {}

impl HeatExchangerNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for HeatExchangerNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("heat-exchanger")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("the heat exchanger, its status and configuration".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

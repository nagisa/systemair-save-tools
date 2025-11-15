use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{CelsiusValue, string_enum};
use homie5::HomieID;
use homie5::device_description::HomieNodeDescription;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "setpoint": CelsiusValue = register "TC_SP" },
    { "cascade-room-setpoint": CelsiusValue = register "TC_CASCADE_SP" },
    { "cascade-min-supply-air-setpoint": CelsiusValue = register "TC_CASCADE_SP_MIN" },
    { "cascade-max-supply-air-setpoint": CelsiusValue = register "TC_CASCADE_SP_MAX" },
    { "mode": ControlMode = register "TC_CONTROL_MODE" },
    { "current-extract-air-setpoint": CelsiusValue = register "TC_EAT_RAT_SP" },
    { "current-room-setpoint": CelsiusValue = register "TC_EAT_RAT_SP" },
    { "current-supply-air-setpoint": CelsiusValue = register "TC_SP_SATC" },
    { "current-room-control-supply-air-setpoint": CelsiusValue = register "TC_ROOM_CTRL_SP_SATC" },
    { "current-outdoor-air-temperature": CelsiusValue = register "SENSOR_OAT" },
    { "current-supply-air-temperature": CelsiusValue = register "SENSOR_SAT" },
    { "current-room-temperature": CelsiusValue = register "SENSOR_RAT" },
    // Is this always correct? There's also SENSOR_EAT which reads 0 for me.
    { "current-extract-air-temperature": CelsiusValue = register "SENSOR_PDM_EAT_VALUE" },
] }

pub struct TemperatureControllerNode {}

impl TemperatureControllerNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for TemperatureControllerNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("temperature-controller")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("temperature controller and its settings".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyValue, RegisterPropertyValue, PropertyDescription)]
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum ControlMode {
        SupplyAir = 0,
        Room = 1,
        ExtractAir = 2,
    }
}

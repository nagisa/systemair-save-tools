use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    BooleanValue, CelsiusValue, PropertyDescription, PropertyValue, RegisterPropertyValue, StopDelay, UintValue
};
use crate::registers::Value;
use homie5::device_description::{HomieNodeDescription, PropertyDescriptionBuilder};
use homie5::HomieID;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "cooldown-active": BooleanValue = register "FUNCTION_ACTIVE_HEATER_COOL_DOWN" },
    { "remaining-cooldown-time": CooldownDuration = register "SPEED_ELECTRICAL_HEATER_HOT_COUNTER" },
    // INVERTED??
    { "demand": UintValue = register "SATC_HEAT_DEMAND" },
    { "active": BooleanValue = register "FUNCTION_ACTIVE_HEATING" },
    { "current": UintValue = register "PWM_TRIAC_OUTPUT" },
    { "enable-eco": BooleanValue = register "ECO_MODE_ON_OFF" },
    { "eco-active": BooleanValue = register "ECO_FUNCTION_ACTIVE" },
    { "eco-temperature-offset": CelsiusValue = register "ECO_T_Y1_OFFSET" },
    { "circulation-pump-start-temperature": CelsiusValue = register "HEATER_CIRC_PUMP_START_T" },
    { "circulation-pump-stop-delay": StopDelay = register "HEATER_CIRC_PUMP_START_T" },
] }

pub struct HeaterNode {}

impl HeaterNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for HeaterNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("heater")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("resistive heater for supply air temperature control".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

struct CooldownDuration(jiff::SignedDuration);
impl PropertyValue for CooldownDuration {
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl PropertyDescription for CooldownDuration {
    fn description(_prop: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(homie5::HomieDataType::Duration).build()
    }
}
impl RegisterPropertyValue for CooldownDuration {
    fn to_modbus(&self) -> u16 {
        self.0.as_secs().clamp(u16::MIN.into(), u16::MAX.into()) as u16
    }
}
impl TryFrom<&str> for CooldownDuration {
    type Error = jiff::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(value.parse()?))
    }
}
impl From<Value> for CooldownDuration {
    fn from(value: Value) -> Self {
        Self(jiff::SignedDuration::new(value.into_inner() as _, 0))
    }
}


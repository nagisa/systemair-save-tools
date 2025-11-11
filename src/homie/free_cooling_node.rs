use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    string_enum, BooleanValue, CelsiusValue, PropertyDescription, PropertyValue, UintValue,
};
use crate::registers::Value;
use homie5::device_description::{HomieNodeDescription, PropertyDescriptionBuilder};
use homie5::HomieID;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
     { "enabled": BooleanValue = register "FREE_COOLING_ON_OFF" },
     { "trigger-above-daytime-temperature": CelsiusValue = register "FREE_COOLING_OUTDOOR_DAYTIME_T" },
     { "stop-above-nighttime-temperature": CelsiusValue = register "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_HIGH_T_LIMIT" },
     { "stop-below-nighttime-temperature": CelsiusValue = register "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_LOW_T_LIMIT" },
     { "stop-below-room-temperature": CelsiusValue = register "FREE_COOLING_ROOM_CANCEL_T" },
     { "start-time": TimeValue = aggregate "FREE_COOLING_START_TIME_H", "FREE_COOLING_START_TIME_M" },
     { "end-time": TimeValue = aggregate "FREE_COOLING_END_TIME_H", "FREE_COOLING_END_TIME_M" },
     { "active": BooleanValue = register "FREE_COOLING_ACTIVE" },
] }

pub struct FreeCoolingNode {}

impl FreeCoolingNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for FreeCoolingNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("free-cooling")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("indoor cooling using cool night air".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

struct TimeValue(jiff::civil::Time);
impl TimeValue {
    fn new(hour: Value, minute: Value) -> Result<Self, ()> {
        Ok(Self(jiff::civil::time(
            hour.into_inner() as _,
            minute.into_inner() as _,
            0,
            0,
        )))
    }
}
impl PropertyValue for TimeValue {
    fn value(&self) -> String {
        format!("T{}", self.0)
    }
}
impl PropertyDescription for TimeValue {
    fn description(_prop: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(homie5::HomieDataType::Datetime)
            .settable(true)
            .build()
    }
}
impl TryFrom<&str> for TimeValue {
    type Error = jiff::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(value.parse()?))
    }
}

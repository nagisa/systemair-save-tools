use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{string_enum, unit, CelsiusValue, UintValue};
use homie5::HomieID;
use homie5::device_description::HomieNodeDescription;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
     { "type": CompensationType = register "FAN_OUTDOOR_COMP_TYPE" },
     { "max-when-winter": UintValue<unit::Percent> = register "FAN_OUTDOOR_COMP_MAX_VALUE" },
     { "max-when-winter-outdoor-temperature": CelsiusValue = register "FAN_OUTDOOR_COMP_MAX_TEMP" },
     { "current": UintValue<unit::Percent> = register "FAN_OUTDOOR_COMP_RESULT" },
     { "start-when-winter-outdoor-temperature": CelsiusValue = register "FAN_OUTDOOR_COMP_START_T_WINTER" },
     { "start-when-summer-outdoor-temperature": CelsiusValue = register "FAN_OUTDOOR_COMP_START_T_SUMMER" },
     { "max-when-summer-outdoor-temperature": CelsiusValue = register "FAN_OUTDOOR_COMP_STOP_T_SUMMER" },
     { "max-when-summer": UintValue<unit::Percent> = register "FAN_OUTDOOR_COMP_VALUE_SUMMER" },
] }

pub struct CompensationNode {}

impl CompensationNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for CompensationNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("compensation")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("outdoor temperature driven airflow speed compensation".to_string()),
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
    enum CompensationType {
        SafOnly = 0,
        SafEaf = 1,
    }
}

use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::string_enum;
use homie5::device_description::HomieNodeDescription;
use homie5::HomieID;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "digital-input-1-connection": DigitalInputConnection = register "DI_CONNECTION_1" },
    { "digital-input-2-connection": DigitalInputConnection = register "DI_CONNECTION_2" },
    { "digital-input-1-polarity": DigitalInputPolarity = register "DI_CFG_POLARITY_1" },
    { "digital-input-2-polarity": DigitalInputPolarity = register "DI_CFG_POLARITY_2" },
] }

pub struct InputNode {}

impl InputNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for InputNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("inputs")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("inputs and their configuration".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyDescription, PropertyValue, RegisterPropertyValue)]
    #[derive(Clone, Copy)]
    #[repr(u16)]
    enum DigitalInputConnection {
        None = 0,
        AwayMode = 1,
        VacuumCleanerMode = 3,
        CookerHoodMode = 4,
        CrowdedMode = 5,
        ExtraControllerEmergencyThermostat = 6,
        ExternalStop = 7,
        ExtraControllerAlarm = 8,
        FireplaceMode = 9,
        HolidayMode = 10,
        RefreshMode = 11,
        RotorGuardSensor = 12,
        ChangeOverFeedback = 13,
        FireAlarm = 14,
        ConfigurableDigitalInput1Mode = 15,
        ConfigurableDigitalInput2Mode = 16,
        ConfigurableDigitalInput3Mode = 17,
        PressureGuard = 18,
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyDescription, PropertyValue, RegisterPropertyValue)]
    #[derive(Clone, Copy)]
    #[repr(u16)]
    enum DigitalInputPolarity {
        NormallyOpen = 0,
        MormallyClosed = 1,
    }
}

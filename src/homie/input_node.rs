use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{CelsiusValue, string_enum};
use homie5::HomieID;
use homie5::device_description::HomieNodeDescription;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "digital-input-1-connection": DigitalInputConnection = register "DI_CONNECTION_1" },
    { "digital-input-2-connection": DigitalInputConnection = register "DI_CONNECTION_2" },
    { "digital-input-1-polarity": DigitalInputPolarity = register "DI_CFG_POLARITY_1" },
    { "digital-input-2-polarity": DigitalInputPolarity = register "DI_CFG_POLARITY_2" },
    { "analog-input-1-function": AnalogInputFunction = register "AI_1_FUNCTION" },
    { "analog-input-2-function": AnalogInputFunction = register "AI_2_FUNCTION" },
    { "analog-input-3-function": AnalogInputFunction = register "AI_3_FUNCTION" },
    { "analog-input-4-function": AnalogInputFunction = register "AI_4_FUNCTION" },
    { "analog-input-5-function": AnalogInputFunction = register "AI_5_FUNCTION" },
    { "analog-input-6-function": AnalogInputFunction = register "AI_6_FUNCTION" },
    { "analog-input-7-function": AnalogInputFunction = register "AI_7_FUNCTION" },
    { "analog-input-1-correction": CelsiusValue = register "AI_1_CORRECTION" },
    { "analog-input-2-correction": CelsiusValue = register "AI_2_CORRECTION" },
    { "analog-input-3-correction": CelsiusValue = register "AI_3_CORRECTION" },
    { "analog-input-4-correction": CelsiusValue = register "AI_4_CORRECTION" },
    { "analog-input-5-correction": CelsiusValue = register "AI_5_CORRECTION" },
    { "analog-input-6-correction": CelsiusValue = register "AI_6_CORRECTION" },
    { "analog-input-7-correction": CelsiusValue = register "AI_7_CORRECTION" },
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
        NormallyClosed = 1,
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyDescription, PropertyValue, RegisterPropertyValue)]
    #[derive(Clone, Copy)]
    #[repr(u16)]
    enum AnalogInputFunction {
        None = 0,
        OutdoorAirTemperature = 1,
        SupplyAirTemperature = 2,
        OverheatTemperature = 3,
        FrostProtectionTemperature = 4,
        RoomAirTemperature = 5,
        ExtractAirTemperature = 6,
        ExtraControllerTemperature = 7,
        EfficiencyTemperature = 8,
    }
}

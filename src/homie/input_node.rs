use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{CelsiusValue, IntValue, string_enum, unit};
use homie5::HomieID;
use homie5::device_description::HomieNodeDescription;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "digital-input-1-connection": DigitalInputConnection = register "DI_CONNECTION_1" },
    { "digital-input-2-connection": DigitalInputConnection = register "DI_CONNECTION_2" },
    { "digital-input-1-polarity": DigitalInputPolarity = register "DI_CFG_POLARITY_1" },
    { "digital-input-2-polarity": DigitalInputPolarity = register "DI_CFG_POLARITY_2" },
    { "analog-input-1-connection": AnalogInputConnection = register "AI_1_FUNCTION" },
    { "analog-input-2-connection": AnalogInputConnection = register "AI_2_FUNCTION" },
    { "analog-input-3-connection": AnalogInputConnection = register "AI_3_FUNCTION" },
    { "analog-input-4-connection": AnalogInputConnection = register "AI_4_FUNCTION" },
    { "analog-input-5-connection": AnalogInputConnection = register "AI_5_FUNCTION" },
    { "analog-input-6-connection": AnalogInputConnection = register "AI_6_FUNCTION" },
    { "analog-input-7-connection": AnalogInputConnection = register "AI_7_FUNCTION" },
    { "analog-input-1-correction": CelsiusValue = register "AI_1_CORRECTION" },
    { "analog-input-2-correction": CelsiusValue = register "AI_2_CORRECTION" },
    { "analog-input-3-correction": CelsiusValue = register "AI_3_CORRECTION" },
    { "analog-input-4-correction": CelsiusValue = register "AI_4_CORRECTION" },
    { "analog-input-5-correction": CelsiusValue = register "AI_5_CORRECTION" },
    { "analog-input-6-correction": CelsiusValue = register "AI_6_CORRECTION" },
    { "analog-input-7-correction": CelsiusValue = register "AI_7_CORRECTION" },
    { "extract-air-temperature-correction": CelsiusValue = register "PDM_CORRECTION_T" },
    { "extract-air-humidity-correction": IntValue<unit::Percent> = register "PDM_CORRECTION_RH" },
    { "universal-input-1-type": UniversalInputType = register "UI_1_MODE" },
    { "universal-input-1-analog-connection": UniversalInputAnalogConnection = register "UI_1_FUNCTION" },
    { "universal-input-1-correction": IntValue = register "UI_CFG_CORRECTION_ANALOG1" },
    { "universal-input-1-digital-connection": DigitalInputConnection = register "UI_CONNECTION_DIGITAL1" },
    { "universal-input-1-digital-polarity": DigitalInputPolarity = register "UI_CFG_POLARITY_DIGITAL1" },
    { "universal-input-2-type": UniversalInputType = register "UI_2_MODE" },
    { "universal-input-2-analog-connection": UniversalInputAnalogConnection = register "UI_2_FUNCTION" },
    { "universal-input-2-correction": IntValue = register "UI_CFG_CORRECTION_ANALOG2" },
    { "universal-input-2-digital-connection": DigitalInputConnection = register "UI_CONNECTION_DIGITAL2" },
    { "universal-input-2-digital-polarity": DigitalInputPolarity = register "UI_CFG_POLARITY_DIGITAL2" },
    { "universal-input-3-type": UniversalInputType = register "UI_3_MODE" },
    { "universal-input-3-analog-connection": UniversalInputAnalogConnection = register "UI_3_FUNCTION" },
    { "universal-input-3-correction": IntValue = register "UI_CFG_CORRECTION_ANALOG3" },
    { "universal-input-3-digital-connection": DigitalInputConnection = register "UI_CONNECTION_DIGITAL3" },
    { "universal-input-3-digital-polarity": DigitalInputPolarity = register "UI_CFG_POLARITY_DIGITAL3" },
    { "universal-input-4-type": UniversalInputType = register "UI_4_MODE" },
    { "universal-input-4-analog-connection": UniversalInputAnalogConnection = register "UI_4_FUNCTION" },
    { "universal-input-4-correction": IntValue = register "UI_CFG_CORRECTION_ANALOG4" },
    { "universal-input-4-digital-connection": DigitalInputConnection = register "UI_CONNECTION_DIGITAL4" },
    { "universal-input-4-digital-polarity": DigitalInputPolarity = register "UI_CFG_POLARITY_DIGITAL4" },
    { "universal-input-5-type": UniversalInputType = register "UI_5_MODE" },
    { "universal-input-5-analog-connection": UniversalInputAnalogConnection = register "UI_5_FUNCTION" },
    { "universal-input-5-correction": IntValue = register "UI_CFG_CORRECTION_ANALOG5" },
    { "universal-input-5-digital-connection": DigitalInputConnection = register "UI_CONNECTION_DIGITAL5" },
    { "universal-input-5-digital-polarity": DigitalInputPolarity = register "UI_CFG_POLARITY_DIGITAL5" },
    { "universal-input-6-type": UniversalInputType = register "UI_6_MODE" },
    { "universal-input-6-analog-connection": UniversalInputAnalogConnection = register "UI_6_FUNCTION" },
    { "universal-input-6-correction": IntValue = register "UI_CFG_CORRECTION_ANALOG6" },
    { "universal-input-6-digital-connection": DigitalInputConnection = register "UI_CONNECTION_DIGITAL6" },
    { "universal-input-6-digital-polarity": DigitalInputPolarity = register "UI_CFG_POLARITY_DIGITAL6" },
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
        BypassDamperPositionSensor = 2,
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
    enum AnalogInputConnection {
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

string_enum! {
    #[impl(TryFromValue, PropertyDescription, PropertyValue, RegisterPropertyValue)]
    #[derive(Clone, Copy)]
    #[repr(u16)]
    enum UniversalInputType {
        NotSet = 0,
        Analog = 1,
        Digital = 2,
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyDescription, PropertyValue, RegisterPropertyValue)]
    #[derive(Clone, Copy)]
    #[repr(u16)]
    enum UniversalInputAnalogConnection {
        None = 0,
        RelativeHumidity = 1,
        Co2 = 2,
        SupplyAirFanControl = 3,
        ExtractAirFanControl = 4,
        BypassDamperFeedback = 5,
    }
}

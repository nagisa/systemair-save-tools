use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    string_enum, BooleanValue, CelsiusValue, PropertyDescription, PropertyValue,
    RegisterPropertyValue, RemainingTimeValue, StopDelay, UintValue,
};
use crate::registers::Value;
use homie5::device_description::{HomieNodeDescription, PropertyDescriptionBuilder};
use homie5::HomieID;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    // We don't support non-SI units currently. Anybody looking to use these are welcome to
    // implement the functionality here :) All of these have to be set to 0 in order for this tool
    // to work.
    //
    // { "airflow-unit": FlowUnit = register "SYSTEM_UNIT_FLOW" },
    // { "pressure-unit": PressureUnit = register "SYSTEM_UNIT_PRESSURE" },
    // { "temperature-unit": PressureUnit = register "SYSTEM_UNIT_TEMPERATURE" },

    { "holiday-duration": UintValue = register "USERMODE_HOLIDAY_TIME" },
    { "holiday-digital-input-off-delay": UintValue = register "USERMODE_HOLIDAY_DI_OFF_DELAY" },
    { "away-duration": UintValue = register "USERMODE_AWAY_TIME" },
    { "away-digital-input-off-delay": UintValue = register "USERMODE_AWAY_DI_OFF_DELAY" },
    { "fireplace-duration": UintValue = register "USERMODE_FIREPLACE_TIME" },
    { "fireplace-digital-input-off-delay": UintValue = register "USERMODE_FIRPLACE_DI_OFF_DELAY" },
    { "refresh-duration": UintValue = register "USERMODE_REFRESH_TIME" }, // minutes
    { "refresh-digital-input-off-delay": UintValue = register "USERMODE_REFRESH_DI_OFF_DELAY" },
    { "crowded-duration": UintValue = register "USERMODE_CROWDED_TIME" },
    { "crowded-digital-input-off-delay": UintValue = register "USERMODE_CROWDED_DI_OFF_DELAY" },
    { "digital-input-1-off-delay": UintValue = register "CDI1_OFF_DELAY" },
    { "digital-input-2-off-delay": UintValue = register "CDI2_OFF_DELAY" },
    { "digital-input-3-off-delay": UintValue = register "CDI3_OFF_DELAY" },
    { "remaining-duration": RemainingTimeValue = aggregate
        "USERMODE_REMAINING_TIME_L", "USERMODE_REMAINING_TIME_H"
    },
    { "ditigal-input-1-remaining-duration": RemainingTimeValue = aggregate
        "USERMODE_REMAINING_TIME_CDI1_L", "USERMODE_REMAINING_TIME_CDI1_H"
    },
    { "ditigal-input-2-remaining-duration": RemainingTimeValue = aggregate
        "USERMODE_REMAINING_TIME_CDI2_L", "USERMODE_REMAINING_TIME_CDI2_H"
    },
    { "ditigal-input-3-remaining-duration": RemainingTimeValue = aggregate
        "USERMODE_REMAINING_TIME_CDI3_L", "USERMODE_REMAINING_TIME_CDI3_H"
    },
    { "digital-input-1-active": BooleanValue = register "FUNCTION_ACTIVE_CDI_1" },
    { "digital-input-2-active": BooleanValue = register "FUNCTION_ACTIVE_CDI_2" },
    { "digital-input-3-active": BooleanValue = register "FUNCTION_ACTIVE_CDI_3" },
    { "pressure-guard-active": BooleanValue = register "FUNCTION_ACTIVE_PRESSURE_GUARD" },
    { "cooker-hood-active": BooleanValue = register "FUNCTION_ACTIVE_COOKER_HOOD" },
    { "vacuum-cleaner-active": BooleanValue = register "FUNCTION_ACTIVE_VACUUM_CLEANER" },
    { "secondary-air-active": BooleanValue = register "FUNCTION_ACTIVE_SECONDARY_AIR" },
    { "crowded-temperature-setpoint-offset": CelsiusValue = register "USERMODE_CROWDED_T_OFFSET" },
    // FIXME: set command for this
    { "current": CurrentMode = aggregate "USERMODE_MODE" },
] }

pub struct ModeNode {}

impl ModeNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for ModeNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("mode")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("settings for device operating modes".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Copy, Clone)]
    enum CurrentMode {
        Auto = 0,
        Manual = 1,
        Crowded = 2,
        Refresh = 3,
        Fireplace = 4,
        Away = 5,
        Holiday = 6,
        CookerHood = 7,
        VacuumCleaner = 8,
        ConfigurableDigitalInput1 = 9,
        ConfigurableDigitalInput2 = 10,
        ConfigurableDigitalInput3 = 11,
        PressureGuard = 12,
    }
}

impl CurrentMode {
    fn new(value: Value) -> Result<Self, ()> {
        value.try_into()
    }
}

#[repr(u16)]
#[derive(Copy, Clone)]
enum RequestMode {
    None = 0,
    Auto = 1,
    Manual = 2,
    Crowded = 3,
    Refresh = 4,
    Fireplace = 5,
    Away = 6,
    Holiday = 7,
}

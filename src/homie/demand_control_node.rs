//! Exposes DEMC group of settings as a homie node.
//!
//! The CO2 and RH setpoints are exposed as settable properties with a `$target`, although for RH
//! the `winter/summer`-specific setpoints may be more interesting.
//!
//! Everything else is bog-standard boolean/integer parameters.
use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{BooleanValue, RemainingTimeValue, UintValue, string_enum, unit};
use homie5::HomieID;
use homie5::device_description::HomieNodeDescription;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "highest-rh-sensor": UintValue<unit::Percent> = register "DEMC_RH_HIGHEST" },
    { "highest-co2-sensor": UintValue<unit::Ppm> = register "DEMC_CO2_HIGHEST" },
    { "current-rh-setpoint": UintValue<unit::Percent> = register "DEMC_RH_PI_SP" },
    { "current-rh": UintValue<unit::Percent> = register "DEMC_RH_PI_FEEDBACK" },
    { "current-rh-airflow-demand": UintValue = register "DEMC_RH_PI_OUTPUT" },
    { "current-co2-setpoint": UintValue<unit::Ppm> = register "DEMC_CO2_PI_SP" },
    { "current-co2": UintValue<unit::Ppm> = register "DEMC_CO2_PI_FEEDBACK" },
    { "current-co2-airflow-demand": UintValue = register "DEMC_CO2_PI_OUTPUT" },
    { "rh-pband": UintValue<unit::Percent> = register "DEMC_RH_SETTINGS_PBAND" },
    { "rh-summer-setpoint": UintValue<unit::Percent> = register "DEMC_RH_SETTINGS_SP_SUMMER" },
    { "rh-winter-setpoint": UintValue<unit::Percent> = register "DEMC_RH_SETTINGS_SP_WINTER" },
    { "rh-enabled": BooleanValue = register "DEMC_RH_SETTINGS_ON_OFF" },
    { "season": Season = register "SUMMER_WINTER_STATE" },
    { "time-until-summer": RemainingTimeValue = aggregate "SUMMER_WINTER_RESTART_COUNTER_L", "SUMMER_WINTER_RESTART_COUNTER_H" },
    { "active": BooleanValue = register "DEMC_ENABLED" },
    { "active-controller": ActiveController = register "DEMC_ACTIVE_CONTROLLER" },
    { "auto-mode-source": AutoModeSource = register "DEMC_AUTO_MODE_SOURCE" },
    { "co2-pband": UintValue<unit::Ppm> = register "DEMC_CO2_SETTINGS_PBAND" },
    { "co2-setpoint": UintValue<unit::Ppm> = register "DEMC_CO2_SETTINGS_SP" },
    { "co2-enabled": BooleanValue = register "DEMC_CO2_SETTINGS_ON_OFF" },
    { "current-indoor-air-quality-level": IaqLevel = register "IAQ_LEVEL" },
] }

pub struct DemandControlNode {}

impl DemandControlNode {
    pub fn new() -> Self {
        Self {}
    }
}

impl Node for DemandControlNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("demand-control")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("demand control settings and status".to_string()),
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
    enum Season {
        Summer = 0,
        Winter = 1,
        Identifying = 2,
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyValue, RegisterPropertyValue, PropertyDescription)]
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum IaqLevel {
        Economic = 0,
        Good = 1,
        Improving = 2,
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyValue, RegisterPropertyValue, PropertyDescription)]
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum ActiveController {
        Co2 = 0,
        Rh = 1,
    }
}

string_enum! {
    #[impl(TryFromValue, PropertyValue, RegisterPropertyValue, PropertyDescription)]
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum AutoModeSource {
        ExternalControl = 0,
        DemandControl = 1,
        WeekSchedule = 2,
        ConfigurationFault = 3,
    }
}

use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{string_enum, BooleanValue, DynPropertyValue, UintValue};
use crate::registers::Value;
use homie5::device_description::HomieNodeDescription;
use homie5::HomieID;
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "min-demand-control": AirflowLevel = register "IAQ_SPEED_LEVEL_MIN" },
    { "max-demand-control": AirflowLevel = register "IAQ_SPEED_LEVEL_MAX" },
    { "usermode-manual": AirflowLevel = register "USERMODE_MANUAL_AIRFLOW_LEVEL_SAF" },
    { "usermode-crowded-supply": AirflowLevel = register "USERMODE_CROWDED_AIRFLOW_LEVEL_SAF" },
    { "usermode-crowded-extract": AirflowLevel = register "USERMODE_CROWDED_AIRFLOW_LEVEL_EAF" },
    { "usermode-refresh-supply": AirflowLevel = register "USERMODE_REFRESH_AIRFLOW_LEVEL_SAF" },
    { "usermode-refresh-extract": AirflowLevel = register "USERMODE_REFRESH_AIRFLOW_LEVEL_EAF" },
    { "usermode-fireplace-supply": AirflowLevel = register "USERMODE_FIREPLACE_AIRFLOW_LEVEL_SAF" },
    { "usermode-fireplace-extract": AirflowLevel = register "USERMODE_FIREPLACE_AIRFLOW_LEVEL_EAF" },
    { "usermode-away-supply": AirflowLevel = register "USERMODE_AWAY_AIRFLOW_LEVEL_SAF" },
    { "usermode-away-extract": AirflowLevel = register "USERMODE_AWAY_AIRFLOW_LEVEL_EAF" },
    { "usermode-holiday-supply": AirflowLevel = register "USERMODE_HOLIDAY_AIRFLOW_LEVEL_SAF" },
    { "usermode-holiday-extract": AirflowLevel = register "USERMODE_HOLIDAY_AIRFLOW_LEVEL_EAF" },
    { "usermode-cooker-hood-supply": AirflowLevel = register "USERMODE_COOKERHOOD_AIRFLOW_LEVEL_SAF" },
    { "usermode-cooker-hood-extract": AirflowLevel = register "USERMODE_COOKERHOOD_AIRFLOW_LEVEL_EAF" },
    { "usermode-vacuum-cleaner-supply": AirflowLevel = register "USERMODE_VACUUMCLEANER_AIRFLOW_LEVEL_SAF" },
    { "usermode-vacuum-cleaner-extract": AirflowLevel = register "USERMODE_VACUUMCLEANER_AIRFLOW_LEVEL_EAF" },
    { "digital-input-1-supply": AirflowLevel = register "CDI_1_AIRFLOW_LEVEL_SAF" },
    { "digital-input-1-extract": AirflowLevel = register "CDI_1_AIRFLOW_LEVEL_EAF" },
    { "digital-input-2-supply": AirflowLevel = register "CDI_2_AIRFLOW_LEVEL_SAF" },
    { "digital-input-2-extract": AirflowLevel = register "CDI_2_AIRFLOW_LEVEL_EAF" },
    { "digital-input-3-supply": AirflowLevel = register "CDI_3_AIRFLOW_LEVEL_SAF" },
    { "digital-input-3-extract": AirflowLevel = register "CDI_3_AIRFLOW_LEVEL_EAF" },
    { "pressure-guard-supply": AirflowLevel = register "PRESSURE_GUARD_AIRFLOW_LEVEL_SAF" },
    { "pressure-guard-extract": AirflowLevel = register "PRESSURE_GUARD_AIRFLOW_LEVEL_EAF" },
    { "min-demand-control-speed": AirflowLevel = register "IAQ_SPEED_LEVEL_MIN" },
    { "max-demand-control-speed": AirflowLevel = register "IAQ_SPEED_LEVEL_MAX" },
    { "regulation-type": RegulationType = register "FAN_REGULATION_UNIT" },
    { "allow-manual-stop": BooleanValue = register "FAN_MANUAL_STOP_ALLOWED" },
    { "supply-percentage-for-minimum": UintValue = register "FAN_LEVEL_SAF_MIN_PERCENTAGE" },
    { "extract-percentage-for-minimum": UintValue = register "FAN_LEVEL_EAF_MIN_PERCENTAGE" },
    { "supply-percentage-for-low": UintValue = register "FAN_LEVEL_SAF_LOW_PERCENTAGE" },
    { "extract-percentage-for-low": UintValue = register "FAN_LEVEL_EAF_LOW_PERCENTAGE" },
    { "supply-percentage-for-normal": UintValue = register "FAN_LEVEL_SAF_NORMAL_PERCENTAGE" },
    { "extract-percentage-for-normal": UintValue = register "FAN_LEVEL_EAF_NORMAL_PERCENTAGE" },
    { "supply-percentage-for-high": UintValue = register "FAN_LEVEL_SAF_HIGH_PERCENTAGE" },
    { "extract-percentage-for-high": UintValue = register "FAN_LEVEL_EAF_HIGH_PERCENTAGE" },
    { "supply-percentage-for-maximum": UintValue = register "FAN_LEVEL_SAF_MAX_PERCENTAGE" },
    { "extract-percentage-for-maximum": UintValue = register "FAN_LEVEL_EAF_MAX_PERCENTAGE" },
    { "supply-rpm-for-minimum": UintValue = register "FAN_LEVEL_SAF_MIN_RPM" },
    { "extract-rpm-for-minimum": UintValue = register "FAN_LEVEL_EAF_MIN_RPM" },
    { "supply-rpm-for-low": UintValue = register "FAN_LEVEL_SAF_LOW_RPM" },
    { "extract-rpm-for-low": UintValue = register "FAN_LEVEL_EAF_LOW_RPM" },
    { "supply-rpm-for-normal": UintValue = register "FAN_LEVEL_SAF_NORMAL_RPM" },
    { "extract-rpm-for-normal": UintValue = register "FAN_LEVEL_EAF_NORMAL_RPM" },
    { "supply-rpm-for-high": UintValue = register "FAN_LEVEL_SAF_HIGH_RPM" },
    { "extract-rpm-for-high": UintValue = register "FAN_LEVEL_EAF_HIGH_RPM" },
    { "supply-rpm-for-maximum": UintValue = register "FAN_LEVEL_SAF_MAX_RPM" },
    { "extract-rpm-for-maximum": UintValue = register "FAN_LEVEL_EAF_MAX_RPM" },
    { "supply-pressure-for-minimum": UintValue = register "FAN_LEVEL_SAF_MIN_PRESSURE" },
    { "extract-pressure-for-minimum": UintValue = register "FAN_LEVEL_EAF_MIN_PRESSURE" },
    { "supply-pressure-for-low": UintValue = register "FAN_LEVEL_SAF_LOW_PRESSURE" },
    { "extract-pressure-for-low": UintValue = register "FAN_LEVEL_EAF_LOW_PRESSURE" },
    { "supply-pressure-for-normal": UintValue = register "FAN_LEVEL_SAF_NORMAL_PRESSURE" },
    { "extract-pressure-for-normal": UintValue = register "FAN_LEVEL_EAF_NORMAL_PRESSURE" },
    { "supply-pressure-for-high": UintValue = register "FAN_LEVEL_SAF_HIGH_PRESSURE" },
    { "extract-pressure-for-high": UintValue = register "FAN_LEVEL_EAF_HIGH_PRESSURE" },
    { "supply-pressure-for-maximum": UintValue = register "FAN_LEVEL_SAF_MAX_PRESSURE" },
    { "extract-pressure-for-maximum": UintValue = register "FAN_LEVEL_EAF_MAX_PRESSURE" },
    { "supply-flow-for-minimum": UintValue = register "FAN_LEVEL_SAF_MIN_FLOW" },
    { "extract-flow-for-minimum": UintValue = register "FAN_LEVEL_EAF_MIN_FLOW" },
    { "supply-flow-for-low": UintValue = register "FAN_LEVEL_SAF_LOW_FLOW" },
    { "extract-flow-for-low": UintValue = register "FAN_LEVEL_EAF_LOW_FLOW" },
    { "supply-flow-for-normal": UintValue = register "FAN_LEVEL_SAF_NORMAL_FLOW" },
    { "extract-flow-for-normal": UintValue = register "FAN_LEVEL_EAF_NORMAL_FLOW" },
    { "supply-flow-for-high": UintValue = register "FAN_LEVEL_SAF_HIGH_FLOW" },
    { "extract-flow-for-high": UintValue = register "FAN_LEVEL_EAF_HIGH_FLOW" },
    { "supply-flow-for-maximum": UintValue = register "FAN_LEVEL_SAF_MAX_FLOW" },
    { "extract-flow-for-maximum": UintValue = register "FAN_LEVEL_EAF_MAX_FLOW" },
    { "min-free-cooling-supply": AirflowLevel = register "FREE_COOLING_MIN_SPEED_LEVEL_SAF" },
    { "min-free-cooling-extract": AirflowLevel = register "FREE_COOLING_MIN_SPEED_LEVEL_EAF" },
    { "during-active-week-schedule": WeeklyScheduleLevel = register "WS_FAN_LEVEL_SCHEDULED" },
    { "during-inactive-week-schedule": WeeklyScheduleLevel = register "WS_FAN_LEVEL_UNSCHEDULED" },
] }

pub struct FanSpeedSettingsNode {}

impl FanSpeedSettingsNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for FanSpeedSettingsNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("fan-speed")
    }

    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("fan speed settings and status".to_string()),
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
    #[derive(Clone, Copy)]
    enum AirflowLevel {
        Off = 0,
        Minimum = 1,
        Low = 2,
        Normal = 3,
        High = 4,
        Maximum = 5,
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum WeeklyScheduleLevel {
        Off = 0,
        Minimum = 1,
        Low = 2,
        Normal = 3,
        High = 4,
        DemandControl = 5,
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum RegulationType {
        Manual = 0,
        RPM = 1,
        ConstantPressure = 2,
        ConstantFlow = 3,
        External = 4,
    }
}

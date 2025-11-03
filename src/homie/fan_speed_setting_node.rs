//! Exposes fan speed settings as a homie node.

use super::{homie_enum, homie_enum_format, BooleanValue, PropertyEvent, PropertyValue};
use crate::connection::Connection;
use crate::homie::node::Node;
use crate::registers::{RegisterIndex, Value};
use futures::Stream;
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use strum::VariantNames;

macro_rules! registers {
    ($(($i: literal, $n: literal),)*) => {
        const {
            [$(((RegisterIndex::from_address($i).unwrap(), HomieID::new_const($n))),)*]
        }
    }
}

static LVL_REGISTERS: [(RegisterIndex, HomieID); 25] = registers![
    (1121, "min-demand-control"),
    (1122, "max-demand-control"),
    (1131, "usermode-manual"),
    (1135, "usermode-crowded-supply"),
    (1136, "usermode-crowded-extract"),
    (1137, "usermode-refresh-supply"),
    (1138, "usermode-refresh-extract"),
    (1139, "usermode-fireplace-supply"),
    (1140, "usermode-fireplace-extract"),
    (1141, "usermode-away-supply"),
    (1142, "usermode-away-extract"),
    (1143, "usermode-holiday-supply"),
    (1144, "usermode-holiday-extract"),
    (1145, "usermode-cooker-hood-supply"),
    (1146, "usermode-cooker-hood-extract"),
    (1147, "usermode-vacuum-cleaner-supply"),
    (1148, "usermode-vacuum-cleaner-extract"),
    (1171, "digital-input-1-supply"),
    (1172, "digital-input-1-extract"),
    (1173, "digital-input-2-supply"),
    (1174, "digital-input-2-extract"),
    (1175, "digital-input-3-supply"),
    (1176, "digital-input-3-extract"),
    (1177, "pressure-guard-supply"),
    (1178, "pressure-guard-extract"),
];

static LVL_REGISTERS_2: [(RegisterIndex, HomieID); 2] = registers![
    (4112, "min-free-cooling-supply"),
    (4113, "min-free-cooling-extract"),
];

static WS_REGISTERS: [(RegisterIndex, HomieID); 2] = registers![
    (5060, "during-active-week-schedule"),
    (5061, "during-inactive-week-schedule"),
];

static LEVEL_SPEEDS: [(RegisterIndex, HomieID); 40] = registers![
    (1401, "supply-percentage-for-minimum"),
    (1402, "extract-percentage-for-minimum"),
    (1403, "supply-percentage-for-low"),
    (1404, "extract-percentage-for-low"),
    (1405, "supply-percentage-for-normal"),
    (1406, "extract-percentage-for-normal"),
    (1407, "supply-percentage-for-high"),
    (1408, "extract-percentage-for-high"),
    (1409, "supply-percentage-for-maximum"),
    (1410, "extract-percentage-for-maximum"),
    (1411, "supply-rpm-for-minimum"),
    (1412, "extract-rpm-for-minimum"),
    (1413, "supply-rpm-for-low"),
    (1414, "extract-rpm-for-low"),
    (1415, "supply-rpm-for-normal"),
    (1416, "extract-rpm-for-normal"),
    (1417, "supply-rpm-for-high"),
    (1418, "extract-rpm-for-high"),
    (1419, "supply-rpm-for-maximum"),
    (1420, "extract-rpm-for-maximum"),
    (1421, "supply-pressure-for-minimum"),
    (1422, "extract-pressure-for-minimum"),
    (1423, "supply-pressure-for-low"),
    (1424, "extract-pressure-for-low"),
    (1425, "supply-pressure-for-normal"),
    (1426, "extract-pressure-for-normal"),
    (1427, "supply-pressure-for-high"),
    (1428, "extract-pressure-for-high"),
    (1429, "supply-pressure-for-maximum"),
    (1430, "extract-pressure-for-maximum"),
    (1431, "supply-flow-for-minimum"),
    (1432, "extract-flow-for-minimum"),
    (1433, "supply-flow-for-low"),
    (1434, "extract-flow-for-low"),
    (1435, "supply-flow-for-normal"),
    (1436, "extract-flow-for-normal"),
    (1437, "supply-flow-for-high"),
    (1438, "extract-flow-for-high"),
    (1439, "supply-flow-for-maximum"),
    (1440, "extract-flow-for-maximum"),
];

const MANUAL_STOP: HomieID = HomieID::new_const("allow-manual-stop");
const REGULATION_TYPE: HomieID = HomieID::new_const("regulation-type");

pub struct FanSpeedSettingsNode;
impl Node for FanSpeedSettingsNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("fan-speed-settings")
    }

    fn description(&self) -> HomieNodeDescription {
        let mut properties = BTreeMap::new();
        let speed = homie_enum::<AirflowLevel>().settable(true).build();
        for (_, name) in &LVL_REGISTERS {
            properties.insert(name.clone(), speed.clone());
        }
        for (_, name) in &LVL_REGISTERS_2 {
            properties.insert(name.clone(), speed.clone());
        }
        let ws_level = homie_enum::<WeeklyScheduleLevel>().settable(true).build();
        for (_, name) in &WS_REGISTERS {
            properties.insert(name.clone(), ws_level.clone());
        }

        let settable_integer = PropertyDescriptionBuilder::new(HomieDataType::Integer)
            .settable(true)
            .build();
        for (_, name) in &LEVEL_SPEEDS {
            properties.insert(name.clone(), settable_integer.clone());
        }

        let settable_boolean = PropertyDescriptionBuilder::new(HomieDataType::Boolean)
            .settable(true)
            .build();
        properties.insert(MANUAL_STOP.clone(), settable_boolean.clone());

        let fan_regulation_type = homie_enum::<RegulationType>().settable(true).build();
        properties.insert(REGULATION_TYPE.clone(), fan_regulation_type.clone());

        HomieNodeDescription {
            name: Some("fan speed settings".to_string()),
            r#type: None,
            properties,
        }
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum AirflowLevel {
    Off = 0,
    Minimum = 1,
    Low = 2,
    Normal = 3,
    High = 4,
    Maximum = 5,
}

impl AirflowLevel {
    fn new(value: Value) -> Self {
        Self::from_repr(value.into_inner()).expect("TODO")
    }
}

impl PropertyValue for AirflowLevel {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum WeeklyScheduleLevel {
    Off = 0,
    Minimum = 1,
    Low = 2,
    Normal = 3,
    High = 4,
    DemandControl = 5,
}

impl WeeklyScheduleLevel {
    fn new(value: Value) -> Self {
        Self::from_repr(value.into_inner()).expect("TODO")
    }
}

impl PropertyValue for WeeklyScheduleLevel {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum RegulationType {
    Manual,
    RPM,
    ConstantPressure,
    ConstantFlow,
    External,
}

impl RegulationType {
    fn new(value: Value) -> Self {
        Self::from_repr(value.into_inner()).expect("TODO")
    }
}

impl PropertyValue for RegulationType {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

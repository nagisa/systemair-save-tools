//! Exposes fan speed settings as a homie node.

use super::{BooleanValue, PropertyEvent, PropertyValue};
use crate::connection::Connection;
use crate::registers::{RegisterIndex, Value};
use futures::Stream;
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{Homie5Message, HomieDataType, HomieID};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use strum::{VariantArray, VariantNames};

macro_rules! registers {
    ($(($i: literal, $n: literal),)*) => {
        const {
            [$(((RegisterIndex::from_address($i).unwrap(), HomieID::new_const($n))),)*]
        }
    }
}

static LVL_REGISTERS: [(RegisterIndex, HomieID); 23] = registers![
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

fn enum_format(variants: &[&str]) -> HomiePropertyFormat {
    HomiePropertyFormat::Enum(variants.iter().copied().map(Into::into).collect())
}

pub fn description() -> HomieNodeDescription {
    let mut properties = BTreeMap::new();
    let speed = PropertyDescriptionBuilder::new(HomieDataType::Enum)
        .format(enum_format(AirflowLevel::VARIANTS))
        .settable(true)
        .build();
    for (_, name) in &LVL_REGISTERS {
        properties.insert(name.clone(), speed.clone());
    }
    for (_, name) in &LVL_REGISTERS_2 {
        properties.insert(name.clone(), speed.clone());
    }
    let ws_level = PropertyDescriptionBuilder::new(HomieDataType::Enum)
        .format(enum_format(WeeklyScheduleLevel::VARIANTS))
        .settable(true)
        .build();
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

    let fan_regulation_type = PropertyDescriptionBuilder::new(HomieDataType::Enum)
        .format(enum_format(RegulationType::VARIANTS))
        .settable(true)
        .build();
    properties.insert(REGULATION_TYPE.clone(), fan_regulation_type.clone());

    HomieNodeDescription {
        name: Some("fan speed settings".to_string()),
        r#type: None,
        properties,
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

pub fn stream(
    node_id: HomieID,
    modbus: Arc<Connection>,
) -> [std::pin::Pin<Box<dyn Stream<Item = PropertyEvent>>>; 6] {
    let address: u16 = const { LVL_REGISTERS[0].0.address() };
    let count: u16 = 1179 - address;
    let stream_levels_1 = super::modbus_read_stream_flatmap_registers(
        &modbus,
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
        &node_id,
        LVL_REGISTERS
            .iter()
            .map(move |(r, p)| (*r, p.clone(), move |v| Box::new(AirflowLevel::new(v)) as _)),
    );

    let address: u16 = const { LVL_REGISTERS_2[0].0.address() };
    let count: u16 = 4114 - address;
    let stream_levels_2 = super::modbus_read_stream_flatmap_registers(
        &modbus,
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
        &node_id,
        LVL_REGISTERS_2
            .iter()
            .map(move |(r, p)| (*r, p.clone(), move |v| Box::new(AirflowLevel::new(v)) as _)),
    );

    let address: u16 = const { WS_REGISTERS[0].0.address() };
    let count: u16 = 5062 - address;
    let stream_week_schedule_levels = super::modbus_read_stream_flatmap_registers(
        &modbus,
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
        &node_id,
        WS_REGISTERS.iter().map(move |(r, p)| {
            (*r, p.clone(), move |v| {
                Box::new(WeeklyScheduleLevel::new(v)) as _
            })
        }),
    );

    let address: u16 = const { LEVEL_SPEEDS[0].0.address() };
    let count: u16 = 1441 - address;
    let stream_level_speeds = super::modbus_read_stream_flatmap_registers(
        &modbus,
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
        &node_id,
        LEVEL_SPEEDS
            .iter()
            .map(|(r, p)| (*r, p.clone(), |v| Box::new(super::SimpleValue(v)) as _)),
    );

    let register = const { RegisterIndex::from_name("FAN_MANUAL_STOP_ALLOWED").unwrap() };
    let address = register.address();
    let stream_manual_stop = super::modbus_read_stream_flatmap_registers(
        &modbus,
        crate::modbus::Operation::GetHoldings { address, count: 1 },
        Duration::from_secs(120),
        &node_id,
        [(register, MANUAL_STOP, |v| {
            Box::new(BooleanValue::from(v)) as _
        })],
    );

    let register = const { RegisterIndex::from_name("FAN_REGULATION_UNIT").unwrap() };
    let address = register.address();
    let stream_regulation_type = super::modbus_read_stream_flatmap_registers(
        &modbus,
        crate::modbus::Operation::GetHoldings { address, count: 1 },
        Duration::from_secs(120),
        &node_id,
        [(register, REGULATION_TYPE, |v| {
            Box::new(RegulationType::new(v)) as _
        })],
    );

    [
        Box::pin(stream_levels_1),
        Box::pin(stream_levels_2),
        Box::pin(stream_week_schedule_levels),
        Box::pin(stream_level_speeds),
        Box::pin(stream_manual_stop),
        Box::pin(stream_regulation_type),
    ]
}

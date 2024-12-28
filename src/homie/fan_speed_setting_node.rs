//! Exposes fan speed settings as a homie node.

use super::{PropertyEvent, PropertyValue, ReadStreamError};
use crate::connection::Connection;
use crate::homie::PropertyEventKind;
use crate::modbus::extract_value;
use crate::registers::{RegisterIndex, Value};
use futures::{Stream, StreamExt as _};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

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

pub fn description() -> HomieNodeDescription {
    let mut properties = BTreeMap::new();
    let speed_property_format = HomiePropertyFormat::Enum(vec![
        "off".to_string(),
        "minimum".to_string(),
        "low".to_string(),
        "normal".to_string(),
        "high".to_string(),
        "maximum".to_string(),
    ]);
    let speed = PropertyDescriptionBuilder::new(HomieDataType::Enum)
        .format(speed_property_format)
        .settable(true)
        .build();
    for (_, name) in &LVL_REGISTERS {
        properties.insert(name.clone(), speed.clone());
    }
    for (_, name) in &LVL_REGISTERS_2 {
        properties.insert(name.clone(), speed.clone());
    }
    let ws_property_format = HomiePropertyFormat::Enum(vec![
        "off".to_string(),
        "low".to_string(),
        "normal".to_string(),
        "high".to_string(),
        "demand-control".to_string(),
    ]);
    let ws_speed = PropertyDescriptionBuilder::new(HomieDataType::Enum)
        .format(ws_property_format)
        .settable(true)
        .build();
    for (_, name) in &WS_REGISTERS {
        properties.insert(name.clone(), ws_speed.clone());
    }

    let integer = PropertyDescriptionBuilder::new(HomieDataType::Integer).build();
    let settable_integer = PropertyDescriptionBuilder::new(HomieDataType::Integer)
        .settable(true)
        .build();
    for (_, name) in &LEVEL_SPEEDS {
        properties.insert(name.clone(), integer.clone());
    }

    HomieNodeDescription {
        name: Some("fan speed settings".to_string()),
        r#type: None,
        properties,
    }
}

#[derive(Clone, Copy)]
enum AirflowSpeed {
    Off,
    Minimum,
    Low,
    Normal,
    High,
    Maximum,
    DemandControl,
}

impl PropertyValue for AirflowSpeed {
    fn value(&self) -> String {
        match self {
            Self::Off => "off",
            Self::Minimum => "minimum",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Maximum => "maximum",
            Self::DemandControl => "demand-control",
        }
        .to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

fn speed_registers(
    node_id: &HomieID,
    base_address: u16,
    registers: &'static [(RegisterIndex, HomieID)],
    max: AirflowSpeed,
    response: Result<crate::modbus::Response, Arc<ReadStreamError>>,
) -> impl Iterator<Item = PropertyEvent> {
    let node_id = node_id.clone();
    registers.iter().map(move |(ri, prop_id)| {
        let kind = PropertyEventKind::from_holdings_response(&response, |vs| {
            let Some(Value::U16(value)) = extract_value(base_address, ri.address(), vs) else {
                panic!("decoding iaq properties should always succeed");
            };
            match value {
                0 => AirflowSpeed::Off,
                1 => AirflowSpeed::Minimum,
                2 => AirflowSpeed::Low,
                3 => AirflowSpeed::Normal,
                4 => AirflowSpeed::High,
                5 => max,
                _ => todo!(),
            }
        });
        PropertyEvent {
            node_id: node_id.clone(),
            property_name: prop_id.clone(),
            kind,
        }
    })
}

pub fn stream(
    node_id: HomieID,
    modbus: Arc<Connection>,
) -> [std::pin::Pin<Box<dyn Stream<Item = PropertyEvent>>>; 4] {
    let address: u16 = 1121;
    let count: u16 = 1179 - address;
    let nid = node_id.clone();
    let max = AirflowSpeed::Maximum;
    let stream_1 = super::modbus_read_stream(
        Arc::clone(&modbus),
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
    )
    .flat_map(move |vs| {
        let vs = vs.map_err(Arc::new);
        futures::stream::iter(speed_registers(&nid, address, &LVL_REGISTERS, max, vs))
    });

    let address: u16 = 4112;
    let count: u16 = 4114 - address;
    let nid = node_id.clone();
    let stream_2 = super::modbus_read_stream(
        Arc::clone(&modbus),
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
    )
    .flat_map(move |vs| {
        let vs = vs.map_err(Arc::new);
        futures::stream::iter(speed_registers(&nid, address, &LVL_REGISTERS_2, max, vs))
    });

    let address: u16 = 5060;
    let count: u16 = 5062 - address;
    let nid = node_id.clone();
    let max = AirflowSpeed::DemandControl;
    let stream_3 = super::modbus_read_stream(
        Arc::clone(&modbus),
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
    )
    .flat_map(move |vs| {
        let vs = vs.map_err(Arc::new);
        futures::stream::iter(speed_registers(&nid, address, &WS_REGISTERS, max, vs))
    });

    let address: u16 = 1401;
    let count: u16 = 1441 - address;
    let stream_4 = super::modbus_read_stream(
        modbus,
        crate::modbus::Operation::GetHoldings { address, count },
        Duration::from_secs(120),
    )
    .flat_map(move |vs| {
        let vs = vs.map_err(Arc::new);
        let node_id = node_id.clone();
        futures::stream::iter(LEVEL_SPEEDS.iter().map(move |(ri, prop_id)| {
            let kind = PropertyEventKind::from_holdings_response(&vs, |vs| {
                let Some(value) = extract_value(address, ri.address(), vs) else {
                    panic!("decoding iaq properties should always succeed");
                };
                super::SimpleProperty(value)
            });
            PropertyEvent {
                node_id: node_id.clone(),
                property_name: prop_id.clone(),
                kind,
            }
        }))
    });
    [
        Box::pin(stream_1),
        Box::pin(stream_2),
        Box::pin(stream_3),
        Box::pin(stream_4),
    ]
}

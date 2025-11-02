use crate::connection::Connection;
use crate::registers::RegisterIndex;
use futures::Stream;
use homie5::device_description::{HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder};
use homie5::{HomieDataType, HomieID, HOMIE_UNIT_DEGREE_CELSIUS};
use strum::VariantNames as _;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use super::PropertyEvent;

const TYPE_ID: HomieID = HomieID::new_const("type");
const WINTER_COMP_ID: HomieID = HomieID::new_const("max-winter-compensation");
const WINTER_MAX_TEMP_ID: HomieID = HomieID::new_const("max-winter-compensation-temperature");
const WINTER_MIN_TEMP_ID: HomieID = HomieID::new_const("start-winter-compensation-temperature");
const SUMMER_MAX_TEMP_ID: HomieID = HomieID::new_const("max-summer-compensation-temperature");
const SUMMER_MIN_TEMP_ID: HomieID = HomieID::new_const("start-summer-compensation-temperature");
const SUMMER_COMP_ID: HomieID = HomieID::new_const("max-summer-compensation");
const COMPUTED: HomieID = HomieID::new_const("result");

pub fn description() -> HomieNodeDescription {
    let mut properties = BTreeMap::new();
    let temperature_prop = PropertyDescriptionBuilder::new(HomieDataType::Integer)
        .settable(true)
        .unit(HOMIE_UNIT_DEGREE_CELSIUS)
        .build();
    properties.insert(WINTER_MAX_TEMP_ID.clone(), temperature_prop.clone());
    properties.insert(WINTER_MIN_TEMP_ID.clone(), temperature_prop.clone());
    properties.insert(SUMMER_MAX_TEMP_ID.clone(), temperature_prop.clone());
    properties.insert(SUMMER_MIN_TEMP_ID.clone(), temperature_prop.clone());
    let compensation_type_format =
        HomiePropertyFormat::Enum(CompensationType::VARIANTS.iter().copied().map(Into::into).collect());
    let compensation_type = PropertyDescriptionBuilder::new(HomieDataType::Enum)
        .format(compensation_type_format)
        .settable(true)
        .build();
    properties.insert(TYPE_ID.clone(), compensation_type);

    HomieNodeDescription {
        name: Some("outdoor temperature driven airflow speed compensation".to_string()),
        r#type: None,
        properties,
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum CompensationType {
    SafOnly,
    SafEaf,
}

// | 1251    | FAN_OUTDOOR_COMP_TYPE                                    | RW   | u16  | 1     | 0    | 1    | Compensate only SF or both SF and EF. 0=SAF, 1=SAF/EAF                                                  |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|
// | 1252    | FAN_OUTDOOR_COMP_MAX_VALUE                               | RW   | i16  | 10    | 0    | 5    | Compensation value at lowest temperature.                                                               |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|
// | 1254    | FAN_OUTDOOR_COMP_MAX_TEMP                                | RW   | i16  | 10    | -30  | 0    | Temperature at which highest compensation is applied.                                                   |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|
// | 1255    | FAN_OUTDOOR_COMP_RESULT                                  | R-   | u16  | 1     | 0    | 100  | Current outdoor compensation value                                                                      |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|
// | 1256    | FAN_OUTDOOR_COMP_START_T_WINTER                          | RW   | i16  | 10    | -30  | 0    | Temperature at which compensation starts during the winter period.                                      |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|
// | 1257    | FAN_OUTDOOR_COMP_START_T_SUMMER                          | RW   | i16  | 10    | 15   | 30   | Temperature at which compensation starts during the summer period.                                      |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|
// | 1258    | FAN_OUTDOOR_COMP_STOP_T_SUMMER                           | RW   | i16  | 10    | 15   | 40   | Temperature at which compensation reaches maximum value during the summer period.                       |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|
// | 1259    | FAN_OUTDOOR_COMP_VALUE_SUMMER                            | RW   | i16  | 10    | 0    | 5    | Compensation value during summer period                                                                 |
// |---------+----------------------------------------------------------+------+------+-------+------+------+---------------------------------------------------------------------------------------------------------|

pub fn stream(
    node_id: HomieID,
    modbus: Arc<Connection>,
) -> [std::pin::Pin<Box<dyn Stream<Item = PropertyEvent>>>; 1] {
    let start = const {
        RegisterIndex::from_name("FAN_OUTDOOR_COMP_TYPE")
            .unwrap()
            .address()
    };
    let end = const {
        RegisterIndex::from_name("FAN_OUTDOOR_COMP_VALUE_SUMMER")
            .unwrap()
            .address()
    };
    todo!()
    // let stream_levels_1 = super::modbus_read_stream_flatmap_registers(
    //     &modbus,
    //     crate::modbus::Operation::GetHoldings { address, count },
    //     Duration::from_secs(120),
    //     &node_id,
    //     LVL_REGISTERS
    //         .iter()
    //         .map(move |(r, p)| (*r, p.clone(), move |v| Box::new(AirflowLevel::new(v)) as _)),
    // );
}

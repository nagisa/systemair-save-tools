use crate::connection::Connection;
use crate::homie::node::Node;
use crate::registers::RegisterIndex;
use futures::Stream;
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID, HOMIE_UNIT_DEGREE_CELSIUS};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use strum::VariantNames as _;

use super::PropertyEvent;

const TYPE_ID: HomieID = HomieID::new_const("type");
const WINTER_COMP_ID: HomieID = HomieID::new_const("max-winter-compensation");
const WINTER_MAX_TEMP_ID: HomieID = HomieID::new_const("max-winter-compensation-temperature");
const WINTER_MIN_TEMP_ID: HomieID = HomieID::new_const("start-winter-compensation-temperature");
const SUMMER_MAX_TEMP_ID: HomieID = HomieID::new_const("max-summer-compensation-temperature");
const SUMMER_MIN_TEMP_ID: HomieID = HomieID::new_const("start-summer-compensation-temperature");
const SUMMER_COMP_ID: HomieID = HomieID::new_const("max-summer-compensation");
const COMPUTED: HomieID = HomieID::new_const("result");

pub struct CompensationNode;
impl Node for CompensationNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("compensation")
    }
    fn description(&self) -> HomieNodeDescription {
        let mut properties = BTreeMap::new();
        let temperature_prop = PropertyDescriptionBuilder::new(HomieDataType::Integer)
            .settable(true)
            .unit(HOMIE_UNIT_DEGREE_CELSIUS)
            .build();
        properties.insert(WINTER_MAX_TEMP_ID.clone(), temperature_prop.clone());
        properties.insert(WINTER_MIN_TEMP_ID.clone(), temperature_prop.clone());
        properties.insert(SUMMER_MAX_TEMP_ID.clone(), temperature_prop.clone());
        properties.insert(SUMMER_MIN_TEMP_ID.clone(), temperature_prop.clone());
        let compensation_type_format = HomiePropertyFormat::Enum(
            CompensationType::VARIANTS
                .iter()
                .copied()
                .map(Into::into)
                .collect(),
        );
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
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum CompensationType {
    SafOnly,
    SafEaf,
}

//! Exposes DEMC group of settings as a homie node.
//!
//! The CO2 and RH setpoints are exposed as settable properties with a `$target`, although for RH
//! the `winter/summer`-specific setpoints may be more interesting.
//!
//! Everything else is bog-standard boolean/integer parameters.

use super::common::{homie_enum, BooleanValue};
use super::{PropertyEvent, PropertyValue, ReadStreamError};
use crate::connection::Connection;
use crate::homie::node::Node;
use crate::homie::PropertyEventKind;
use crate::registers::{RegisterIndex, Value};
use futures::Stream;
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use strum::VariantNames;

static IS_WINTER: HomieID = HomieID::new_const("is-winter");
static CO2_ENABLED: HomieID = HomieID::new_const("co2-enabled");
static RH_ENABLED: HomieID = HomieID::new_const("rh-enabled");
static CO2: HomieID = HomieID::new_const("co2");
static RH: HomieID = HomieID::new_const("rh");
static CO2_HIGHEST: HomieID = HomieID::new_const("highest-co2");
static RH_HIGHEST: HomieID = HomieID::new_const("highest-rh");
static CO2_DEMAND: HomieID = HomieID::new_const("co2-airflow-demand");
static RH_DEMAND: HomieID = HomieID::new_const("rh-airflow-demand");
static CO2_PBAND: HomieID = HomieID::new_const("co2-pband");
static RH_PBAND: HomieID = HomieID::new_const("rh-pband");
static RH_WINTER_SETPOINT: HomieID = HomieID::new_const("rh-winter-setpoint");
static RH_SUMMER_SETPOINT: HomieID = HomieID::new_const("rh-summer-setpoint");
static IAQ_LEVEL: HomieID = HomieID::new_const("iaq-level");

pub struct DemandControlNode;

impl Node for DemandControlNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("demand-control")
    }
    fn description(&self) -> HomieNodeDescription {
        let mut properties = BTreeMap::new();
        let boolean = BooleanValue::homie_prop_builder().build();
        let settable_boolean = BooleanValue::homie_prop_builder().settable(true).build();
        let integer = PropertyDescriptionBuilder::new(HomieDataType::Integer).build();
        let settable_integer = PropertyDescriptionBuilder::new(HomieDataType::Integer)
            .settable(true)
            .build();
        properties.insert(IS_WINTER.clone(), boolean);
        properties.insert(CO2_ENABLED.clone(), settable_boolean.clone());
        properties.insert(RH_ENABLED.clone(), settable_boolean.clone());
        properties.insert(CO2.clone(), settable_integer.clone());
        properties.insert(RH.clone(), settable_integer.clone());
        properties.insert(CO2_HIGHEST.clone(), integer.clone());
        properties.insert(RH_HIGHEST.clone(), integer.clone());
        properties.insert(CO2_DEMAND.clone(), integer.clone());
        properties.insert(RH_DEMAND.clone(), integer.clone());
        properties.insert(CO2_PBAND.clone(), settable_integer.clone());
        properties.insert(RH_PBAND.clone(), settable_integer.clone());
        properties.insert(RH_WINTER_SETPOINT.clone(), settable_integer.clone());
        properties.insert(RH_SUMMER_SETPOINT.clone(), settable_integer.clone());
        properties.insert(IAQ_LEVEL.clone(), homie_enum::<IaqValue>().build());
        HomieNodeDescription {
            name: Some("demand control settings".to_string()),
            r#type: None,
            properties,
        }
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum IaqValue {
    Economic,
    Good,
    Improving,
}

impl IaqValue {
    fn new(value: Value) -> Self {
        Self::from_repr(value.into_inner()).expect("TODO")
    }
}

impl PropertyValue for IaqValue {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

struct PropertyWithSetpoint {
    current: Value,
    setpoint: Value,
}

impl PropertyValue for PropertyWithSetpoint {
    fn value(&self) -> String {
        self.current.to_string()
    }

    fn target(&self) -> Option<String> {
        Some(self.setpoint.to_string())
    }
}

const START_ADDRESS: u16 = 1001;
const REGISTER_COUNT: u16 = 1045 - START_ADDRESS;

fn boolean_property(
    node_id: &HomieID,
    prop_id: &HomieID,
    register_address: u16,
    response: &Result<crate::modbus::Response, Arc<ReadStreamError>>,
) -> PropertyEvent {
    let kind = PropertyEventKind::from_holdings_response(&response, |vs| {
        let Some(value) = extract_value(START_ADDRESS, register_address, vs) else {
            panic!("decoding boolean properties should always succeed");
        };
        BooleanValue::from(value)
    });
    PropertyEvent {
        node_id: node_id.clone(),
        property_name: prop_id.clone(),
        kind,
    }
}

fn extract_value(base: u16, value_address: u16, response: &[u8]) -> Option<Value> {
    let value_register = RegisterIndex::from_address(value_address).unwrap();
    let value_offset = 2 * usize::from(value_address - base);
    let value_data_type = value_register.data_type();
    value_data_type
        .from_bytes(&response[value_offset..][..value_data_type.bytes()])
        .next()
}

fn property_with_setpoint(
    node_id: &HomieID,
    prop_id: &HomieID,
    value_address: u16,
    setpoint_address: u16,
    response: &Result<crate::modbus::Response, Arc<ReadStreamError>>,
) -> PropertyEvent {
    let kind = PropertyEventKind::from_holdings_response(&response, |vs| {
        let Some(value) = extract_value(START_ADDRESS, value_address, vs) else {
            panic!("decoding setpoint properties should always succeed");
        };
        let Some(setpoint) = extract_value(START_ADDRESS, setpoint_address, vs) else {
            panic!("decoding setpoint properties should always succeed");
        };
        PropertyWithSetpoint {
            current: value,
            setpoint,
        }
    });
    PropertyEvent {
        node_id: node_id.clone(),
        property_name: prop_id.clone(),
        kind,
    }
}

fn simple_property(
    node_id: &HomieID,
    prop_id: &HomieID,
    value_address: u16,
    response: &Result<crate::modbus::Response, Arc<ReadStreamError>>,
) -> PropertyEvent {
    let kind = PropertyEventKind::from_holdings_response(&response, |vs| {
        let Some(value) = extract_value(START_ADDRESS, value_address, vs) else {
            panic!("decoding setpoint properties should always succeed");
        };
        super::common::SimpleValue(value)
    });
    PropertyEvent {
        node_id: node_id.clone(),
        property_name: prop_id.clone(),
        kind,
    }
}

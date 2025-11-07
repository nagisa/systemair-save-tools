//! Exposes DEMC group of settings as a homie node.
//!
//! The CO2 and RH setpoints are exposed as settable properties with a `$target`, although for RH
//! the `winter/summer`-specific setpoints may be more interesting.
//!
//! Everything else is bog-standard boolean/integer parameters.
use super::value::{homie_enum, BooleanValue};
use super::PropertyValue;
use crate::homie::value::{adjust_for_register, string_enum, PropertyDescription, UintValue};
use crate::homie::node::{Node, NodeEvent, PropertyRegisterEntry};
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::HomieNodeDescription;
use homie5::HomieID;
use std::collections::BTreeMap;
use tokio::sync::broadcast::Sender;

static REGISTERS: [PropertyRegisterEntry; 17] = super::node::property_registers![
    (1001 is "highest-rh-sensor": UintValue),
    (1002 is "highest-co2-sensor": UintValue),
    (1011 is "current-rh-setpoint": UintValue),
    (1012 is "current-rh": UintValue),
    (1019 is "current-rh-airflow-demand": UintValue),
    (1021 is "current-co2-setpoint": UintValue),
    (1022 is "current-co2": UintValue),
    (1029 is "current-co2-airflow-demand": UintValue),
    (1031 is "rh-pband": UintValue),
    (1033 is "rh-summer-setpoint": UintValue),
    (1034 is "rh-winter-setpoint": UintValue),
    (1035 is "rh-enabled": BooleanValue),
    (1039 is "season": Season),
    (1041 is "co2-pband": UintValue),
    (1043 is "co2-setpoint": UintValue),
    (1044 is "co2-enabled": BooleanValue),
    (1123 is "current-indoor-air-quality-level": IaqLevel),
];

pub struct DemandControlNode {
    device_values: [Option<Value>; REGISTERS.len()],
    sender: Sender<NodeEvent>,
}

impl DemandControlNode {
    pub(crate) fn new(sender: Sender<NodeEvent>) -> Self {
        Self {
            device_values: [None; _],
            sender,
        }
    }
}

impl Node for DemandControlNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("demand-control")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = REGISTERS
            .iter()
            .map(|prop| {
                let mut description = (prop.mk_description)();
                adjust_for_register(&mut description, prop.register);
                (prop.prop_id.clone(), description)
            })
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("demand control settings and status".to_string()),
            r#type: None,
            properties,
        }
    }
    fn registers(&self) -> &'static [PropertyRegisterEntry] {
        &REGISTERS
    }
    fn record_register_value(&mut self, index: usize, value: Value) -> Option<Option<Value>> {
        let old_value = self.device_values[index];
        if old_value == Some(value) {
            return None;
        }
        self.device_values[index] = Some(value);
        return Some(old_value);
    }
    fn broadcast_node_event(&self, node_event: NodeEvent) {
        let _ignore_no_receivers = self.sender.send(node_event);
    }
    fn values_populated(&self) -> bool {
        self.device_values.iter().all(|v| v.is_some())
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum Season {
        Summer = 0,
        Winter = 1,
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum IaqLevel {
        Economic = 0,
        Good = 1,
        Improving = 2,
    }
}

use crate::homie::common::{
    adjust_for_register, homie_enum, string_enum, CelsiusValue, PropertyDescription, PropertyValue, UintValue
};
use crate::homie::node::{Node, NodeEvent, PropertyRegisterEntry};
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::HomieNodeDescription;
use homie5::HomieID;
use std::collections::BTreeMap;
use tokio::sync::broadcast::Sender;

static REGISTERS: [PropertyRegisterEntry; 8] = super::node::property_registers![
    (1251 is "type": CompensationType),
    (1252 is "max-when-winter": UintValue),
    (1254 is "max-when-winter-outdoor-temperature": CelsiusValue),
    (1255 is "current": UintValue),
    (1256 is "start-when-winter-outdoor-temperature": CelsiusValue),
    (1257 is "start-when-summer-outdoor-temperature": CelsiusValue),
    (1258 is "max-when-summer-outdoor-temperature": CelsiusValue),
    (1259 is "max-when-summer": UintValue),
];

pub struct CompensationNode {
    device_values: [Option<Value>; REGISTERS.len()],
    sender: Sender<NodeEvent>,
}

impl CompensationNode {
    pub(crate) fn new(sender: Sender<NodeEvent>) -> Self {
        Self {
            device_values: [None; _],
            sender,
        }
    }
}

impl Node for CompensationNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("compensation")
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
            name: Some("outdoor temperature driven airflow speed compensation".to_string()),
            r#type: None,
            properties,
        }
    }

    fn broadcast_node_event(&self, node_event: super::node::NodeEvent) {
        let _ignore_no_receivers = self.sender.send(node_event);
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

    fn values_populated(&self) -> bool {
        self.device_values.iter().all(|v| v.is_some())
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum CompensationType {
        SafOnly = 0,
        SafEaf = 1,
    }
}

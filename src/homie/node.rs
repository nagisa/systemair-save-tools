use crate::homie::common::PropertyValue;
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyDescription, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

pub trait Node {
    /// The ID for this homie node.
    fn node_id(&self) -> HomieID;
    /// The Homie description for the node.
    fn description(&self) -> HomieNodeDescription;

    fn on_register_value(&mut self, register: RegisterIndex, value: Value);

    fn node_events(&self) -> Receiver<NodeEvent>;

    fn values_populated(&self) -> bool;
}

#[derive(Clone)]
pub enum NodeEvent {
    PropertyChanged {
        node_id: HomieID,
        prop_id: HomieID,
        new: Arc<dyn PropertyValue>,
    },
    TargetChanged {
        node_id: HomieID,
        prop_id: HomieID,
        new: Arc<dyn PropertyValue>,
    },
}

pub(crate) struct PropertyRegisterEntry {
    pub register: RegisterIndex,
    pub prop_id: HomieID,
    pub mk_description: fn() -> HomiePropertyDescription,
    pub from_value: fn(Value) -> Result<Arc<dyn Send + Sync + PropertyValue>, ()>,
}

macro_rules! property_registers {
    ($(($i: literal is $n: literal: $ty: ty),)*) => {
        const {
            [$(PropertyRegisterEntry {
                register: RegisterIndex::from_address($i).unwrap(),
                prop_id: HomieID::new_const($n),
                mk_description: <$ty as PropertyDescription>::description,
                from_value: |v| {
                    let v = <$ty as TryFrom<Value>>::try_from(v)?;
                    Ok(Arc::new(v) as Arc<dyn Send + Sync + PropertyValue>)
                }
            },)*]
        }
    }
}

pub(crate) use property_registers;

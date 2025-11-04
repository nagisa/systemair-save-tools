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
    fn broadcast_node_event(&self, node_event: NodeEvent);
    fn registers(&self) -> &'static [PropertyRegisterEntry];
    // Should return Some of the previous value if the value has changed.
    //
    // Index is the index into the `registers` slice.
    fn record_register_value(&mut self, index: usize, value: Value) -> Option<Option<Value>>;

    fn on_register_value(&mut self, register: RegisterIndex, value: Value) {
        let registers = self.registers();
        let Ok(idx) = registers.binary_search_by_key(&register, |v| v.register) else {
            return;
        };
        let Some(old_value) = self.record_register_value(idx, value) else {
            return;
        };
        let PropertyRegisterEntry {
            prop_id,
            from_value,
            ..
        } = &registers[idx];
        let old_value = old_value.map(from_value);
        let new_value = from_value(value);
        let (tgt_changed, val_changed, new) = match (old_value, new_value) {
            (None | Some(Err(_)) | Some(Ok(_)), Err(_)) => {
                tracing::debug!(
                    ?value,
                    address = register.address(),
                    ?prop_id,
                    "could not parse value from device"
                );
                return;
            }
            (None | Some(Err(_)), Ok(new)) => (new.has_target(), true, new),
            (Some(Ok(old)), Ok(new)) => (
                new.has_target() && old.target() != new.target(),
                old.value() != new.value(),
                new,
            ),
        };
        if tgt_changed {
            self.broadcast_node_event(NodeEvent::TargetChanged {
                node_id: self.node_id(),
                prop_id: prop_id.clone(),
                new: Arc::clone(&new) as _,
            });
        }
        if val_changed {
            self.broadcast_node_event(NodeEvent::PropertyChanged {
                node_id: self.node_id(),
                prop_id: prop_id.clone(),
                new,
            });
        }
    }

    fn property_by_name(&self, prop_id: &HomieID) -> Option<(usize, &PropertyRegisterEntry)> {
        let registers = self.registers();
        registers.iter().enumerate().find(|(_, v)| &v.prop_id == prop_id)
    }

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
    pub from_str: fn(&str) -> Result<Arc<dyn Send + Sync + PropertyValue>, ()>,
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
                },
                from_str: |v| {
                    let v = <$ty as TryFrom<&str>>::try_from(v).map_err(|_| todo!())?;
                    Ok(Arc::new(v) as Arc<dyn Send + Sync + PropertyValue>)
                },
            },)*]
        }
    }
}

pub(crate) use property_registers;

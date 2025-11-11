use crate::connection::Connection;
use crate::homie::value::{
    ActionPropertyValue, DynPropertyValue, PropertyValue, RegisterPropertyValue,
};
use crate::homie::{EventResult, ModbusStream};
use crate::modbus;
use crate::modbus_device_cache::ModbusDeviceValues;
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    FloatRange, HomieNodeDescription, HomiePropertyDescription, IntegerRange,
};
use homie5::{HomieDataType, HomieID};
use std::pin::Pin;
use std::sync::Arc;

pub trait Node {
    /// The ID for this homie node.
    fn node_id(&self) -> HomieID;
    /// The Homie description for the node.
    fn description(&self) -> HomieNodeDescription;
    fn properties(&self) -> &[PropertyEntry];
}

pub(crate) trait PropertyKind: Send + Sync {
    fn registers(&self) -> &[RegisterIndex];
    fn value_from_modbus(
        &self,
        modbus: &ModbusDeviceValues,
    ) -> Option<Result<Box<DynPropertyValue>, ()>>;
    fn value_from_homie(&self, mqtt: &str) -> Result<Box<DynPropertyValue>, ()>;
    fn homie_set_to_modbus(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: Arc<Connection>,
        device_id: u8,
        value: Box<DynPropertyValue>,
    ) -> Pin<Box<ModbusStream>>;
    fn adjust_description(&self, description: &mut HomiePropertyDescription);
}

pub(crate) struct RegisterPropertyKind<T> {
    pub register: RegisterIndex,
    pub _phantom: std::marker::PhantomData<T>,
}

impl<T> PropertyKind for RegisterPropertyKind<T>
where
    T: PropertyValue + RegisterPropertyValue + 'static,
    T: TryFrom<Value> + for<'a> TryFrom<&'a str>,
{
    fn registers(&self) -> &[RegisterIndex] {
        std::slice::from_ref(&self.register)
    }

    fn value_from_modbus(
        &self,
        modbus: &ModbusDeviceValues,
    ) -> Option<Result<Box<DynPropertyValue>, ()>> {
        let modbus_value = modbus.value_of(self.register)?;
        let cvt = <T as TryFrom<Value>>::try_from(modbus_value);
        Some(cvt.map(|v| Box::new(v) as _).map_err(|_| ()))
    }

    fn value_from_homie(&self, mqtt: &str) -> Result<Box<DynPropertyValue>, ()> {
        let v = <T as TryFrom<&str>>::try_from(mqtt).map_err(|_| ())?;
        Ok(Box::new(v) as Box<DynPropertyValue>)
    }

    fn homie_set_to_modbus(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: Arc<Connection>,
        device_id: u8,
        value: Box<DynPropertyValue>,
    ) -> Pin<Box<ModbusStream>> {
        tracing::warn!("here?!");
        let address = self.register.address();
        let value = (value as Box<dyn std::any::Any>)
            .downcast::<T>()
            .expect("type confusion");
        let value = value.to_modbus();
        Box::pin(futures::stream::once(async move {
            tracing::warn!("setting!");
            let operation = modbus::Operation::SetHolding { address, value };
            let request = modbus::Request {
                device_id,
                transaction_id: modbus.new_transaction_id(),
                operation: operation.clone(),
            };
            let response = modbus.send_retrying(request).await?;
            if response.exception_code().is_some() {
                return Ok(EventResult::HomieSet {
                    node_id,
                    prop_idx,
                    operation: operation.clone(),
                    response: response.kind,
                });
            }
            let operation = modbus::Operation::GetHoldings { address, count: 1 };
            let request = modbus::Request {
                device_id,
                transaction_id: modbus.new_transaction_id(),
                operation,
            };
            let response = modbus.send_retrying(request).await?;
            return Ok(EventResult::HomieSet {
                node_id,
                prop_idx,
                operation,
                response: response.kind,
            });
        })) as _
    }

    fn adjust_description(&self, description: &mut HomiePropertyDescription) {
        let register = self.register;
        description.settable = register.mode().is_writable();
        description.retained = true;
        let min = register.minimum_value().map(|v| match v {
            Value::U16(v) => i64::from(v),
            Value::I16(v) => i64::from(v),
            Value::Celsius(v) => i64::from(v),
            Value::SpecificHumidity(v) => i64::from(v),
        });
        let max = register.maximum_value().map(|v| match v {
            Value::U16(v) => i64::from(v),
            Value::I16(v) => i64::from(v),
            Value::Celsius(v) => i64::from(v),
            Value::SpecificHumidity(v) => i64::from(v),
        });
        'no_format: {
            description.format = match (description.datatype, min, max) {
                (HomieDataType::Boolean, Some(min), Some(max)) => {
                    assert_eq!((min, max), (0, 1), "{} is not bool", register.address());
                    break 'no_format;
                }
                (HomieDataType::Integer, min, max) => IntegerRange {
                    min,
                    max,
                    step: None,
                }
                .into(),
                (HomieDataType::Float, min, max) => FloatRange {
                    min: min.map(|v| v as f64 / register.data_type().scale() as f64),
                    max: max.map(|v| v as f64 / register.data_type().scale() as f64),
                    step: Some(1.0f64 / register.data_type().scale() as f64),
                }
                .into(),
                _ => break 'no_format,
            }
        }
    }
}

pub(crate) struct AggregatePropertyKind<T> {
    pub registers: &'static [RegisterIndex],
    // Macro-generated constructor that delegates to `::new()`.
    pub from_modbus: fn(&ModbusDeviceValues) -> Option<Result<Box<DynPropertyValue>, ()>>,
    pub _phantom: std::marker::PhantomData<T>,
}

impl<T> PropertyKind for AggregatePropertyKind<T>
where
    T: PropertyValue + 'static,
    T: for<'a> TryFrom<&'a str>,
{
    fn registers(&self) -> &[RegisterIndex] {
        self.registers
    }

    fn value_from_modbus(
        &self,
        modbus: &ModbusDeviceValues,
    ) -> Option<Result<Box<DynPropertyValue>, ()>> {
        (self.from_modbus)(modbus)
    }

    fn value_from_homie(&self, mqtt: &str) -> Result<Box<DynPropertyValue>, ()> {
        let v = <T as TryFrom<&str>>::try_from(mqtt).map_err(|_| ())?;
        Ok(Box::new(v) as Box<DynPropertyValue>)
    }

    fn homie_set_to_modbus(
        &self,
        _node_id: HomieID,
        _prop_idx: usize,
        _modbus: Arc<Connection>,
        _device_id: u8,
        _value: Box<DynPropertyValue>,
    ) -> Pin<Box<ModbusStream>> {
        todo!()
    }

    fn adjust_description(&self, description: &mut HomiePropertyDescription) {
        description.retained = true;
    }
}

pub(crate) struct ActionPropertyKind<T> {
    pub _phantom: std::marker::PhantomData<T>,
}

impl<T> PropertyKind for ActionPropertyKind<T>
where
    T: PropertyValue + ActionPropertyValue + 'static,
    T: for<'a> TryFrom<&'a str>,
{
    fn registers(&self) -> &[RegisterIndex] {
        &[]
    }

    fn value_from_modbus(
        &self,
        _: &ModbusDeviceValues,
    ) -> Option<Result<Box<DynPropertyValue>, ()>> {
        None
    }

    fn value_from_homie(&self, mqtt: &str) -> Result<Box<DynPropertyValue>, ()> {
        let v = <T as TryFrom<&str>>::try_from(mqtt).map_err(|_| ())?;
        Ok(Box::new(v) as Box<DynPropertyValue>)
    }

    fn homie_set_to_modbus(
        &self,
        _node_id: HomieID,
        _prop_idx: usize,
        _modbus: Arc<Connection>,
        _device_id: u8,
        value: Box<DynPropertyValue>,
    ) -> Pin<Box<ModbusStream>> {
        let value = (value as Box<dyn std::any::Any>)
            .downcast::<T>()
            .expect("type confusion");
        value.invoke()
    }

    fn adjust_description(&self, description: &mut HomiePropertyDescription) {
        description.retained = false;
        description.settable = true;
    }
}

pub(crate) struct PropertyEntry {
    pub prop_id: HomieID,
    pub mk_description: fn(&PropertyEntry) -> HomiePropertyDescription,
    pub kind: &'static dyn PropertyKind,
}

impl PropertyEntry {
    pub fn description(&self) -> HomiePropertyDescription {
        let mut initial = (self.mk_description)(&self);
        self.kind.adjust_description(&mut initial);
        initial
    }
}

macro_rules! properties {
    (static $static:ident = [$( { $prop_id:literal: $value_type:ty = $($tokens:tt)* }, )*]) => {
        static $static: [PropertyEntry; 0 $(+ $crate::homie::node::properties!(@one $prop_id))*] = [
            $($crate::homie::node::properties!(@property $prop_id: $value_type = $($tokens)*)),*
        ];
    };

    (@one $prop_id: literal) => { 1 };

    (@property $prop_id:literal: $value_type:ty = register $name:literal) => {
        PropertyEntry {
            prop_id: HomieID::new_const($prop_id),
            mk_description: <$value_type as $crate::homie::value::PropertyDescription>::description,
            kind: &$crate::homie::node::RegisterPropertyKind::<$value_type> {
                register: $crate::registers::RegisterIndex::from_name($name).unwrap(),
                _phantom: std::marker::PhantomData
            }
        }
    };
    (@property $prop_id:literal: $value_type:ty = action) => {
        PropertyEntry {
            prop_id: HomieID::new_const($prop_id),
            mk_description: <$value_type as $crate::homie::value::PropertyDescription>::description,
            kind: &$crate::homie::node::ActionPropertyKind::<$value_type> {
                _phantom: std::marker::PhantomData
            },
        }
    };
    (@property $prop_id:literal: $value_type:ty = aggregate $($register:literal),+) => {
        PropertyEntry {
            prop_id: HomieID::new_const($prop_id),
            mk_description: <$value_type as $crate::homie::value::PropertyDescription>::description,
            kind: &$crate::homie::node::AggregatePropertyKind::<$value_type> {
                registers: &[$(RegisterIndex::from_name($register).unwrap()),*],
                from_modbus: |modbus| {
                    let result = <$value_type>::new($(
                        modbus.value_of(const { RegisterIndex::from_name($register).unwrap() })?
                    ),*);
                    Some(result.map(|v| Box::new(v) as _))
                },
                _phantom: std::marker::PhantomData,
            }
        }
    };
}

pub(crate) use properties;

use crate::homie::value::{DynPropertyValue, PropertyValue};
use crate::modbus_device_cache::ModbusDeviceValues;
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    FloatRange, HomieNodeDescription, HomiePropertyDescription, IntegerRange,
};
use homie5::{HomieDataType, HomieID};

pub trait Node {
    /// The ID for this homie node.
    fn node_id(&self) -> HomieID;
    /// The Homie description for the node.
    fn description(&self) -> HomieNodeDescription;
    fn properties(&self) -> &[PropertyEntry];
    fn property_value(&self, property_index: usize) -> Option<&DynPropertyValue>;
    fn set_property_value(
        &mut self,
        property_index: usize,
        value: Box<DynPropertyValue>,
    ) -> Option<Box<DynPropertyValue>>;
}

pub(crate) enum PropertyKind {
    /// This property is a 1:1 mapping to a modbus register.
    Register {
        register: RegisterIndex,
        // FIXME: error handling
        from_modbus: fn(Value) -> Result<Box<DynPropertyValue>, ()>,
        // FIXME: error handling
        from_homie: fn(&str) -> Result<Box<DynPropertyValue>, ()>,
    },
    /// An action that node implements custom handling logic for.
    Action {
        from_homie: fn(&str) -> Result<Box<DynPropertyValue>, ()>,
    },
    Aggregate {
        registers: &'static [RegisterIndex],
        from_modbus: fn(&ModbusDeviceValues) -> Option<Result<Box<DynPropertyValue>, ()>>,
        from_homie: fn(&str) -> Result<Box<DynPropertyValue>, ()>,
    },
}

impl PropertyKind {
    pub(crate) const fn new_register<T, E>(name: &str) -> Self
    where
        T: PropertyValue + 'static,
        T: TryFrom<Value, Error = ()>,
        T: for<'a> TryFrom<&'a str, Error = E>,
    {
        Self::Register {
            register: RegisterIndex::from_name(name).expect("invalid register name"),
            from_modbus: |v| {
                let v = <T as TryFrom<Value>>::try_from(v)?;
                Ok(Box::new(v) as Box<DynPropertyValue>)
            },
            from_homie: |v| {
                let v = <T as TryFrom<&str>>::try_from(v).map_err(|_| ())?;
                Ok(Box::new(v) as Box<DynPropertyValue>)
            },
        }
    }

    /// All registers this property is interested in.
    pub(crate) fn registers(&self) -> Box<dyn Iterator<Item = RegisterIndex>> {
        match self {
            PropertyKind::Register { register, .. } => Box::new(std::iter::once(*register)),
            PropertyKind::Action { .. } => Box::new(std::iter::empty()),
            PropertyKind::Aggregate { registers, .. } => Box::new(registers.iter().copied()),
        }
    }

    pub(crate) fn value_from_modbus(
        &self,
        modbus: &ModbusDeviceValues,
    ) -> Option<Result<Box<DynPropertyValue>, ()>> {
        match self {
            PropertyKind::Register {
                register,
                from_modbus,
                ..
            } => {
                let modbus_value = modbus.value_of(*register)?;
                Some(from_modbus(modbus_value))
            }
            PropertyKind::Action { .. } => None,
            PropertyKind::Aggregate { from_modbus, .. } => from_modbus(modbus),
        }
    }

    pub(crate) fn value_from_homie(&self, mqtt: &str) -> Result<Box<DynPropertyValue>, ()> {
        match self {
            PropertyKind::Register { from_homie, .. }
            | PropertyKind::Action { from_homie }
            | PropertyKind::Aggregate { from_homie, .. } => from_homie(mqtt),
        }
    }
}

pub(crate) struct PropertyEntry {
    pub prop_id: HomieID,
    pub mk_description: fn(&PropertyEntry) -> HomiePropertyDescription,
    pub kind: PropertyKind,
}

impl PropertyEntry {
    pub fn description(&self) -> HomiePropertyDescription {
        let mut initial = (self.mk_description)(&self);
        match self.kind {
            PropertyKind::Action { .. } => {
                initial.retained = false;
                initial.settable = true;
            }
            PropertyKind::Aggregate { .. } => {
                // Aggregate values decide for themselves if they're writable or not.
                initial.retained = true;
            }
            PropertyKind::Register { register, .. } => {
                initial.settable = register.mode().is_writable();
                initial.retained = true;
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
                    (&mut initial).format = match (initial.datatype, min, max) {
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
            kind: $crate::homie::node::PropertyKind::new_register::<$value_type, _>($name)
        }
    };
    (@property $prop_id:literal: $value_type:ty = action) => {
        PropertyEntry {
            prop_id: HomieID::new_const($prop_id),
            mk_description: <$value_type as $crate::homie::value::PropertyDescription>::description,
            kind: $crate::homie::node::PropertyKind::Action {
                from_homie: |_v| {
                    todo!()
                }
            },
        }
    };
    (@property $prop_id:literal: $value_type:ty = aggregate $($register:literal),+) => {
        PropertyEntry {
            prop_id: HomieID::new_const($prop_id),
            mk_description: <$value_type as $crate::homie::value::PropertyDescription>::description,
            kind: $crate::homie::node::PropertyKind::Aggregate {
                registers: &[$(
                    RegisterIndex::from_name($register).expect("invalid register name")
                ),+],
                from_modbus: |modbus| {
                    let result = <$value_type>::new($(
                        modbus.value_of(const { RegisterIndex::from_name($register).unwrap() })?
                    ),*);
                    Some(result.map(|v| Box::new(v) as _))
                },
                from_homie: |_v| {
                    todo!()
                }
            },
        }
    };
}

pub(crate) use properties;

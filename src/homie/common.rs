use crate::registers::{DataType, RegisterIndex, Value};
use homie5::device_description::{
    FloatRange, HomiePropertyDescription, HomiePropertyFormat, IntegerRange,
    PropertyDescriptionBuilder,
};
use homie5::HomieDataType;

pub(crate) fn homie_enum<T: strum::VariantNames>() -> PropertyDescriptionBuilder {
    PropertyDescriptionBuilder::new(HomieDataType::Enum).format(homie_enum_format::<T>())
}

pub(crate) fn homie_enum_format<T: strum::VariantNames>() -> HomiePropertyFormat {
    HomiePropertyFormat::Enum(T::VARIANTS.iter().copied().map(Into::into).collect())
}

pub(crate) fn adjust_for_register(
    description: &mut HomiePropertyDescription,
    register: RegisterIndex,
) {
    description.settable = register.mode().is_writable();
    let min = register.minimum_value().map(|v| match v {
        Value::U16(v) => i64::from(v),
        Value::I16(v) => i64::from(v),
        Value::Celsius(v) => i64::from(v),
        Value::SpecificHumidity(v) => i64::from(v),
    });
    let max = register.minimum_value().map(|v| match v {
        Value::U16(v) => i64::from(v),
        Value::I16(v) => i64::from(v),
        Value::Celsius(v) => i64::from(v),
        Value::SpecificHumidity(v) => i64::from(v),
    });
    'no_format: {
        description.format = match (description.datatype, min, max) {
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

pub(crate) trait PropertyValue: Send + Sync {
    fn modbus(&self) -> Value;
    fn value(&self) -> String;
    fn target(&self) -> Option<String> {
        None
    }
    fn has_target(&self) -> bool {
        false
    }
}

pub(crate) trait PropertyDescription {
    fn description() -> HomiePropertyDescription;
}

pub(crate) struct BooleanValue(pub(crate) bool);
impl PropertyDescription for BooleanValue {
    fn description() -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Boolean).build()
    }
}
impl TryFrom<Value> for BooleanValue {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(BooleanValue(value.into_inner() != 0))
    }
}
impl TryFrom<&str> for BooleanValue {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(value.parse().map_err(|_| ())?))
    }
}
impl PropertyValue for BooleanValue {
    fn modbus(&self) -> Value {
        Value::U16(self.0 as u16)
    }
    fn value(&self) -> String {
        self.0.to_string()
    }
}

pub(crate) struct UintValue(pub(crate) u16);
impl TryFrom<Value> for UintValue {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(Self(value.into_inner()))
    }
}
impl TryFrom<&str> for UintValue {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(value.parse().map_err(|_| ())?))
    }
}
impl PropertyValue for UintValue {
    fn modbus(&self) -> Value {
        Value::U16(self.0)
    }
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl PropertyDescription for UintValue {
    fn description() -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Integer).build()
    }
}

pub(crate) struct CelsiusValue(pub(crate) i16);
impl TryFrom<Value> for CelsiusValue {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(Self(value.into_inner() as i16))
    }
}
impl TryFrom<&str> for CelsiusValue {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = value.parse::<f32>().map_err(|_| ())?;
        Ok(Self((value * DataType::CEL.scale() as f32).round() as _))
    }
}
impl PropertyValue for CelsiusValue {
    fn modbus(&self) -> Value {
        Value::Celsius(self.0)
    }
    fn value(&self) -> String {
        (self.0 as f32 / DataType::CEL.scale() as f32).to_string()
    }
}
impl PropertyDescription for CelsiusValue {
    fn description() -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Float)
            .unit(homie5::HOMIE_UNIT_DEGREE_CELSIUS)
            .build()
    }
}

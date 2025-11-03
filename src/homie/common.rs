use crate::registers::Value;
use homie5::device_description::{HomiePropertyFormat, PropertyDescriptionBuilder};
use homie5::HomieDataType;

pub(crate) fn homie_enum<T: strum::VariantNames>() -> PropertyDescriptionBuilder {
    PropertyDescriptionBuilder::new(HomieDataType::Enum).format(homie_enum_format::<T>())
}

pub(crate) fn homie_enum_format<T: strum::VariantNames>() -> HomiePropertyFormat {
    HomiePropertyFormat::Enum(T::VARIANTS.iter().copied().map(Into::into).collect())
}

pub(crate) trait PropertyValue: Send + Sync {
    fn value(&self) -> String;
    fn target(&self) -> Option<String>;
}

pub(crate) struct SimpleValue(pub(crate) Value);

impl PropertyValue for SimpleValue {
    fn value(&self) -> String {
        self.0.to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

pub(crate) struct BooleanValue(pub(crate) bool);

impl BooleanValue {
    pub(crate) fn homie_prop_builder() -> PropertyDescriptionBuilder {
        PropertyDescriptionBuilder::new(HomieDataType::Boolean)
    }
}

impl From<Value> for BooleanValue {
    fn from(value: Value) -> Self {
        BooleanValue(match value {
            Value::U16(v) => v != 0,
            Value::I16(v) => v != 0,
            Value::Celsius(v) => v != 0,
            Value::SpecificHumidity(v) => v != 0,
        })
    }
}

impl PropertyValue for BooleanValue {
    fn value(&self) -> String {
        self.0.to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

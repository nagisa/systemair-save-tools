use crate::registers::Value;
use homie5::device_description::{
    HomiePropertyDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::HomieDataType;

pub(crate) fn homie_enum<T: strum::VariantNames>() -> PropertyDescriptionBuilder {
    PropertyDescriptionBuilder::new(HomieDataType::Enum).format(homie_enum_format::<T>())
}

pub(crate) fn homie_enum_format<T: strum::VariantNames>() -> HomiePropertyFormat {
    HomiePropertyFormat::Enum(T::VARIANTS.iter().copied().map(Into::into).collect())
}

pub(crate) trait PropertyValue: Send + Sync {
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
impl PropertyValue for BooleanValue {
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
impl PropertyValue for UintValue {
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl PropertyDescription for UintValue {
    fn description() -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Integer).build()
    }
}

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

pub(crate) struct BooleanValue<const MUTABLE: bool>(pub(crate) bool);
impl<const MUTABLE: bool> PropertyDescription for BooleanValue<MUTABLE> {
    fn description() -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Boolean)
            .settable(MUTABLE)
            .build()
    }
}
impl<const MUTABLE: bool> TryFrom<Value> for BooleanValue<MUTABLE> {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(BooleanValue(value.into_inner() != 0))
    }
}
impl<const MUTABLE: bool> PropertyValue for BooleanValue<MUTABLE> {
    fn value(&self) -> String {
        self.0.to_string()
    }
}

pub(crate) struct UintValue<const MUTABLE: bool>(pub(crate) u16);
impl<const MUTABLE: bool> TryFrom<Value> for UintValue<MUTABLE> {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(Self(value.into_inner()))
    }
}
impl<const MUTABLE: bool> PropertyValue for UintValue<MUTABLE> {
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl<const MUTABLE: bool> PropertyDescription for UintValue<MUTABLE> {
    fn description() -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Integer)
            .settable(true)
            .build()
    }
}


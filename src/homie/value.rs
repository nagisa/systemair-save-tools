use crate::homie::node::PropertyEntry;
use crate::registers::{DataType, Value};
use homie5::device_description::{
    HomiePropertyDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::HomieDataType;

pub(crate) fn homie_enum<T: strum::VariantNames + strum::VariantArray + num_traits::ToPrimitive>(
    prop: &PropertyEntry,
) -> PropertyDescriptionBuilder {
    let names = <T as strum::VariantNames>::VARIANTS;
    let values = <T as strum::VariantArray>::VARIANTS;
    let format = match prop.kind {
        super::node::PropertyKind::Action { .. } | super::node::PropertyKind::Aggregate { .. } => {
            HomiePropertyFormat::Enum(names.iter().copied().map(Into::into).collect())
        }
        super::node::PropertyKind::Register { register, .. } => {
            let min = register.minimum_value().unwrap().into_inner();
            let max = register.maximum_value().unwrap().into_inner();
            let zip = names.iter().zip(values);
            let converted = zip.map(|(n, v)| (*n, v.to_u16().unwrap()));
            let filtered_names = converted.filter(|&(_, v)| v >= min && v <= max);
            HomiePropertyFormat::Enum(filtered_names.map(|v| v.0.into()).collect())
        }
    };
    PropertyDescriptionBuilder::new(HomieDataType::Enum).format(format)
}

pub(crate) trait PropertyValue: Send + Sync {
    fn modbus(&self) -> Value;
    fn value(&self) -> String;
    fn target(&self) -> Option<String> {
        None
    }
}

pub(crate) type DynPropertyValue = dyn Send + Sync + PropertyValue;

pub(crate) trait PropertyDescription {
    fn description(prop: &PropertyEntry) -> HomiePropertyDescription;
}

pub(crate) struct BooleanValue(pub(crate) bool);
impl PropertyDescription for BooleanValue {
    fn description(_prop: &PropertyEntry) -> HomiePropertyDescription {
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
        Value::U16(self.0 as u16)
    }
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl PropertyDescription for UintValue {
    fn description(_prop: &PropertyEntry) -> HomiePropertyDescription {
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
        Value::U16(self.0 as u16)
    }
    fn value(&self) -> String {
        (self.0 as f32 / DataType::CEL.scale() as f32).to_string()
    }
}
impl PropertyDescription for CelsiusValue {
    fn description(_prop: &PropertyEntry) -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Float)
            .unit(homie5::HOMIE_UNIT_DEGREE_CELSIUS)
            .build()
    }
}

macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident = $value:literal),* $(,)?
        }
    ) => {
        #[derive(
            strum::VariantNames,
            strum::VariantArray,
            strum::FromRepr,
            strum::IntoStaticStr,
            strum::EnumString,
            num_derive::ToPrimitive,
        )]
        #[strum(serialize_all = "kebab-case")]
        $(#[$meta])*
        $vis enum $name {
            $($variant = $value),*
        }

        impl TryFrom<Value> for $name {
            type Error = ();
            fn try_from(value: crate::registers::Value) -> Result<Self, Self::Error> {
                Self::from_repr(value.into_inner()).ok_or(())
            }
        }

        impl $crate::homie::value::PropertyValue for $name {
            fn modbus(&self) -> crate::registers::Value {
                crate::registers::Value::U16(*self as u16)
            }
            fn value(&self) -> String {
                <&'static str>::from(self).to_string()
            }
        }

        impl $crate::homie::value::PropertyDescription for $name {
            fn description(prop: &$crate::homie::node::PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
                $crate::homie::value::homie_enum::<Self>(prop).build()
            }
        }
    };
}

pub(crate) use string_enum;

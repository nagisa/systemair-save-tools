use std::any::Any;
use std::pin::Pin;
use std::sync::Arc;

use crate::connection::Connection;
use crate::homie::EventStream;
use crate::homie::node::PropertyEntry;
use crate::registers::{DataType, Value};
use homie5::device_description::{
    HomiePropertyDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};

pub(crate) fn homie_enum<T: strum::VariantNames + strum::VariantArray + num_traits::ToPrimitive>(
    prop: &PropertyEntry,
) -> PropertyDescriptionBuilder {
    let names = <T as strum::VariantNames>::VARIANTS;
    let values = <T as strum::VariantArray>::VARIANTS;
    let format = match prop.kind.registers() {
        [register] => {
            let min = register.minimum_value().unwrap().into_inner();
            let max = register.maximum_value().unwrap().into_inner();
            let zip = names.iter().zip(values);
            let converted = zip.map(|(n, v)| (*n, v.to_u16().unwrap()));
            let filtered_names = converted.filter(|&(_, v)| v >= min && v <= max);
            HomiePropertyFormat::Enum(filtered_names.map(|v| v.0.into()).collect())
        }
        _ => HomiePropertyFormat::Enum(names.iter().copied().map(Into::into).collect()),
    };
    PropertyDescriptionBuilder::new(HomieDataType::Enum).format(format)
}

pub(crate) trait PropertyValue: Any + Send + Sync {
    fn value(&self) -> String;
    fn target(&self) -> Option<String> {
        None
    }
    fn on_property_change(
        &self,
        _node_id: HomieID,
        _prop_idx: usize,
        _modbus: Arc<Connection>,
    ) -> Pin<Box<EventStream>> {
        Box::pin(futures::stream::empty())
    }
}

pub(crate) type DynPropertyValue = dyn Send + Sync + PropertyValue;

/// [`PropertyValue`] types that directly correspond to a modbus register.
///
/// This means that these can directly produce a `Value` instead of more complicated concepts when
/// writeback of them is initiated.
pub(crate) trait RegisterPropertyValue {
    fn to_modbus(&self) -> u16;
}

pub(crate) trait ActionPropertyValue {
    fn invoke(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: Arc<Connection>,
    ) -> Pin<Box<EventStream>>;
}

pub(crate) trait AggregatePropertyValue {
    const SETTABLE: bool;
    fn set(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: Arc<Connection>,
    ) -> Pin<Box<EventStream>>;
}

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
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl RegisterPropertyValue for BooleanValue {
    fn to_modbus(&self) -> u16 {
        self.0 as u16
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
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl PropertyDescription for UintValue {
    fn description(_prop: &PropertyEntry) -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Integer).build()
    }
}
impl RegisterPropertyValue for UintValue {
    fn to_modbus(&self) -> u16 {
        self.0
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
    fn value(&self) -> String {
        (self.0 as f32 / DataType::CEL.scale() as f32).to_string()
    }
}
impl RegisterPropertyValue for CelsiusValue {
    fn to_modbus(&self) -> u16 {
        self.0 as u16
    }
}
impl PropertyDescription for CelsiusValue {
    fn description(_prop: &PropertyEntry) -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Float)
            .unit(homie5::HOMIE_UNIT_DEGREE_CELSIUS)
            .build()
    }
}

pub(crate) struct SpcHumidityValue(u16);
impl TryFrom<Value> for SpcHumidityValue {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(Self(value.into_inner()))
    }
}
impl TryFrom<&str> for SpcHumidityValue {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let value = value.parse::<f32>().map_err(|_| ())?;
        Ok(Self((value * DataType::SPH.scale() as f32).round() as _))
    }
}
impl PropertyValue for SpcHumidityValue {
    fn value(&self) -> String {
        (self.0 as f32 / DataType::SPH.scale() as f32).to_string()
    }
}
impl RegisterPropertyValue for SpcHumidityValue {
    fn to_modbus(&self) -> u16 {
        self.0 as u16
    }
}
impl PropertyDescription for SpcHumidityValue {
    fn description(_prop: &PropertyEntry) -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Float)
            .unit("g/kg")
            .build()
    }
}

pub(crate) struct StopDelay(u16);
impl PropertyValue for StopDelay {
    fn value(&self) -> String {
        format!("PT{}M", self.0)
    }
}
impl PropertyDescription for StopDelay {
    fn description(_prop: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(homie5::HomieDataType::Duration).build()
    }
}
impl RegisterPropertyValue for StopDelay {
    fn to_modbus(&self) -> u16 {
        self.0
    }
}
impl TryFrom<&str> for StopDelay {
    type Error = jiff::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let span = value.parse::<jiff::Span>()?;
        let minutes = span.total(jiff::Unit::Minute)?;
        if minutes < 0.0 || minutes > 20.0 {
            return Err(jiff::Error::from_args(format_args!(
                "pump stop delay out of range"
            )));
        }
        Ok(Self(minutes.round() as u16))
    }
}
impl From<Value> for StopDelay {
    fn from(value: Value) -> Self {
        Self(value.into_inner())
    }
}

pub(crate) struct RemainingTimeValue(jiff::Span);
impl RemainingTimeValue {
    pub(crate) fn new(l: Value, h: Value) -> Result<Self, ()> {
        let seconds_remaining = u32::from(h.into_inner()) << 16 | u32::from(l.into_inner());
        let span = jiff::Span::new().seconds(seconds_remaining);
        let now = jiff::Zoned::now();
        let round_cfg = jiff::SpanRound::new()
            .largest(jiff::Unit::Month)
            .relative(&now);
        Ok(Self(span.round(round_cfg).map_err(|_| ())?))
    }
}
impl PropertyValue for RemainingTimeValue {
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl AggregatePropertyValue for RemainingTimeValue {
    const SETTABLE: bool = false;
    fn set(
        &self,
        _: HomieID,
        _: usize,
        _: Arc<Connection>,
    ) -> std::pin::Pin<Box<super::EventStream>> {
        unreachable!("remaining time is computed and not settable");
    }
}
impl PropertyDescription for RemainingTimeValue {
    fn description(_: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Duration).build()
    }
}
// TODO: can we avoid unnecessary implementations like these?
impl TryFrom<&str> for RemainingTimeValue {
    type Error = ();
    fn try_from(_: &str) -> Result<Self, Self::Error> {
        unreachable!()
    }
}

macro_rules! string_enum {
    (
        #[impl($($impl:ident),*)]
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

        $($crate::homie::value::string_enum!(@impl $impl for $name);)*
    };
    (@impl TryFromValue for $name:ident) => {
        impl TryFrom<Value> for $name {
            type Error = ();
            fn try_from(value: crate::registers::Value) -> Result<Self, Self::Error> {
                Self::from_repr(value.into_inner()).ok_or(())
            }
        }
    };
    (@impl PropertyValue for $name:ident) => {
        impl $crate::homie::value::PropertyValue for $name {
            fn value(&self) -> String {
                <&'static str>::from(self).to_string()
            }
        }
    };
    (@impl RegisterPropertyValue for $name:ident) => {
        impl $crate::homie::value::RegisterPropertyValue for $name {
            fn to_modbus(&self) -> u16 {
                *self as u16
            }
        }
    };
    (@impl PropertyDescription for $name:ident) => {
        impl $crate::homie::value::PropertyDescription for $name {
            fn description(prop: &$crate::homie::node::PropertyEntry)
            -> homie5::device_description::HomiePropertyDescription {
                $crate::homie::value::homie_enum::<Self>(prop).build()
            }
        }
    };
}

pub(crate) use string_enum;

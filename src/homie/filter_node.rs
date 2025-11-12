use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    ActionPropertyValue, BooleanValue, PropertyDescription, PropertyValue, RegisterPropertyValue,
    RemainingTimeValue,
};
use crate::homie::EventResult;
use crate::modbus;
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
    { "replacement-period": ReplacementPeriod = register "FILTER_PERIOD" },
    { "remaining-time": RemainingTimeValue = aggregate
        "FILTER_REMAINING_TIME_L", "FILTER_REMAINING_TIME_H"
    },
    { "should-replace": BooleanValue = register "FILTER_ALARM_WAS_DETECTED" },
    { "replace": ReplaceAction = action },
] }

pub struct FilterNode {}

impl FilterNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for FilterNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("filter")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("filter state and replacement".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

struct ReplacementPeriod {
    months: u16,
}
impl PropertyValue for ReplacementPeriod {
    fn value(&self) -> String {
        format!("P{}M", self.months)
    }
}
impl PropertyDescription for ReplacementPeriod {
    fn description(_prop: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(homie5::HomieDataType::Duration).build()
    }
}
impl RegisterPropertyValue for ReplacementPeriod {
    fn to_modbus(&self) -> u16 {
        self.months as u16
    }
}
impl TryFrom<&str> for ReplacementPeriod {
    type Error = jiff::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let duration = value.parse::<jiff::Span>()?;
        let months = duration.total(jiff::Unit::Month)?;
        if months < 3.0 || months > 15.0 {
            return Err(jiff::Error::from_args(format_args!(
                "filter replacement period out of range"
            )));
        }
        Ok(Self {
            months: months.round() as u16,
        })
    }
}
impl From<Value> for ReplacementPeriod {
    fn from(value: Value) -> Self {
        Self {
            months: value.into_inner(),
        }
    }
}

#[derive(Clone, Copy)]
struct ReplaceAction;
impl PropertyValue for ReplaceAction {
    fn value(&self) -> String {
        "now".to_string()
    }
}
impl PropertyDescription for ReplaceAction {
    fn description(_: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Enum)
            .format(HomiePropertyFormat::Enum(vec!["now".into()]))
            .build()
    }
}
impl TryFrom<&str> for ReplaceAction {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        (value == "now").then_some(ReplaceAction).ok_or(())
    }
}
impl ActionPropertyValue for ReplaceAction {
    fn invoke(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: std::sync::Arc<crate::connection::Connection>,
    ) -> std::pin::Pin<Box<super::EventStream>> {
        Box::pin(async_stream::stream! {
            let register = const { RegisterIndex::from_name("FILTER_PERIOD_SET").unwrap() };
            let address = register.address();
            let operation = modbus::Operation::SetHoldings { address, values: vec![1] };
            let _response = modbus.send_retrying(operation.clone()).await?.kind;
            // SUBTLE: _response could be a server exception. We still signal to the action invoker
            // that we received the command. They will be able to tell if this had a desired effect
            // by our later reading out holding registers below.
            yield Ok(EventResult::ActionResponse {
                node_id: node_id.clone(),
                prop_idx,
                value: Box::new(Self)
            });
            let operation = modbus::Operation::GetHoldings { address, count: 3 };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            yield Ok(EventResult::HomieSet { node_id, prop_idx, operation, response });
        })
    }
}

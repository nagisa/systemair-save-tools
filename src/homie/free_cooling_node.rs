use crate::homie::EventResult;
use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    AggregatePropertyValue, BooleanValue, CelsiusValue, PropertyDescription, PropertyValue,
};
use crate::modbus;
use crate::registers::Value;
use homie5::HomieID;
use homie5::device_description::{HomieNodeDescription, PropertyDescriptionBuilder};
use std::collections::BTreeMap;

super::node::properties! { static PROPERTIES = [
     { "enabled": BooleanValue = register "FREE_COOLING_ON_OFF" },
     { "trigger-above-daytime-temperature": CelsiusValue = register "FREE_COOLING_OUTDOOR_DAYTIME_T" },
     { "stop-above-nighttime-temperature": CelsiusValue = register "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_HIGH_T_LIMIT" },
     { "stop-below-nighttime-temperature": CelsiusValue = register "FREE_COOLING_OUTDOOR_NIGHTTIME_DEACTIVATION_LOW_T_LIMIT" },
     { "stop-below-room-temperature": CelsiusValue = register "FREE_COOLING_ROOM_CANCEL_T" },
     { "start-time": TimeValue = aggregate "FREE_COOLING_START_TIME_H", "FREE_COOLING_START_TIME_M" },
     { "end-time": TimeValue = aggregate "FREE_COOLING_END_TIME_H", "FREE_COOLING_END_TIME_M" },
     { "active": BooleanValue = register "FREE_COOLING_ACTIVE" },
] }

pub struct FreeCoolingNode {}

impl FreeCoolingNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for FreeCoolingNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("free-cooling")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("indoor cooling using cool night air".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

struct TimeValue(jiff::civil::Time);
impl TimeValue {
    fn new(hour: Value, minute: Value) -> Result<Self, ()> {
        Ok(Self(jiff::civil::time(
            hour.into_inner() as _,
            minute.into_inner() as _,
            0,
            0,
        )))
    }
}
impl PropertyValue for TimeValue {
    fn value(&self) -> String {
        format!("T{}", self.0)
    }
}
impl AggregatePropertyValue for TimeValue {
    const SETTABLE: bool = true;
    fn set(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: std::sync::Arc<crate::connection::Connection>,
    ) -> std::pin::Pin<Box<super::ModbusStream>> {
        let register = PROPERTIES[prop_idx].kind.registers()[0];
        let address = register.address();
        let values = vec![self.0.hour() as u16, self.0.minute() as u16];
        Box::pin(async_stream::stream! {
            let operation = modbus::Operation::SetHoldings { address, values };
            let response = modbus.send_retrying(operation.clone()).await?;
            if response.exception_code().is_some() {
                yield Ok(EventResult::HomieSet {
                    node_id: node_id.clone(),
                    prop_idx,
                    operation: operation,
                    response: response.kind,
                });
            }
            let operation = modbus::Operation::GetHoldings { address, count: 2 };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            yield Ok(EventResult::HomieSet { node_id, prop_idx, operation, response });
        })
    }
}

impl PropertyDescription for TimeValue {
    fn description(_prop: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(homie5::HomieDataType::Datetime)
            .settable(true)
            .build()
    }
}
impl TryFrom<&str> for TimeValue {
    type Error = jiff::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(value.parse()?))
    }
}

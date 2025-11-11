use crate::connection::Connection;
use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    string_enum, ActionPropertyValue, BooleanValue, PropertyDescription, PropertyValue,
};
use crate::homie::EventResult;
use crate::modbus;
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

super::node::properties! { static PROPERTIES = [
    { "clock": ClockValue = aggregate
        "TIME_YEAR", "TIME_MONTH", "TIME_DAY", "TIME_HOUR", "TIME_MINUTE", "TIME_SECOND"
    },
    { "dst-enabled": BooleanValue = register "TIME_AUTO_SUM_WIN" },
    { "use-24-hour-format": BooleanValue = register "HOUR_FORMAT" },
    { "weekday": Weekday = register "DAY_OF_THE_WEEK" },
    { "dst-active": BooleanValue = register "DST_PERIOD_ACTIVE" },
    { "synchronize": SynchronizeClockValue = action },
] }

pub struct ClockNode {}

impl ClockNode {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Node for ClockNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("clock")
    }

    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription {
            name: Some("time and date".to_string()),
            r#type: None,
            properties,
        }
    }

    fn properties(&self) -> &[PropertyEntry] {
        &PROPERTIES
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum Weekday {
        Monday = 0,
        Tuesday = 1,
        Wednesday = 2,
        Thursday = 3,
        Friday = 4,
        Saturday = 5,
        Sunday = 6,
    }
}

string_enum! {
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum DaylightMode {
        WinterTime = 0,
        SummerTime = 1,
    }
}

struct ClockValue {
    y: Value,
    mo: Value,
    d: Value,
    h: Value,
    min: Value,
    s: Value,
}
impl ClockValue {
    fn new(y: Value, mo: Value, d: Value, h: Value, min: Value, s: Value) -> Result<Self, ()> {
        Ok(Self {
            y,
            mo,
            d,
            h,
            min,
            s,
        })
    }
}
impl PropertyValue for ClockValue {
    fn value(&self) -> String {
        format!(
            "{:0>2}-{:0>2}-{:0>2}T{:0>2}:{:0>2}:{:0>2}",
            self.y, self.mo, self.d, self.h, self.min, self.s
        )
    }
}
impl PropertyDescription for ClockValue {
    fn description(_: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Datetime)
            .settable(true)
            .build()
    }
}
impl TryFrom<&str> for ClockValue {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        todo!()
    }
}

struct SynchronizeClockValue;
impl PropertyDescription for SynchronizeClockValue {
    fn description(_: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Enum)
            .format(HomiePropertyFormat::Enum(vec!["now".into()]))
            .build()
    }
}
impl TryFrom<&str> for SynchronizeClockValue {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value != "now" {
            return Err(());
        }
        return Ok(SynchronizeClockValue);
    }
}
impl ActionPropertyValue for SynchronizeClockValue {
    fn invoke(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: Arc<Connection>,
    ) -> std::pin::Pin<Box<super::ModbusStream>> {
        Box::pin(async_stream::stream! {
            let system_tz = jiff::tz::TimeZone::system();
            let address = const { RegisterIndex::from_name("TIME_YEAR").unwrap().address() };
            yield loop {
                let time = jiff::Timestamp::now().to_zoned(system_tz.clone());
                let values = vec![
                    time.year() as u16,
                    time.month() as u16,
                    time.day() as u16,
                    time.hour() as u16,
                    time.minute() as u16,
                    time.second() as u16,
                ];
                let operation = modbus::Operation::SetHoldings { address, values };
                let response = modbus.send(operation.clone()).await?;
                let Some(response) = response else { continue };
                if response.is_server_busy() {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                    continue;
                } else {
                    break Ok(EventResult::ActionResponse {
                        node_id: node_id.clone(),
                        prop_idx,
                        value: Box::new(SynchronizeClockValue),
                    });
                }
            };
            // immediately after setting time reload the clock so it also gets reported straight
            // away.
            let operation = modbus::Operation::GetHoldings { address, count: 6 };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            let prop_idx = 0;
            assert_eq!(PROPERTIES[prop_idx].prop_id.as_str(), "clock");
            yield Ok(EventResult::HomieSet { node_id, prop_idx, operation, response });
        })
    }
}
impl PropertyValue for SynchronizeClockValue {
    fn value(&self) -> String {
        "now".into()
    }
}

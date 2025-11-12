use crate::connection::Connection;
use crate::homie::node::{Node, PropertyEntry};
use crate::homie::value::{
    string_enum, ActionPropertyValue, AggregatePropertyValue, BooleanValue, PropertyDescription,
    PropertyValue,
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

super::node::properties! { static PROPERTIES = [
    { "clock": ClockValue = aggregate
        "TIME_YEAR", "TIME_MONTH", "TIME_DAY", "TIME_HOUR", "TIME_MINUTE", "TIME_SECOND"
    },
    { "dst-enabled": BooleanValue = register "TIME_AUTO_SUM_WIN" },
    { "use-24-hour-format": BooleanValue = register "HOUR_FORMAT" },
    { "weekday": Weekday = register "DAY_OF_THE_WEEK" },
    { "dst-active": BooleanValue = register "DST_PERIOD_ACTIVE" },
    { "uptime": UptimeValue = aggregate
        "TIME_RTC_SECONDS_L", "TIME_RTC_SECONDS_H",
        "SYSTEM_START_UP_TIME_L", "SYSTEM_START_UP_TIME_H"
    },
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
    #[impl(TryFromValue, PropertyValue, RegisterPropertyValue, PropertyDescription)]
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
    #[impl(TryFromValue, PropertyValue, RegisterPropertyValue, PropertyDescription)]
    #[repr(u16)]
    #[derive(Clone, Copy)]
    enum DaylightMode {
        WinterTime = 0,
        SummerTime = 1,
    }
}

struct ClockValue(jiff::civil::DateTime);
impl ClockValue {
    fn new(y: Value, mo: Value, d: Value, h: Value, min: Value, s: Value) -> Result<Self, ()> {
        Ok(Self(jiff::civil::datetime(
            y.into_inner() as _,
            mo.into_inner() as _,
            d.into_inner() as _,
            h.into_inner() as _,
            min.into_inner() as _,
            s.into_inner() as _,
            0,
        )))
    }
}
impl PropertyValue for ClockValue {
    fn value(&self) -> String {
        self.0.to_string()
    }
}
impl AggregatePropertyValue for ClockValue {
    const SETTABLE: bool = true;
    fn set(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: Arc<Connection>,
    ) -> std::pin::Pin<Box<super::ModbusStream>> {
        let address = const { RegisterIndex::from_name("TIME_YEAR").unwrap().address() };
        let values = vec![
            self.0.year() as u16,
            self.0.month() as u16,
            self.0.day() as u16,
            self.0.hour() as u16,
            self.0.minute() as u16,
            self.0.second() as u16,
        ];
        Box::pin(async_stream::stream! {
            let operation = modbus::Operation::SetHoldings { address, values };
            // Don't bother checking for the result as any failures here (timeouts, server
            // exceptions, etc.) likely make the value we wanted to set outdated.
            let response = modbus.send(operation.clone()).await?;
            if let Some(response) = response && response.exception_code().is_some() {
                yield Ok(EventResult::HomieSet {
                    node_id: node_id.clone(),
                    prop_idx,
                    operation: operation.clone(),
                    response: response.kind,
                });
            }
            let operation = modbus::Operation::GetHoldings { address, count: 6 };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            yield Ok(EventResult::HomieSet { node_id, prop_idx, operation, response });
        })
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
    fn try_from(_: &str) -> Result<Self, Self::Error> {
        unimplemented!("although clock is settable separately, this has not been implemented yet")
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
        (value == "now").then_some(SynchronizeClockValue).ok_or(())
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
                    modbus.handle_server_busy().await;
                    continue;
                } else {
                    break Ok(EventResult::ActionResponse {
                        node_id: node_id.clone(),
                        prop_idx,
                        value: Box::new(SynchronizeClockValue),
                    });
                }
            };
            let operation = modbus::Operation::GetHoldings { address, count: 6 };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            yield Ok(EventResult::HomieSet { node_id, prop_idx, operation, response });
        })
    }
}
impl PropertyValue for SynchronizeClockValue {
    fn value(&self) -> String {
        "now".into()
    }
}

struct UptimeValue {
    rtc: u32,
    start_time: u32,
}
impl UptimeValue {
    fn new(rtc_l: Value, rtc_h: Value, up_l: Value, up_h: Value) -> Result<Self, ()> {
        Ok(Self {
            rtc: u32::from(rtc_h.into_inner()) << 16 | u32::from(rtc_l.into_inner()),
            start_time: u32::from(up_h.into_inner()) << 16 | u32::from(up_l.into_inner()),
        })
    }
}
impl PropertyValue for UptimeValue {
    fn value(&self) -> String {
        let current = jiff::Timestamp::from_second(self.rtc as i64).unwrap();
        let start = jiff::Timestamp::from_second(self.start_time as i64).unwrap();
        let duration = current.duration_since(start);
        duration.to_string()
    }
}
impl AggregatePropertyValue for UptimeValue {
    const SETTABLE: bool = false;
    fn set(
        &self,
        _: HomieID,
        _: usize,
        _: Arc<Connection>,
    ) -> std::pin::Pin<Box<super::ModbusStream>> {
        unreachable!("uptime is not settable");
    }
}
impl PropertyDescription for UptimeValue {
    fn description(_: &PropertyEntry) -> homie5::device_description::HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Duration).build()
    }
}
// TODO: can we avoid unnecessary implementations like these?
impl TryFrom<&str> for UptimeValue {
    type Error = ();
    fn try_from(_: &str) -> Result<Self, Self::Error> {
        unreachable!()
    }
}

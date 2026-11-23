//! Exposes the device alarms as a list of the active ones.
//!
//! The SystemAIR units keep two bits of information for each alarm: whether it is firing, and
//! whether it is pending a change to the other state. A firing alarm that has been cleared stays
//! firing ("acknowledged") until its condition goes away, and a clear alarm whose condition has
//! appeared is "evaluating" until it fires. `active` lists the alarms that are firing, including
//! the acknowledged ones.
//!
//! The alarm registers are many, but change rarely. `count` follows the number of active alarms,
//! and a change in it re-reads all the alarm registers, so polling `alarm/count` frequently is
//! enough to notice alarms promptly.

use crate::connection::Connection;
use crate::homie::node::{Node, PropertyEntry, PropertyKind};
use crate::homie::value::{
    ActionPropertyValue, DynPropertyValue, PropertyDescription, PropertyValue, unit,
};
use crate::homie::{EventResult, EventStream};
use crate::modbus;
use crate::modbus_device_cache::ModbusDeviceValues;
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{
    HomieNodeDescription, HomiePropertyDescription, HomiePropertyFormat, PropertyDescriptionBuilder,
};
use homie5::{HomieDataType, HomieID};
use std::collections::{BTreeMap, BTreeSet};
use std::pin::Pin;
use std::sync::Arc;

super::node::properties! { static PROPERTIES = [
    { "active": ActiveAlarms = kind &ActiveAlarmsKind },
    { "count": AlarmCount = register "ALARM_AMOUNT_ACTIVE" },
    { "clear": ClearAction = action },
] }

const fn register(name: &str) -> RegisterIndex {
    RegisterIndex::from_name(name).unwrap()
}

const ALARMS: [(&str, RegisterIndex); 33] = [
    ("supply-air-fan-control", register("ALARM_SAF_CTRL_ALARM")),
    ("extract-air-fan-control", register("ALARM_EAF_CTRL_ALARM")),
    ("frost-protection", register("ALARM_FROST_PROT_ALARM")),
    ("defrosting", register("ALARM_DEFROSTING_ALARM")),
    ("supply-air-fan-rpm", register("ALARM_SAF_RPM_ALARM")),
    ("extract-air-fan-rpm", register("ALARM_EAF_RPM_ALARM")),
    ("frost-protection-sensor", register("ALARM_FPT_ALARM")),
    ("outdoor-air-temperature", register("ALARM_OAT_ALARM")),
    ("supply-air-temperature", register("ALARM_SAT_ALARM")),
    ("room-air-temperature", register("ALARM_RAT_ALARM")),
    ("extract-air-temperature", register("ALARM_EAT_ALARM")),
    ("extra-controller-temperature", register("ALARM_ECT_ALARM")),
    ("efficiency-temperature", register("ALARM_EFT_ALARM")),
    ("overheat-protection-sensor", register("ALARM_OHT_ALARM")),
    ("emergency-thermostat", register("ALARM_EMT_ALARM")),
    ("rotor-guard", register("ALARM_RGS_ALARM")),
    ("bypass-damper-position-sensor", register("ALARM_BYS_ALARM")),
    ("secondary-air", register("ALARM_SECONDARY_AIR_ALARM")),
    ("filter", register("ALARM_FILTER_ALARM")),
    ("extra-controller", register("ALARM_EXTRA_CONTROLLER_ALARM")),
    ("external-stop", register("ALARM_EXTERNAL_STOP_ALARM")),
    ("relative-humidity", register("ALARM_RH_ALARM")),
    ("co2", register("ALARM_CO2_ALARM")),
    ("low-supply-air-temperature", register("ALARM_LOW_SAT_ALARM")),
    ("bypass-damper-feedback", register("ALARM_BYF_ALARM")),
    ("manual-override-outputs", register("ALARM_MANUAL_OVERRIDE_OUTPUTS_ALARM")),
    ("pdm-room-humidity-sensor", register("ALARM_PDM_RHS_ALARM")),
    ("pdm-extract-room-temperature", register("ALARM_PDM_EAT_ALARM")),
    ("manual-fan-stop", register("ALARM_MANUAL_FAN_STOP_ALARM")),
    ("overheat-temperature", register("ALARM_OVERHEAT_TEMPERATURE_ALARM")),
    ("fire", register("ALARM_FIRE_ALARM_ALARM")),
    ("filter-warning", register("ALARM_FILTER_WARNING_ALARM")),
    ("heater-malfunction", register("ALARM_HEATER_MALFUNCTION_ALARM")),
];

static ALARM_REGISTERS: [RegisterIndex; ALARMS.len()] = {
    let mut registers = [ALARMS[0].1; ALARMS.len()];
    let mut i = 0;
    while i < ALARMS.len() {
        registers[i] = ALARMS[i].1;
        i += 1;
    }
    registers
};

/// Read all the alarm registers, in as few requests as possible.
fn read_alarms(modbus: Arc<Connection>) -> Pin<Box<EventStream>> {
    let mut addresses: Vec<u16> = ALARM_REGISTERS.iter().map(|r| r.address()).collect();
    addresses.sort();
    let mut ranges: Vec<(u16, u16)> = Vec::new();
    for address in addresses {
        match ranges.last_mut() {
            Some((start, count)) if address - *start < modbus::MAX_SAFE_READ_COUNT => {
                *count = address - *start + 1;
            }
            _ => ranges.push((address, 1)),
        }
    }
    Box::pin(async_stream::stream! {
        for (address, count) in ranges {
            let operation = modbus::Operation::GetHoldings { address, count };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            yield Ok(EventResult::Periodic { operation, response });
        }
    })
}

pub struct AlarmNode {}

impl AlarmNode {
    pub fn new() -> Self {
        Self {}
    }
}

impl Node for AlarmNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("alarm")
    }
    fn description(&self) -> HomieNodeDescription {
        let properties = PROPERTIES
            .iter()
            .map(|prop| (prop.prop_id.clone(), prop.description()))
            .collect::<BTreeMap<_, _>>();
        HomieNodeDescription { name: Some("device alarms".to_string()), r#type: None, properties }
    }

    fn properties(&self) -> &'static [super::node::PropertyEntry] {
        &PROPERTIES
    }
}

/// The names of the firing alarms, sorted.
pub(crate) struct ActiveAlarms(BTreeSet<&'static str>);

impl PropertyValue for ActiveAlarms {
    fn value(&self) -> String {
        serde_json::to_string(&self.0).expect("a list of strings serializes")
    }
}

impl PropertyDescription for ActiveAlarms {
    fn description(_: &PropertyEntry) -> HomiePropertyDescription {
        let schema = r#"{"type":"array","items":{"type":"string"}}"#;
        PropertyDescriptionBuilder::new(HomieDataType::JSON)
            .format(HomiePropertyFormat::Json(schema.to_string()))
            .build()
    }
}

pub(crate) struct ActiveAlarmsKind;

impl PropertyKind for ActiveAlarmsKind {
    fn registers(&self) -> &[RegisterIndex] {
        &ALARM_REGISTERS
    }

    fn value_from_modbus(
        &self,
        modbus: &ModbusDeviceValues,
    ) -> Option<Result<Box<DynPropertyValue>, ()>> {
        let mut active = BTreeSet::new();
        for (name, register) in ALARMS {
            // 0 = clear, 1 = firing, 2 = clear but evaluating, 3 = firing but acknowledged.
            match modbus.value_of(register)?.into_inner() {
                0 | 2 => {}
                1 | 3 => {
                    active.insert(name);
                }
                _ => return Some(Err(())),
            }
        }
        Some(Ok(Box::new(ActiveAlarms(active))))
    }

    fn value_from_homie(&self, _: &str) -> Result<Box<DynPropertyValue>, ()> {
        Err(())
    }

    fn homie_set_to_modbus(
        &self,
        _: HomieID,
        _: usize,
        _: Arc<Connection>,
        _: Box<DynPropertyValue>,
    ) -> Pin<Box<EventStream>> {
        unreachable!("the list of active alarms is not settable")
    }

    fn on_property_change(
        &self,
        _: HomieID,
        _: usize,
        _: Arc<Connection>,
        _: Box<DynPropertyValue>,
    ) -> Pin<Box<EventStream>> {
        Box::pin(futures::stream::empty())
    }

    fn adjust_description(&self, description: &mut HomiePropertyDescription) {
        description.retained = true;
        description.settable = false;
    }
}

pub(crate) struct AlarmCount(u16);

impl TryFrom<Value> for AlarmCount {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Ok(Self(value.into_inner()))
    }
}

impl TryFrom<&str> for AlarmCount {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(value.parse().map_err(|_| ())?))
    }
}

impl PropertyValue for AlarmCount {
    fn value(&self) -> String {
        self.0.to_string()
    }

    fn on_property_change(
        &self,
        _: HomieID,
        _: usize,
        modbus: Arc<Connection>,
    ) -> Pin<Box<EventStream>> {
        read_alarms(modbus)
    }
}

impl crate::homie::value::RegisterPropertyValue for AlarmCount {
    fn to_modbus(&self) -> u16 {
        self.0
    }
}

impl PropertyDescription for AlarmCount {
    fn description(prop: &PropertyEntry) -> HomiePropertyDescription {
        crate::homie::value::UintValue::<unit::None>::description(prop)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ClearAction;

impl PropertyValue for ClearAction {
    fn value(&self) -> String {
        "all".to_string()
    }
}

impl PropertyDescription for ClearAction {
    fn description(_: &PropertyEntry) -> HomiePropertyDescription {
        PropertyDescriptionBuilder::new(HomieDataType::Enum)
            .format(HomiePropertyFormat::Enum(vec!["all".into()]))
            .build()
    }
}

impl TryFrom<&str> for ClearAction {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        (value == "all").then_some(ClearAction).ok_or(())
    }
}

impl ActionPropertyValue for ClearAction {
    fn invoke(
        &self,
        node_id: HomieID,
        prop_idx: usize,
        modbus: Arc<Connection>,
    ) -> Pin<Box<EventStream>> {
        Box::pin(async_stream::stream! {
            let address = const { register("ALARM_CLEAR_ALL") }.address();
            let operation = modbus::Operation::SetHoldings { address, values: vec![1] };
            let response = modbus.send_retrying(operation).await?;
            if response.exception_code().is_some() {
                yield Ok(EventResult::ActionFailure {
                    node_id,
                    prop_idx,
                    why: "the device refused to clear the alarms",
                });
                return;
            }
            yield Ok(EventResult::ActionResponse { node_id, prop_idx, value: Box::new(Self) });
            let address = const { register("ALARM_AMOUNT_ACTIVE") }.address();
            let operation = modbus::Operation::GetHoldings { address, count: 1 };
            let response = modbus.send_retrying(operation.clone()).await?.kind;
            yield Ok(EventResult::Periodic { operation, response });
            for await result in read_alarms(modbus) {
                yield result;
            }
        })
    }
}

//! Exposes fan speed settings as a homie node.

use crate::homie::common::{
    homie_enum, BooleanValue, PropertyDescription, PropertyValue, UintValue,
};
use crate::homie::node::{Node, NodeEvent, PropertyRegisterEntry};
use crate::registers::{RegisterIndex, Value};
use homie5::device_description::{HomieNodeDescription, HomiePropertyDescription};
use homie5::HomieID;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

static REGISTERS: [PropertyRegisterEntry; 71] = super::node::property_registers![
    (1121 is "min-demand-control": AirflowLevel),
    (1122 is "max-demand-control": AirflowLevel),
    (1131 is "usermode-manual": AirflowLevel),
    (1135 is "usermode-crowded-supply": AirflowLevel),
    (1136 is "usermode-crowded-extract": AirflowLevel),
    (1137 is "usermode-refresh-supply": AirflowLevel),
    (1138 is "usermode-refresh-extract": AirflowLevel),
    (1139 is "usermode-fireplace-supply": AirflowLevel),
    (1140 is "usermode-fireplace-extract": AirflowLevel),
    (1141 is "usermode-away-supply": AirflowLevel),
    (1142 is "usermode-away-extract": AirflowLevel),
    (1143 is "usermode-holiday-supply": AirflowLevel),
    (1144 is "usermode-holiday-extract": AirflowLevel),
    (1145 is "usermode-cooker-hood-supply": AirflowLevel),
    (1146 is "usermode-cooker-hood-extract": AirflowLevel),
    (1147 is "usermode-vacuum-cleaner-supply": AirflowLevel),
    (1148 is "usermode-vacuum-cleaner-extract": AirflowLevel),
    (1171 is "digital-input-1-supply": AirflowLevel),
    (1172 is "digital-input-1-extract": AirflowLevel),
    (1173 is "digital-input-2-supply": AirflowLevel),
    (1174 is "digital-input-2-extract": AirflowLevel),
    (1175 is "digital-input-3-supply": AirflowLevel),
    (1176 is "digital-input-3-extract": AirflowLevel),
    (1177 is "pressure-guard-supply": AirflowLevel),
    (1178 is "pressure-guard-extract": AirflowLevel),
    (1274 is "regulation-type": RegulationType),
    (1353 is "allow-manual-stop": BooleanValue<true>),
    (1401 is "supply-percentage-for-minimum": UintValue<true>),
    (1402 is "extract-percentage-for-minimum": UintValue<true>),
    (1403 is "supply-percentage-for-low": UintValue<true>),
    (1404 is "extract-percentage-for-low": UintValue<true>),
    (1405 is "supply-percentage-for-normal": UintValue<true>),
    (1406 is "extract-percentage-for-normal": UintValue<true>),
    (1407 is "supply-percentage-for-high": UintValue<true>),
    (1408 is "extract-percentage-for-high": UintValue<true>),
    (1409 is "supply-percentage-for-maximum": UintValue<true>),
    (1410 is "extract-percentage-for-maximum": UintValue<true>),
    (1411 is "supply-rpm-for-minimum": UintValue<true>),
    (1412 is "extract-rpm-for-minimum": UintValue<true>),
    (1413 is "supply-rpm-for-low": UintValue<true>),
    (1414 is "extract-rpm-for-low": UintValue<true>),
    (1415 is "supply-rpm-for-normal": UintValue<true>),
    (1416 is "extract-rpm-for-normal": UintValue<true>),
    (1417 is "supply-rpm-for-high": UintValue<true>),
    (1418 is "extract-rpm-for-high": UintValue<true>),
    (1419 is "supply-rpm-for-maximum": UintValue<true>),
    (1420 is "extract-rpm-for-maximum": UintValue<true>),
    (1421 is "supply-pressure-for-minimum": UintValue<true>),
    (1422 is "extract-pressure-for-minimum": UintValue<true>),
    (1423 is "supply-pressure-for-low": UintValue<true>),
    (1424 is "extract-pressure-for-low": UintValue<true>),
    (1425 is "supply-pressure-for-normal": UintValue<true>),
    (1426 is "extract-pressure-for-normal": UintValue<true>),
    (1427 is "supply-pressure-for-high": UintValue<true>),
    (1428 is "extract-pressure-for-high": UintValue<true>),
    (1429 is "supply-pressure-for-maximum": UintValue<true>),
    (1430 is "extract-pressure-for-maximum": UintValue<true>),
    (1431 is "supply-flow-for-minimum": UintValue<true>),
    (1432 is "extract-flow-for-minimum": UintValue<true>),
    (1433 is "supply-flow-for-low": UintValue<true>),
    (1434 is "extract-flow-for-low": UintValue<true>),
    (1435 is "supply-flow-for-normal": UintValue<true>),
    (1436 is "extract-flow-for-normal": UintValue<true>),
    (1437 is "supply-flow-for-high": UintValue<true>),
    (1438 is "extract-flow-for-high": UintValue<true>),
    (1439 is "supply-flow-for-maximum": UintValue<true>),
    (1440 is "extract-flow-for-maximum": UintValue<true>),
    (4112 is "min-free-cooling-supply": AirflowLevel),
    (4113 is "min-free-cooling-extract": AirflowLevel),
    (5060 is "during-active-week-schedule": WeeklyScheduleLevel),
    (5061 is "during-inactive-week-schedule": WeeklyScheduleLevel),
];

pub struct FanSpeedSettingsNode {
    device_values: [Option<Value>; REGISTERS.len()],
    sender: Sender<NodeEvent>,
}

impl FanSpeedSettingsNode {
    pub(crate) fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel::<NodeEvent>(1024);
        Self {
            device_values: [None; _],
            sender,
        }
    }
}

impl Node for FanSpeedSettingsNode {
    fn node_id(&self) -> HomieID {
        HomieID::new_const("fan-speed-settings")
    }

    fn description(&self) -> HomieNodeDescription {
        let mut properties = BTreeMap::new();
        for prop_register in &REGISTERS {
            properties.insert(
                prop_register.prop_id.clone(),
                (prop_register.mk_description)(),
            );
        }
        HomieNodeDescription {
            name: Some("fan speed settings".to_string()),
            r#type: None,
            properties,
        }
    }

    fn on_register_value(&mut self, register: RegisterIndex, value: Value) {
        let Ok(idx) = REGISTERS.binary_search_by_key(&register, |v| v.register) else {
            return;
        };
        let old_value = self.device_values[idx];
        if old_value == Some(value) {
            return;
        }
        self.device_values[idx] = Some(value);
        let PropertyRegisterEntry {
            prop_id,
            from_value,
            ..
        } = &REGISTERS[idx];
        let old_value = old_value.map(from_value);
        let new_value = from_value(value);
        let (tgt_changed, val_changed, new) = match (old_value, new_value) {
            (None | Some(Err(_)) | Some(Ok(_)), Err(_)) => {
                tracing::debug!(
                    ?value,
                    address = register.address(),
                    ?prop_id,
                    "could not parse value from device"
                );
                return;
            }
            (None | Some(Err(_)), Ok(new)) => (new.has_target(), true, new),
            (Some(Ok(old)), Ok(new)) => (
                new.has_target() && old.target() != new.target(),
                old.value() != new.value(),
                new,
            ),
        };
        if tgt_changed {
            let _ignore_no_receivers = self.sender.send(NodeEvent::TargetChanged {
                node_id: self.node_id(),
                prop_id: prop_id.clone(),
                new: Arc::clone(&new) as _,
            });
        }
        if val_changed {
            let _ignore_no_receivers = self.sender.send(NodeEvent::PropertyChanged {
                node_id: self.node_id(),
                prop_id: prop_id.clone(),
                new,
            });
        }
    }

    fn node_events(&self) -> tokio::sync::broadcast::Receiver<NodeEvent> {
        self.sender.subscribe()
    }

    fn values_populated(&self) -> bool {
        self.device_values.iter().all(|v| v.is_some())
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum AirflowLevel {
    Off = 0,
    Minimum = 1,
    Low = 2,
    Normal = 3,
    High = 4,
    Maximum = 5,
}
impl PropertyValue for AirflowLevel {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }
}
impl PropertyDescription for AirflowLevel {
    fn description() -> HomiePropertyDescription {
        homie_enum::<AirflowLevel>().settable(true).build()
    }
}
impl TryFrom<Value> for AirflowLevel {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::from_repr(value.into_inner()).ok_or(())
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum WeeklyScheduleLevel {
    Off = 0,
    Minimum = 1,
    Low = 2,
    Normal = 3,
    High = 4,
    DemandControl = 5,
}
impl PropertyValue for WeeklyScheduleLevel {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }
}
impl PropertyDescription for WeeklyScheduleLevel {
    fn description() -> HomiePropertyDescription {
        homie_enum::<WeeklyScheduleLevel>().settable(true).build()
    }
}
impl TryFrom<Value> for WeeklyScheduleLevel {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::from_repr(value.into_inner()).ok_or(())
    }
}

#[repr(u16)]
#[derive(Clone, Copy, strum::VariantNames, strum::FromRepr, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum RegulationType {
    Manual = 0,
    RPM = 1,
    ConstantPressure = 2,
    ConstantFlow = 3,
    External = 4,
}
impl TryFrom<Value> for RegulationType {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        Self::from_repr(value.into_inner()).ok_or(())
    }
}
impl PropertyValue for RegulationType {
    fn value(&self) -> String {
        <&'static str>::from(self).to_string()
    }
}
impl PropertyDescription for RegulationType {
    fn description() -> HomiePropertyDescription {
        homie_enum::<RegulationType>().settable(true).build()
    }
}

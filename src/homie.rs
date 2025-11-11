mod alarm_node;
mod clock_node;
mod compensation_node;
mod cooler_node;
mod demand_control_node;
mod fan_speed_setting_node;
mod filter_node;
mod free_cooling_node;
mod heat_exchanger_node;
mod heater_node;
mod node;
mod value;

use crate::connection::{self, Connection};
use crate::homie::node::Node;
use crate::homie::value::DynPropertyValue;
use crate::modbus::{self, Operation, ResponseKind};
use crate::modbus_device_cache::{ModbusDeviceValues, RegisterBitmask};
use futures::stream::SelectAll;
use futures::{Stream, StreamExt as _};
use homie5::client::{Publish, QoS, Subscription};
use homie5::device_description::HomieDeviceDescription;
use homie5::{Homie5DeviceProtocol, HomieDeviceStatus, HomieID, PropertyRef};
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

pub(crate) enum EventResult {
    Periodic {
        operation: Operation,
        response: ResponseKind,
    },
    HomieSet {
        node_id: HomieID,
        prop_idx: usize,
        operation: Operation,
        response: ResponseKind,
    },
    ActionResponse {
        node_id: HomieID,
        prop_idx: usize,
        value: Box<DynPropertyValue>,
    },
}

type ModbusStream = dyn Send + Sync + Stream<Item = Result<EventResult, connection::Error>>;

type ModbusReadStream = SelectAll<Pin<Box<ModbusStream>>>;

pub(crate) struct SystemAirDevice {
    mqtt: rumqttc::v5::AsyncClient,
    protocol: Homie5DeviceProtocol,
    state: HomieDeviceStatus,
    description: HomieDeviceDescription,
    nodes: BTreeMap<HomieID, Box<dyn Node>>,
    modbus: Arc<Connection>,
    modbus_values: ModbusDeviceValues,
    event_stream: ModbusReadStream,
    commands: mpsc::UnboundedReceiver<Command>,
}

impl SystemAirDevice {
    pub(crate) fn new(
        mqtt: rumqttc::v5::AsyncClient,
        protocol: Homie5DeviceProtocol,
        modbus: Arc<Connection>,
        commands: mpsc::UnboundedReceiver<Command>,
    ) -> Self {
        let nodes = [
            Box::new(clock_node::ClockNode::new()) as Box<dyn Node>,
            Box::new(alarm_node::AlarmNode::new()) as Box<dyn Node>,
            Box::new(demand_control_node::DemandControlNode::new()) as _,
            Box::new(fan_speed_setting_node::FanSpeedSettingsNode::new()) as _,
            Box::new(compensation_node::CompensationNode::new()) as _,
            Box::new(free_cooling_node::FreeCoolingNode::new()) as _,
            Box::new(filter_node::FilterNode::new()) as _,
            Box::new(heater_node::HeaterNode::new()) as _,
            Box::new(heat_exchanger_node::HeatExchangerNode::new()) as _,
            Box::new(cooler_node::CoolerNode::new()) as _,
        ];
        let mut description =
            homie5::device_description::DeviceDescriptionBuilder::new().name("SystemAIR SAVE");
        for node in &nodes {
            description = description.add_node(node.node_id(), node.description());
        }
        let description = description.build();
        let nodes = nodes.into_iter().map(|v| (v.node_id(), v)).collect();
        Self {
            mqtt,
            state: HomieDeviceStatus::Init,
            protocol,
            description,
            commands,
            nodes,
            modbus,
            modbus_values: ModbusDeviceValues::new(),
            event_stream: ModbusReadStream::new(),
        }
    }

    pub async fn publish_device(&mut self) -> Result<(), rumqttc::v5::ClientError> {
        for step in homie5::homie_device_publish_steps() {
            match step {
                homie5::DevicePublishStep::DeviceStateInit => {
                    self.state = HomieDeviceStatus::Init;
                    let p = self.protocol.publish_state(self.state);
                    self.mqtt.homie_publish(p).await?;
                }
                homie5::DevicePublishStep::DeviceDescription => {
                    let p = self
                        .protocol
                        .publish_description(&self.description)
                        .expect("TODO");
                    self.mqtt.homie_publish(p).await?;
                }
                homie5::DevicePublishStep::PropertyValues => {
                    tracing::info!("waiting for device read out…");
                    let mut need_registers = RegisterBitmask::new();
                    self.event_stream.clear();
                    for (_, node) in &self.nodes {
                        for property in node.properties() {
                            for register in property.kind.registers() {
                                need_registers.set(register.address());
                            }
                        }
                    }
                    for range in need_registers.find_optimal_ranges(modbus::MAX_SAFE_READ_COUNT) {
                        self.schedule_periodic_read(
                            *range.start(),
                            u16::try_from(range.len()).unwrap(),
                            // FIXME: determine read period from node information.
                            Duration::from_secs(30),
                        );
                    }

                    while !self.modbus_values.has_all_values(&need_registers) {
                        self.step().await.expect("TODO");
                    }
                    // rumqttc appears to be sending publishes in a weird order that results in
                    // some of the properties getting published *after* `$state = Ready`. Yielding
                    // here gives it *some* time to do it's thing.
                    tokio::task::yield_now().await;
                }
                homie5::DevicePublishStep::SubscribeProperties => {
                    // FIXME: fix need for peekable upstream somehow? Right now an empty
                    // subscription surfaces in `client_loop.poll()` result
                    // `MqttState(EmptySubscription)`
                    let mut p = self
                        .protocol
                        .subscribe_props(&self.description)
                        .expect("TODO")
                        .peekable();
                    if p.peek().is_some() {
                        self.mqtt.homie_subscribe(p).await?;
                    }
                }
                homie5::DevicePublishStep::DeviceStateReady => {
                    tracing::debug!("device becomes ready...");
                    self.state = HomieDeviceStatus::Ready;
                    let p = self.protocol.publish_state(self.state);
                    self.mqtt.homie_publish(p).await?;
                }
            }
        }
        Ok(())
    }

    fn schedule_periodic_read(&mut self, address: u16, count: u16, period: Duration) {
        let modbus = Arc::clone(&self.modbus);
        let stream = futures::stream::unfold(Instant::now(), move |when| {
            let modbus = Arc::clone(&modbus);
            async move {
                let operation = Operation::GetHoldings { address, count };
                tokio::time::sleep_until(when).await;
                let response = match modbus.send_retrying(operation.clone()).await {
                    Ok(r) => r,
                    Err(e) => return Some((Err(e), when + period)),
                };
                let result = EventResult::Periodic {
                    operation,
                    response: response.kind,
                };
                Some((Ok(result), when + period))
            }
        });
        self.event_stream.push(Box::pin(stream));
    }

    async fn handle_value_change(
        &mut self,
        node_id: &HomieID,
        prop_idx: usize,
        new: &DynPropertyValue,
    ) -> Result<(), ()> {
        let node = self.nodes.get(node_id).unwrap();
        let prop = &node.properties()[prop_idx];
        let prop_id = &prop.prop_id;
        let Some(pd) = self.description.get_property_by_id(node_id, prop_id) else {
            tracing::warn!(
                ?node_id,
                ?prop_id,
                "property change event without description"
            );
            return Ok(());
        };
        let val = new.value();
        let msg = self
            .protocol
            .publish_value(node_id, prop_id, val, pd.retained);
        self.mqtt.homie_publish(msg).await.expect("TODO");
        Ok(())
    }

    async fn handle_target_change(
        &mut self,
        node_id: &HomieID,
        prop_idx: usize,
        new: &DynPropertyValue,
    ) -> Result<(), ()> {
        let node = self.nodes.get(node_id).unwrap();
        let prop = &node.properties()[prop_idx];
        let prop_id = &prop.prop_id;
        let Some(pd) = self.description.get_property_by_id(node_id, &prop_id) else {
            tracing::warn!(
                ?node_id,
                ?prop_id,
                "property change event without description"
            );
            return Ok(());
        };
        let Some(tgt) = new.target() else {
            return Ok(());
        };
        let msg = self
            .protocol
            .publish_target(&node_id, &prop_id, tgt, pd.retained);
        self.mqtt.homie_publish(msg).await.expect("TODO");
        Ok(())
    }

    async fn handle_modbus_register_response(
        &mut self,
        address: u16,
        values: Vec<u8>,
        inhibit_change_handling: bool,
    ) -> Result<(), ()> {
        // This is somewhat awkward to write as we want our nodes to see a full atomic change to
        // the register value view, but we also want to have the old values to compare against.
        // For that reason the updates to our cached view of the modbus registers occurs after we
        // compute old property values with the unmodified map. We could simplify the code by
        // cloning the register values, but its a full 128kB clone!
        let mut changing_registers = RegisterBitmask::new();
        let (chunks, remainder) = values.as_chunks::<2>();
        if !remainder.is_empty() {
            tracing::warn!("response contains non-even number of bytes, modbus is misbehaving?");
        }
        for (word, address) in chunks.iter().zip(address..) {
            let word = u16::from_be_bytes(*word);
            if self.modbus_values.value_of_address(address) != Some(word) {
                changing_registers.set(address);
            }
        }
        let mut property_values = Vec::new();
        for (node_id, node) in &self.nodes {
            'next_prop: for (prop_idx, property) in node.properties().iter().enumerate() {
                for register in property.kind.registers() {
                    let address = register.address();
                    if !changing_registers.is_set(address) {
                        continue;
                    }
                    let prop_id = &property.prop_id;
                    tracing::debug!(
                        %node_id,
                        %prop_id,
                        address,
                        "register change reloads node property"
                    );
                    let old = property.kind.value_from_modbus(&self.modbus_values);
                    property_values.push((node_id.clone(), prop_idx, old));
                    continue 'next_prop;
                }
            }
        }
        for (word, address) in chunks.iter().zip(address..) {
            let word = u16::from_be_bytes(*word);
            self.modbus_values.set_value(address, word);
        }
        for (node_id, prop_idx, old) in property_values {
            let node = self.nodes.get(&node_id).unwrap();
            let prop = &node.properties()[prop_idx];
            let new = prop.kind.value_from_modbus(&self.modbus_values);
            let (value_changed, target_changed, new) = match (old, new) {
                (None, None) => (false, false, None),
                (Some(_), None) => todo!("erasing a value??"),
                (_, Some(Err(error))) => {
                    tracing::debug!(
                        %node_id,
                        prop_id = %prop.prop_id,
                        ?error,
                        "could not parse property from device"
                    );
                    (false, false, None)
                }
                (None, Some(Ok(new))) => (true, new.target().is_some(), Some(new)),
                (Some(Err(error)), Some(Ok(new))) => {
                    tracing::debug!(
                        %node_id,
                        prop_id = %prop.prop_id,
                        ?error,
                        "could not parse old property from device"
                    );
                    (true, new.target().is_some(), Some(new))
                }
                (Some(Ok(old)), Some(Ok(new))) => {
                    let value_changed = old.value() != new.value();
                    let new_target = new.target();
                    let target_changed = new_target.is_some() && new_target != old.target();
                    (value_changed, target_changed, Some(new))
                }
            };
            if let Some(new) = new {
                if !inhibit_change_handling && value_changed {
                    self.handle_value_change(&node_id, prop_idx, &*new).await?;
                }
                if !inhibit_change_handling && target_changed {
                    self.handle_target_change(&node_id, prop_idx, &*new).await?;
                }
            }
        }
        Ok(())
    }

    async fn handle_command(&mut self, command: Command) -> Result<(), ()> {
        if self.state != HomieDeviceStatus::Ready {
            tracing::debug!(?command, "command ignored, device is not ready yet");
            return Ok(());
        }
        match command {
            Command::Set { property, value } => {
                if property.device_id() != self.protocol.device_ref().device_id() {
                    return Err(()); // TODO
                }
                let Some(node) = self.nodes.get(property.node_id()) else {
                    return Err(()); // TODO
                };
                let node_id = node.node_id().clone();
                let properties = node.properties();
                let property = properties
                    .iter()
                    .enumerate()
                    .find(|(_, p)| &p.prop_id == property.prop_id());
                let Some((prop_idx, property)) = property else {
                    return Err(()); // TODO
                };
                let prop_id = property.prop_id.clone();
                let Ok(value) = property.kind.value_from_homie(&value) else {
                    tracing::warn!(%node_id, %prop_id, value, "property/set could not be parsed");
                    let Some(Ok(old)) = property.kind.value_from_modbus(&self.modbus_values) else {
                        tracing::warn!(%node_id, %prop_id, "old value could not be parsed");
                        return Ok(());
                    };
                    self.handle_target_change(&node_id, prop_idx, &*old).await?;
                    self.handle_value_change(&node_id, prop_idx, &*old).await?;
                    return Ok(());
                };
                let task = property.kind.homie_set_to_modbus(
                    node_id,
                    prop_idx,
                    Arc::clone(&self.modbus),
                    value,
                );
                self.event_stream.push(task);
            }
        }
        Ok(())
    }

    pub async fn step(&mut self) -> Result<(), ()> {
        loop {
            tracing::trace!(commands.len = self.commands.len(), "step");
            tokio::select! {
                event = self.event_stream.next(), if !self.event_stream.is_empty() => {
                    let Some(read_event) = event else { continue };
                    let result = read_event.expect("TODO");
                    return self.handle_event_result(result).await;
                },
                command = self.commands.recv() => {
                    let Some(command) = command else { todo!() };
                    return self.handle_command(command).await;
                },
            }
        }
    }

    async fn handle_event_result(&mut self, result: EventResult) -> Result<(), ()> {
        match result {
            EventResult::Periodic {
                operation,
                response: ResponseKind::ErrorCode(code),
            } => {
                tracing::error!(code, ?operation, "modbus server exception occurred");
                return Ok(());
            }
            EventResult::Periodic {
                operation: Operation::GetHoldings { address, count: _ },
                response: ResponseKind::GetHoldings { values },
            } => {
                self.handle_modbus_register_response(address, values, false)
                    .await?;
            }
            EventResult::HomieSet {
                node_id,
                prop_idx,
                operation,
                response: ResponseKind::ErrorCode(code),
            } => {
                tracing::error!(code, ?operation, "modbus server exception occurred");
                let node = self.nodes.get(&node_id).unwrap();
                let prop = &node.properties()[prop_idx];
                if let Some(Ok(old)) = prop.kind.value_from_modbus(&self.modbus_values) {
                    let r1 = self.handle_target_change(&node_id, prop_idx, &*old).await;
                    let r2 = self.handle_value_change(&node_id, prop_idx, &*old).await;
                    r1.and(r2)?;
                } else {
                    let prop_id = &prop.prop_id;
                    tracing::debug!(%node_id, %prop_id, "failed to parse old value from modbus");
                }
            }
            EventResult::HomieSet {
                node_id,
                prop_idx,
                operation: Operation::GetHoldings { address, .. },
                response: ResponseKind::GetHoldings { values },
            } => {
                let r1 = self
                    .handle_modbus_register_response(address, values, true)
                    .await;
                let node = self.nodes.get(&node_id).unwrap();
                let prop = &node.properties()[prop_idx];
                if let Some(Ok(new)) = prop.kind.value_from_modbus(&self.modbus_values) {
                    let r2 = self.handle_target_change(&node_id, prop_idx, &*new).await;
                    let r3 = self.handle_value_change(&node_id, prop_idx, &*new).await;
                    r1.and(r2).and(r3)?;
                } else {
                    let prop_id = &prop.prop_id;
                    tracing::debug!(%node_id, %prop_id, "failed to parse set value from modbus");
                }
            }
            EventResult::ActionResponse {
                node_id,
                prop_idx,
                value,
            } => {
                self.handle_value_change(&node_id, prop_idx, &*value)
                    .await?;
            }
            EventResult::Periodic { .. } => unreachable!("EventResult::Periodic"),
            EventResult::HomieSet { .. } => unreachable!("EventResult::HomieSet"),
        }
        Ok(())
    }
}

trait MqttClientExt {
    type PublishError;
    type SubscribeError;
    async fn homie_publish(&self, p: Publish) -> Result<(), Self::PublishError>;
    async fn homie_subscribe(
        &self,
        subs: impl Iterator<Item = Subscription> + Send,
    ) -> Result<(), Self::SubscribeError>;
}

impl MqttClientExt for rumqttc::v5::AsyncClient {
    type PublishError = rumqttc::v5::ClientError;
    type SubscribeError = rumqttc::v5::ClientError;
    async fn homie_publish(&self, p: Publish) -> Result<(), Self::PublishError> {
        self.publish(p.topic, convert_qos(p.qos), p.retain, p.payload)
            .await
    }

    async fn homie_subscribe(
        &self,
        subs: impl Iterator<Item = Subscription> + Send,
    ) -> Result<(), Self::SubscribeError> {
        self.subscribe_many(
            subs.map(|sub| {
                rumqttc::v5::mqttbytes::v5::Filter::new(sub.topic, convert_qos(sub.qos))
            }),
        )
        .await
    }
}

pub fn convert_qos(homie: QoS) -> rumqttc::v5::mqttbytes::QoS {
    match homie {
        QoS::AtMostOnce => rumqttc::v5::mqttbytes::QoS::AtMostOnce,
        QoS::AtLeastOnce => rumqttc::v5::mqttbytes::QoS::AtLeastOnce,
        QoS::ExactlyOnce => rumqttc::v5::mqttbytes::QoS::ExactlyOnce,
    }
}

#[derive(Debug)]
pub(crate) enum Command {
    Set {
        property: PropertyRef,
        value: String,
    },
}

impl Command {
    pub(crate) fn try_from_mqtt_command(
        msg: rumqttc::v5::mqttbytes::v5::Publish,
    ) -> Result<Self, rumqttc::v5::mqttbytes::v5::Publish> {
        let topic = str::from_utf8(&msg.topic).expect("TODO");
        match homie5::parse_mqtt_message(topic, &msg.payload) {
            Ok(homie5::Homie5Message::PropertySet {
                property,
                set_value,
            }) => Ok(Self::Set {
                property: property,
                value: set_value,
            }),
            _ => Err(msg),
        }
    }
}

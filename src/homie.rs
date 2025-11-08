mod alarm_node;
mod clock_node;
mod compensation_node;
mod demand_control_node;
mod fan_speed_setting_node;
mod node;
mod value;

use crate::connection::{self, Connection};
use crate::homie::node::Node;
use crate::modbus::{self, Operation, Request, ResponseKind};
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

pub enum EventResult {
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
}

type ModbusReadStream =
    SelectAll<Pin<Box<dyn Send + Sync + Stream<Item = Result<EventResult, connection::Error>>>>>;

pub(crate) struct SystemAirDevice {
    mqtt: rumqttc::v5::AsyncClient,
    protocol: Homie5DeviceProtocol,
    state: HomieDeviceStatus,
    description: HomieDeviceDescription,
    nodes: BTreeMap<HomieID, Box<dyn Node>>,
    modbus: Arc<Connection>,
    modbus_device_id: u8,
    modbus_values: ModbusDeviceValues,
    event_stream: ModbusReadStream,
    commands: mpsc::UnboundedReceiver<Command>,
}

impl SystemAirDevice {
    pub(crate) fn new(
        mqtt: rumqttc::v5::AsyncClient,
        protocol: Homie5DeviceProtocol,
        modbus: Arc<Connection>,
        modbus_device_id: u8,
        commands: mpsc::UnboundedReceiver<Command>,
    ) -> Self {
        let nodes = [
            Box::new(clock_node::ClockNode::new()) as Box<dyn Node>,
            Box::new(alarm_node::AlarmNode::new()) as Box<dyn Node>,
            Box::new(demand_control_node::DemandControlNode::new()) as _,
            Box::new(fan_speed_setting_node::FanSpeedSettingsNode::new()) as _,
            Box::new(compensation_node::CompensationNode::new()) as _,
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
            modbus_device_id,
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
                            Duration::from_secs(5),
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
        use std::sync::Mutex;
        let next_slot = Arc::new(Mutex::new(Instant::now()));
        let operation = Operation::GetHoldings { address, count };
        let modbus = Arc::clone(&self.modbus);
        let device_id = self.modbus_device_id;
        let stream = futures::stream::repeat(()).then(move |()| {
            let modbus = Arc::clone(&modbus);
            let next_slot = Arc::clone(&next_slot);
            let transaction_id = modbus.new_transaction_id();
            async move {
                loop {
                    {
                        let timeout = *next_slot.lock().unwrap_or_else(|e| e.into_inner());
                        tokio::time::sleep_until(timeout).await;
                    }
                    let outcome = modbus
                        .send(Request {
                            device_id,
                            transaction_id,
                            operation,
                        })
                        .await?;
                    let Some(result) = outcome else {
                        continue;
                    };
                    let mut next_slot = next_slot.lock().unwrap_or_else(|e| e.into_inner());
                    if result.is_server_busy() {
                        // IAM was busy with other requests. Give it some time…
                        // TODO: maybe add a flag to control this?
                        // TODO: configurable retry sleep time?
                        *next_slot = Instant::now() + Duration::from_millis(25);
                        continue;
                    }
                    *next_slot = Instant::now() + period;
                    let result = EventResult::Periodic {
                        operation,
                        response: result.kind,
                    };
                    return Ok::<_, connection::Error>(result);
                }
            }
        });
        self.event_stream.push(Box::pin(stream));
    }

    async fn handle_value_change(&mut self, node_id: &HomieID, prop_idx: usize) -> Result<(), ()> {
        let node = self.nodes.get(node_id).unwrap();
        let prop_id = &node.properties()[prop_idx].prop_id;
        let Some(pd) = self.description.get_property_by_id(node_id, prop_id) else {
            tracing::warn!(
                ?node_id,
                ?prop_id,
                "property change event without description"
            );
            return Ok(());
        };
        let val = node
            .property_value(prop_idx)
            .map(|v| v.value())
            .unwrap_or_default();
        let msg = self
            .protocol
            .publish_value(node_id, prop_id, val, pd.retained);
        self.mqtt.homie_publish(msg).await.expect("TODO");
        Ok(())
    }

    async fn handle_target_change(&mut self, node_id: &HomieID, prop_idx: usize) -> Result<(), ()> {
        let node = self.nodes.get(node_id).unwrap();
        let prop_id = &node.properties()[prop_idx].prop_id;
        let Some(pd) = self.description.get_property_by_id(node_id, &prop_id) else {
            tracing::warn!(
                ?node_id,
                ?prop_id,
                "property change event without description"
            );
            return Ok(());
        };
        let Some(tgt) = node.property_value(prop_idx).and_then(|v| v.target()) else {
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
        let mut changed_registers = RegisterBitmask::new();
        let (chunks, remainder) = values.as_chunks::<2>();
        if !remainder.is_empty() {
            tracing::warn!("response contains non-even number of bytes, modbus is misbehaving");
        }
        for (word, address) in chunks.iter().zip(address..) {
            let word = u16::from_be_bytes(*word);
            if self.modbus_values.set_value(address, word) {
                changed_registers.set(address);
            }
        }

        let mut changes = vec![];
        'next_node: for (node_id, node) in &mut self.nodes {
            for property in node.properties() {
                'next_register: for register in property.kind.registers() {
                    if !changed_registers.is_set(register.address()) {
                        continue 'next_register;
                    }
                    tracing::debug!(
                        %node_id,
                        address = register.address(),
                        "register change reloads node"
                    );
                    for (prop_idx, property) in node.properties().iter().enumerate() {
                        let old = node.property_value(prop_idx);
                        let new = property.kind.value_from_modbus(&self.modbus_values);
                        let node_id = node_id.clone();
                        let prop_id = &property.prop_id;
                        match (old, new) {
                            (None, None) => continue,
                            (Some(_), None) => todo!("erasing a value??"),
                            (_, Some(Err(error))) => {
                                tracing::debug!(
                                    %prop_id,
                                    ?error,
                                    "could not parse property from device"
                                );
                            }
                            (None, Some(Ok(new))) => {
                                let target_changed = new.target().is_some();
                                changes.push((node_id, prop_idx, new, true, target_changed));
                            }
                            (Some(old), Some(Ok(new))) => {
                                let value = new.value();
                                let target = new.target();
                                let val_changed = value != old.value();
                                let tgt_changed = target.is_some() && target != old.target();
                                if val_changed || tgt_changed {
                                    tracing::debug!(%prop_id, "property changed");
                                    changes.push((
                                        node_id,
                                        prop_idx,
                                        new,
                                        val_changed,
                                        tgt_changed,
                                    ));
                                }
                            }
                        }
                    }
                    continue 'next_node;
                }
            }
        }
        for (node_id, prop_idx, value, value_changed, target_changed) in changes {
            self.nodes
                .get_mut(&node_id)
                .unwrap()
                .set_property_value(prop_idx, value);
            if !inhibit_change_handling && value_changed {
                self.handle_value_change(&node_id, prop_idx).await?;
            }
            if !inhibit_change_handling && target_changed {
                self.handle_target_change(&node_id, prop_idx).await?;
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
                    tracing::warn!(%node_id, %prop_id, value, "set value could not be parsed");
                    self.handle_target_change(&node_id, prop_idx).await?;
                    self.handle_value_change(&node_id, prop_idx).await?;
                    return Ok(());
                };
                match property.kind {
                    node::PropertyKind::Register { register, .. } => {
                        let modbus = Arc::clone(&self.modbus);
                        let device_id = self.modbus_device_id;
                        let transaction_id = self.modbus.new_transaction_id();
                        self.event_stream
                            .push(Box::pin(futures::stream::once(async move {
                                loop {
                                    let address = register.address();
                                    // FIXME: this should probably generate modbus operations
                                    // straight away thus supporting aggregate values.
                                    let value = value.modbus().into_inner();
                                    let operation = Operation::SetHolding { address, value };
                                    let response = modbus
                                        .send(Request {
                                            device_id,
                                            transaction_id,
                                            operation,
                                        })
                                        .await?;
                                    let Some(response) = response else { continue };
                                    if response.exception_code().is_none() {
                                        tracing::debug!(%node_id, %prop_id, value, "property set");
                                        break;
                                    } else if response.is_server_busy() {
                                        continue;
                                    } else {
                                        return Ok(EventResult::HomieSet {
                                            node_id,
                                            prop_idx,
                                            operation,
                                            response: response.kind,
                                        });
                                    }
                                }
                                loop {
                                    let operation = Operation::GetHoldings {
                                        address: register.address(),
                                        count: 1,
                                    };
                                    let response = modbus
                                        .send(Request {
                                            device_id,
                                            transaction_id,
                                            operation,
                                        })
                                        .await?;
                                    let Some(response) = response else { continue };
                                    if response.is_server_busy() {
                                        continue;
                                    } else {
                                        return Ok(EventResult::HomieSet {
                                            node_id,
                                            prop_idx,
                                            operation,
                                            response: response.kind,
                                        });
                                    }
                                }
                            })));
                    }
                    node::PropertyKind::Action { .. } => {}
                    node::PropertyKind::Aggregate { .. } => {}
                }
            }
        }
        Ok(())
    }

    pub async fn step(&mut self) -> Result<(), ()> {
        loop {
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
                let r1 = self.handle_target_change(&node_id, prop_idx).await;
                let r2 = self.handle_value_change(&node_id, prop_idx).await;
                r1.and(r2)?;
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
                let r2 = self.handle_target_change(&node_id, prop_idx).await;
                let r3 = self.handle_value_change(&node_id, prop_idx).await;
                r1.and(r2).and(r3)?;
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

mod alarm_node;
mod common;
// mod compensation_node;
// mod demand_control_node;
// mod fan_speed_setting_node;
mod node;
mod read_stream;

use crate::connection::Connection;
use crate::homie::node::{Node, NodeEvent};
use crate::homie::read_stream::RegisterEvent;
use crate::modbus::{Response, ResponseKind};
use common::PropertyValue;
use futures::stream::SelectAll;
use futures::{Stream, StreamExt as _};
use homie5::client::{Publish, QoS, Subscription};
use homie5::device_description::HomieDeviceDescription;
use homie5::{Homie5DeviceProtocol, HomieDeviceStatus, HomieID};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::task::AbortOnDropHandle;

pub struct SystemAirDevice {
    mqtt: rumqttc::v5::AsyncClient,
    protocol: Homie5DeviceProtocol,
    state: HomieDeviceStatus,
    description: HomieDeviceDescription,
    #[allow(unused)] // exists for its drop handler
    device_read_task: AbortOnDropHandle<()>,
    read_events: Receiver<RegisterEvent>,
    nodes: BTreeMap<HomieID, Box<dyn Node>>,
    node_event_stream: SelectAll<BroadcastStream<NodeEvent>>,
}

impl SystemAirDevice {
    pub fn new(
        mqtt: rumqttc::v5::AsyncClient,
        protocol: Homie5DeviceProtocol,
        modbus: Arc<Connection>,
    ) -> Self {
        let nodes = [
            Box::new(alarm_node::AlarmNode::new()) as Box<dyn Node>,
            // Box::new(demand_control_node::DemandControlNode) as _,
            // Box::new(fan_speed_setting_node::FanSpeedSettingsNode) as _,
            // Box::new(compensation_node::CompensationNode) as _,
        ];
        let mut description =
            homie5::device_description::DeviceDescriptionBuilder::new().name("SystemAIR SAVE");
        for node in &nodes {
            description = description.add_node(node.node_id(), node.description());
        }
        let description = description.build();
        let (read_sender, read_events) =
            tokio::sync::broadcast::channel::<read_stream::RegisterEvent>(1024);
        let device_read_task = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
            let mut read_stream = read_stream::read_device(modbus);
            loop {
                let register_event = match read_stream.next().await {
                    None => return,
                    Some(e) => e,
                };
                tracing::trace!(
                    register.address = register_event.register.address(),
                    register.event = ?register_event.kind,
                    "read a register event"
                );
                let Ok(_) = read_sender.send(register_event) else {
                    return;
                };
            }
        }));
        let node_event_stream =
            SelectAll::from_iter(nodes.iter().map(|n| BroadcastStream::new(n.node_events())));
        let nodes = nodes.into_iter().map(|v| (v.node_id(), v)).collect();
        Self {
            mqtt,
            state: HomieDeviceStatus::Init,
            protocol,
            description,
            device_read_task,
            node_event_stream,
            read_events,
            nodes,
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
                    tracing::debug!("waiting for device read out…");
                    let mut all_populated = false;
                    while !all_populated {
                        self.step().await.expect("TODO");
                        all_populated = true;
                        for (_, node) in &self.nodes {
                            all_populated &= node.values_populated();
                        }
                    }
                    // rumqttc appears to be sending publishes in a weird order that results in
                    // some of the properties getting published *after* `$state = Ready` unless we
                    // yield here...
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

    async fn handle_node_event(&mut self, node_event: NodeEvent) -> Result<(), ()> {
        let Self {
            description,
            protocol,
            mqtt,
            ..
        } = self;
        match node_event {
            NodeEvent::PropertyChanged {
                node_id,
                prop_id,
                new,
            } => {
                let Some(pd) = description.get_property_by_id(&node_id, &prop_id) else {
                    tracing::warn!(
                        ?node_id,
                        ?prop_id,
                        "property change event without description"
                    );
                    return Ok(());
                };
                let val = new.value();
                let msg = protocol.publish_value(&node_id, &prop_id, val, pd.retained);
                mqtt.homie_publish(msg).await.expect("TODO");
            }
            NodeEvent::TargetChanged {
                node_id,
                prop_id,
                new,
            } => {
                let Some(pd) = description.get_property_by_id(&node_id, &prop_id) else {
                    tracing::warn!(
                        ?node_id,
                        ?prop_id,
                        "property change event without description"
                    );
                    return Ok(());
                };
                let val = new.target().unwrap_or_default();
                let msg = protocol.publish_target(&node_id, &prop_id, val, pd.retained);
                mqtt.homie_publish(msg).await.expect("TODO");
            }
        }
        Ok(())
    }

    async fn handle_read_event(&mut self, event: RegisterEvent) -> Result<(), ()> {
        let value = match event.kind {
            read_stream::RegisterEventKind::Value(value) => value,
            read_stream::RegisterEventKind::ReadError(read_stream_error) => {
                tracing::error!(err = ?read_stream_error, "reading error has occurred");
                return Ok(());
            }
            read_stream::RegisterEventKind::ServerException(exc_code) => {
                tracing::debug!(exc_code, "modbus server exception reported while reading");
                return Ok(());
            }
        };
        for (_, node) in &mut self.nodes {
            node.on_register_value(event.register, value);
        }
        Ok(())
    }

    pub async fn step(&mut self) -> Result<(), ()> {
        loop {
            tokio::select! {
                node_event = self.node_event_stream.next() => {
                    let node_event = match node_event {
                        // report an error upstream so everything cleans up and finishes execution
                        None => todo!(),
                        Some(Ok(event)) => event,
                        Some(Err(BroadcastStreamRecvError::Lagged(messages_lost))) => {
                            tracing::warn!(messages_lost, "node event handler fell behind");
                            continue;
                        }
                    };
                    return self.handle_node_event(node_event).await;
                },
                read_event = self.read_events.recv() => {
                    let event = match read_event {
                        Ok(event) => event,
                        // report an error upstream so everything cleans up and finishes execution
                        Err(RecvError::Closed) => {
                            todo!();
                        }
                        Err(RecvError::Lagged(count)) => {
                            tracing::warn!(count, "read event handler lagged");
                            continue;
                        }
                    };
                    return self.handle_read_event(event).await;
                },
            }
        }
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

#[derive(Debug, thiserror::Error)]
enum ReadStreamError {
    #[error("could not send a modbus request")]
    Send(#[source] crate::connection::Error),
}

enum PropertyEventKind {
    /// Value changed.
    PropertyValue(Box<dyn PropertyValue>),
    /// There was an error reading the value behind this property.
    ReadError(Arc<ReadStreamError>),
    /// There was a server exception indicated in the response.
    ServerException(u8),
}

impl PropertyEventKind {
    fn from_holdings_response<V: PropertyValue + 'static>(
        response: &Result<Response, Arc<ReadStreamError>>,
        holdings: impl FnOnce(&[u8]) -> V,
    ) -> Self {
        match response {
            Err(e) => return PropertyEventKind::ReadError(Arc::clone(e)),
            Ok(Response {
                kind: ResponseKind::ErrorCode(e),
                ..
            }) => return PropertyEventKind::ServerException(*e),
            Ok(Response {
                kind: ResponseKind::GetHoldings { values },
                ..
            }) => {
                let value = holdings(&values);
                return PropertyEventKind::PropertyValue(Box::new(value));
            }
        }
    }
}

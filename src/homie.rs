mod common;
mod read_stream;
mod alarm_node;
// mod compensation_node;
// mod demand_control_node;
// mod fan_speed_setting_node;

use crate::connection::Connection;
use crate::modbus::{extract_value, Operation, Request, Response, ResponseKind};
use crate::registers::{RegisterIndex, Value};
use common::PropertyValue;
use futures::stream::SelectAll;
use futures::{Stream, StreamExt as _};
use homie5::client::{Publish, QoS, Subscription};
use homie5::device_description::HomieDeviceDescription;
use homie5::{Homie5DeviceProtocol, HomieDeviceStatus, HomieID};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;

pub struct SystemAirDevice {
    mqtt: rumqttc::v5::AsyncClient,
    protocol: Homie5DeviceProtocol,
    state: HomieDeviceStatus,
    description: HomieDeviceDescription,
    read_stream: SelectAll<Pin<Box<dyn Stream<Item = read_stream::RegisterEvent>>>>,
    last_values: BTreeMap<(HomieID, HomieID), Option<Box<dyn PropertyValue>>>,
}

impl SystemAirDevice {
    pub fn new(
        mqtt: rumqttc::v5::AsyncClient,
        protocol: Homie5DeviceProtocol,
        modbus: Arc<Connection>,
    ) -> Self {
        let alarm_node_id = HomieID::new_const("alarm");
        let demc_node_id = HomieID::new_const("demand-control");
        let fan_speed_node_id = HomieID::new_const("fan-speed-settings");
        let compensation_node_id = HomieID::new_const("compensation");
        let description = homie5::device_description::DeviceDescriptionBuilder::new()
            .name("SystemAIR SAVE")
            .add_node(alarm_node_id.clone(), alarm_node::description())
            // .add_node(demc_node_id.clone(), demand_control_node::description())
            // .add_node(
            //     fan_speed_node_id.clone(),
            //     fan_speed_setting_node::description(),
            // )
            // .add_node(
            //     compensation_node_id.clone(),
            //     compensation_node::description(),
            // )
            .build();

        let mut last_values = BTreeMap::new();
        for (n, _, p, _) in description.iter() {
            last_values.insert((n.clone(), p.clone()), None);
        }
        Self {
            mqtt,
            state: HomieDeviceStatus::Init,
            protocol,
            description,
            read_stream: read_stream::read_device(modbus),
            last_values,
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
                    // while self.last_values.values().any(|v| v.is_none()) {
                    //     self.step().await;
                    // }
                }
                homie5::DevicePublishStep::SubscribeProperties => {
                    // TODO:
                    let p = self
                        .protocol
                        .subscribe_props(&self.description)
                        .expect("TODO");
                    println!("subscriptions for {:?}", self.description);
                    for s in p {
                        println!("sub {}", s.topic);
                    }
                    // self.mqtt.homie_subscribe(p).await?;
                }
                homie5::DevicePublishStep::DeviceStateReady => {
                    self.state = HomieDeviceStatus::Ready;
                    let p = self.protocol.publish_state(self.state);
                    self.mqtt.homie_publish(p).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn step(&mut self) {
        let register_event = match self.read_stream.next().await {
            None => return,
            Some(e) => e,
        };
        let register_index = register_event.register;
        let register_value = match register_event.kind {
            // For the most part ignorable, though may make sense to raise a homie alarm?
            read_stream::RegisterEventKind::ReadError(_) => return,
            read_stream::RegisterEventKind::ServerException(_) => return,
            read_stream::RegisterEventKind::Value(value) => value,
        };
        println!("{} = {register_value:?}", register_index.address());


            // Some(PropertyEvent {
            //     node_id,
            //     property_name: prop_id,
            //     kind,
            // }) => match kind {
            //     PropertyEventKind::PropertyValue(value) => {
            //         let (entry, prev) = match self.last_values.entry((node_id, prop_id)) {
            //             Entry::Vacant(ve) => {
            //                 let key = ve.key();
            //                 panic!("{}/{} was published but not in description", key.0, key.1);
            //             }
            //             Entry::Occupied(mut oe) => {
            //                 let previous = oe.get_mut().replace(value);
            //                 (oe, previous)
            //             }
            //         };
            //         let key = entry.key();
            //         let prop = entry
            //             .get()
            //             .as_ref()
            //             .expect("the statement above will have put `Some` in this entry");
            //         let (value_op, target_op) = self
            //             .description
            //             .with_property_by_id(&key.0, &key.1, |pd| {
            //                 let val = prop.value();
            //                 let prev_val = prev.as_ref().map(|p| p.value());
            //                 let val = (Some(&val) != prev_val.as_ref()).then(|| {
            //                     self.protocol
            //                         .publish_value(&key.0, &key.1, val, pd.retained)
            //                 });
            //                 let tgt = prop.target().and_then(|tgt| {
            //                     let prev_tgt = prev.and_then(|p| p.target());
            //                     (val.is_some() || Some(&tgt) != prev_tgt.as_ref()).then(|| {
            //                         self.protocol
            //                             .publish_target(&key.0, &key.1, tgt, pd.retained)
            //                     })
            //                 });
            //                 (val, tgt)
            //             })
            //             .expect("property should always be present in device description");
            //         if let Some(target_op) = target_op {
            //             self.mqtt.homie_publish(target_op).await.expect("TODO");
            //         }
            //         if let Some(value_op) = value_op {
            //             self.mqtt.homie_publish(value_op).await.expect("TODO");
            //         }
            //     }
            //     PropertyEventKind::ReadError(arc) => {
            //         // TODO: raise homie device alarm?
            //     }
            //     PropertyEventKind::ServerException(e) => {
            //         // TODO: raise homie device alarm?
            //     }
            // },
            // None => return,
        // }
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

struct PropertyEvent {
    node_id: HomieID,
    property_name: HomieID,
    kind: PropertyEventKind,
}


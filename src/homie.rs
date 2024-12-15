mod alarm_node;
mod demand_control_node;

use crate::connection::Connection;
use crate::modbus::{Operation, Request, Response};
use futures::stream::SelectAll;
use futures::{Stream, StreamExt as _};
use homie5::client::{Publish, QoS, Subscription};
use homie5::device_description::HomieDeviceDescription;
use homie5::{Homie5DeviceProtocol, HomieDeviceStatus, HomieID, HomieValue};
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tokio::time::Instant;

pub struct SystemAirDevice {
    mqtt: rumqttc::v5::AsyncClient,
    protocol: Homie5DeviceProtocol,
    state: HomieDeviceStatus,
    description: HomieDeviceDescription,
    read_stream: SelectAll<Pin<Box<dyn Stream<Item = PropertyEvent>>>>,
    last_values: BTreeMap<(HomieID, HomieID), Option<Box<dyn PropertyValue>>>,
}

impl SystemAirDevice {
    pub fn new(
        mqtt: rumqttc::v5::AsyncClient,
        protocol: Homie5DeviceProtocol,
        modbus: Arc<Connection>,
    ) -> Self {
        let alarm_node_id = HomieID::new_const("alarm");
        let demand_control_node_id = HomieID::new_const("demand-control");
        let description = homie5::device_description::DeviceDescriptionBuilder::new()
            .name("SystemAIR SAVE")
            .add_node(alarm_node_id.clone(), alarm_node::description())
            // .add_node(
            //     demand_control_node_id.clone(),
            //     demand_control_node::description(),
            // )
            .build();
        let mut read_stream = SelectAll::new();
        for stream in alarm_node::stream(alarm_node_id, Arc::clone(&modbus)) {
            read_stream.push(stream);
        }
        let mut last_values = BTreeMap::new();
        for (n, _, p, _) in description.iter() {
            last_values.insert((n.clone(), p.clone()), None);
        }
        Self {
            mqtt,
            state: HomieDeviceStatus::Init,
            protocol,
            description,
            read_stream,
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
                    while self.last_values.values().any(|v| v.is_none()) {
                        self.step().await;
                    }
                }
                homie5::DevicePublishStep::SubscribeProperties => {
                    let p = self
                        .protocol
                        .subscribe_props(&self.description)
                        .expect("TODO");
                    self.mqtt.homie_subscribe(p).await?;
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
        match self.read_stream.next().await {
            Some(PropertyEvent {
                node_id,
                property_name: prop_id,
                kind,
            }) => match kind {
                PropertyEventKind::PropertyValue(value) => {
                    let entry = match self.last_values.entry((node_id, prop_id)) {
                        Entry::Vacant(ve) => {
                            let key = ve.key();
                            panic!("{}/{} was published but not in description", key.0, key.1);
                        }
                        Entry::Occupied(mut oe) => {
                            match oe.get_mut() {
                                Some(o) => {
                                    // TODO: consider tracking value and target changes separately?
                                    if o.value() == value.value() && o.target() == value.target() {
                                        return; // No change, skip publishing.
                                    } else {
                                        *o = value;
                                    }
                                }
                                v @ None => {
                                    v.replace(value);
                                }
                            }
                            oe
                        }
                    };
                    let key = entry.key();
                    let value = entry
                        .get()
                        .as_ref()
                        .expect("the statement above will have put `Some` in this entry");
                    let (value_op, target_op) = self
                        .description
                        .with_property_by_id(&key.0, &key.1, |pd| {
                            let val = self.protocol.publish_value(
                                &key.0,
                                &key.1,
                                value.value(),
                                pd.retained,
                            );
                            let tgt = self.protocol.publish_target(
                                &key.0,
                                &key.1,
                                value.target(),
                                pd.retained,
                            );
                            (val, tgt)
                        })
                        .expect("property should always be present in device description");
                    self.mqtt.homie_publish(target_op).await.expect("TODO");
                    self.mqtt.homie_publish(value_op).await.expect("TODO");
                }
                PropertyEventKind::ReadError(arc) => {
                    // TODO: raise homie device alarm?
                }
                PropertyEventKind::ServerException(e) => {
                    // TODO: raise homie device alarm?
                }
            },
            None => return,
        }
    }

    //         match step {
    //             homie5::DevicePublishStep::DeviceStateInit => {
    //                 self.state = HomieDeviceStatus::Init;
    //                 let p = self.protocol.publish_state(self.state);

    //             }
    //             homie5::DevicePublishStep::DeviceDescription => {
    //                 self.publish_description().await?;
    //             }
    //             homie5::DevicePublishStep::PropertyValues => {
    //                 self.publish_property_values().await?;
    //             }
    //             homie5::DevicePublishStep::SubscribeProperties => {
    //                 self.subscribe_props().await?;
    //             }
    //             homie5::DevicePublishStep::DeviceStateReady => {
    //                 self.set_state(HomieDeviceStatus::Ready);
    //                 self.publish_state().await?;
    //             }
    //         }
    //     }
    //     Ok(())
    // }
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

trait PropertyValue {
    fn value(&self) -> String;
    fn target(&self) -> String;
}

enum PropertyEventKind {
    /// Value changed.
    PropertyValue(Box<dyn PropertyValue>),
    /// There was an error reading the value behind this property.
    ReadError(Arc<ReadStreamError>),
    /// There was a server exception indicated in the response.
    ServerException(u8),
}

struct PropertyEvent {
    node_id: HomieID,
    property_name: HomieID,
    kind: PropertyEventKind,
}

fn modbus_read_stream(
    modbus: Arc<Connection>,
    operation: Operation,
    period: Duration,
) -> impl Stream<Item = Result<Response, ReadStreamError>> {
    let next_slot = Arc::new(Mutex::new(Instant::now()));
    futures::stream::repeat(modbus.new_transaction_id()).then(move |transaction_id| {
        let modbus = Arc::clone(&modbus);
        let next_slot = Arc::clone(&next_slot);
        async move {
            loop {
                {
                    let timeout = *next_slot.lock().unwrap_or_else(|e| e.into_inner());
                    tokio::time::sleep_until(timeout).await;
                }
                let outcome = modbus
                    .send(Request {
                        device_id: 1,
                        transaction_id,
                        operation,
                    })
                    .await
                    .map_err(ReadStreamError::Send)?;
                {
                    let mut next_slot = next_slot.lock().unwrap_or_else(|e| e.into_inner());
                    *next_slot = Instant::now() + period;
                }
                let Some(result) = outcome else {
                    continue;
                };
                if result.is_server_busy() {
                    // IAM was busy with other requests. Give it some time…
                    // TODO: maybe add a flag to control this?
                    // TODO: configurable retries, sleep time?
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    continue;
                }
                return Ok::<_, ReadStreamError>(result);
            }
        }
    })
}

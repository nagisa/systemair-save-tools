mod alarm_node;
mod compensation_node;
mod demand_control_node;
mod fan_speed_setting_node;

use crate::connection::Connection;
use crate::modbus::{extract_value, Operation, Request, Response, ResponseKind};
use crate::registers::{RegisterIndex, Value};
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
        let demc_node_id = HomieID::new_const("demand-control");
        let fan_speed_node_id = HomieID::new_const("fan-speed-settings");
        let compensation_node_id = HomieID::new_const("compensation");
        let description = homie5::device_description::DeviceDescriptionBuilder::new()
            .name("SystemAIR SAVE")
            .add_node(alarm_node_id.clone(), alarm_node::description())
            .add_node(demc_node_id.clone(), demand_control_node::description())
            .add_node(
                fan_speed_node_id.clone(),
                fan_speed_setting_node::description(),
            )
            .add_node(
                compensation_node_id.clone(),
                compensation_node::description(),
            )
            .build();
        let mut read_stream = SelectAll::new();
        for stream in alarm_node::stream(alarm_node_id, Arc::clone(&modbus)) {
            read_stream.push(stream);
        }
        for stream in demand_control_node::stream(demc_node_id, Arc::clone(&modbus)) {
            read_stream.push(stream);
        }
        for stream in fan_speed_setting_node::stream(fan_speed_node_id, Arc::clone(&modbus)) {
            read_stream.push(stream);
        }
        for stream in compensation_node::stream(compensation_node_id, Arc::clone(&modbus)) {
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
                    let (entry, prev) = match self.last_values.entry((node_id, prop_id)) {
                        Entry::Vacant(ve) => {
                            let key = ve.key();
                            panic!("{}/{} was published but not in description", key.0, key.1);
                        }
                        Entry::Occupied(mut oe) => {
                            let previous = oe.get_mut().replace(value);
                            (oe, previous)
                        }
                    };
                    let key = entry.key();
                    let prop = entry
                        .get()
                        .as_ref()
                        .expect("the statement above will have put `Some` in this entry");
                    let (value_op, target_op) = self
                        .description
                        .with_property_by_id(&key.0, &key.1, |pd| {
                            let val = prop.value();
                            let prev_val = prev.as_ref().map(|p| p.value());
                            let val = (Some(&val) != prev_val.as_ref()).then(|| {
                                self.protocol
                                    .publish_value(&key.0, &key.1, val, pd.retained)
                            });
                            let tgt = prop.target().and_then(|tgt| {
                                let prev_tgt = prev.and_then(|p| p.target());
                                (val.is_some() || Some(&tgt) != prev_tgt.as_ref()).then(|| {
                                    self.protocol
                                        .publish_target(&key.0, &key.1, tgt, pd.retained)
                                })
                            });
                            (val, tgt)
                        })
                        .expect("property should always be present in device description");
                    if let Some(target_op) = target_op {
                        self.mqtt.homie_publish(target_op).await.expect("TODO");
                    }
                    if let Some(value_op) = value_op {
                        self.mqtt.homie_publish(value_op).await.expect("TODO");
                    }
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
    fn target(&self) -> Option<String>;
}

enum PropertyEventKind {
    /// Value changed.
    PropertyValue(Box<dyn PropertyValue>),
    /// There was an error reading the value behind this property.
    ReadError(Arc<ReadStreamError>),
    /// There was a server exception indicated in the response.
    ServerException(u8),
}

struct SimpleValue(Value);

impl PropertyValue for SimpleValue {
    fn value(&self) -> String {
        self.0.to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
}

struct BooleanValue(bool);

impl From<Value> for BooleanValue {
    fn from(value: Value) -> Self {
        BooleanValue(match value {
            Value::U16(v) => v != 0,
            Value::I16(v) => v != 0,
            Value::Celsius(v) => v != 0,
            Value::SpecificHumidity(v) => v != 0,
        })
    }
}

impl PropertyValue for BooleanValue {
    fn value(&self) -> String {
        self.0.to_string()
    }

    fn target(&self) -> Option<String> {
        None
    }
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
                let Some(result) = outcome else {
                    continue;
                };
                let mut next_slot = next_slot.lock().unwrap_or_else(|e| e.into_inner());
                if result.is_server_busy() {
                    // IAM was busy with other requests. Give it some time…
                    // TODO: maybe add a flag to control this?
                    // TODO: configurable retry sleep time?
                    *next_slot = Instant::now() + std::time::Duration::from_millis(25);
                    continue;
                }
                *next_slot = Instant::now() + period;
                return Ok::<_, ReadStreamError>(result);
            }
        }
    })
}

fn modbus_read_stream_flatmap<F, S>(
    modbus: &Arc<Connection>,
    operation: Operation,
    period: Duration,
    mut f: F,
) -> impl Stream<Item = PropertyEvent>
where
    F: FnMut(Result<Response, Arc<ReadStreamError>>) -> S,
    S: Stream<Item = PropertyEvent>,
{
    let stream = modbus_read_stream(Arc::clone(modbus), operation, period);
    stream.flat_map(move |vs| {
        let vs = vs.map_err(Arc::new);
        f(vs)
    })
}

fn modbus_read_stream_flatmap_registers<R, F>(
    modbus: &Arc<Connection>,
    operation: Operation,
    period: Duration,
    node_id: &HomieID,
    registers: R,
) -> impl Stream<Item = PropertyEvent>
where
    R: IntoIterator<Item = (RegisterIndex, HomieID, F)> + Clone,
    F: FnOnce(Value) -> Box<dyn PropertyValue>,
{
    let node_id = node_id.clone();
    let start_address = match operation {
        Operation::GetHoldings { address, count: _ } => address,
    };
    modbus_read_stream_flatmap(modbus, operation, period, move |vs| {
        let node_id = node_id.clone();
        futures::stream::iter(
            registers
                .clone()
                .into_iter()
                .map(move |(ri, prop_id, value_cvt)| {
                    let node_id = node_id.clone();
                    let kind = match &vs {
                        Err(e) => PropertyEventKind::ReadError(Arc::clone(&e)),
                        Ok(Response {
                            kind: ResponseKind::ErrorCode(e),
                            ..
                        }) => PropertyEventKind::ServerException(*e),
                        Ok(Response {
                            kind: ResponseKind::GetHoldings { values },
                            ..
                        }) => {
                            let Some(value) = extract_value(start_address, ri.address(), &values)
                            else {
                                todo!("some sensible error here");
                            };
                            PropertyEventKind::PropertyValue(value_cvt(value))
                        }
                    };
                    PropertyEvent {
                        node_id: node_id.clone(),
                        property_name: prop_id.clone(),
                        kind,
                    }
                }),
        )
    })
}

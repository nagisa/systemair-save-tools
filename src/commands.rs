pub mod registers {
    use crate::registers::{Mode, Value};

    #[derive(clap::ValueEnum, Clone, Debug)]
    pub enum Format {
        Table,
        Json,
        Csv,
    }

    /// Search and output known modbus registers.
    #[derive(clap::Parser)]
    pub struct Args {
        filter: Option<String>,
        #[clap(flatten)]
        output: crate::output::Args,
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error(transparent)]
        CreateOutput(crate::output::Error),
        #[error(transparent)]
        WriteOutput(crate::output::Error),
        #[error(transparent)]
        CommitOutput(crate::output::Error),
    }

    #[derive(serde::Serialize)]
    pub struct RegisterSchema {
        pub address: u16,
        pub name: &'static str,
        pub mode: Mode,
        pub signed: bool,
        pub scale: u8,
        pub minimum: Option<Value>,
        pub maximum: Option<Value>,
        pub description: &'static str,
    }

    impl RegisterSchema {
        pub fn all_registers() -> impl Iterator<Item = Self> {
            use crate::registers::*;
            use std::iter::zip;
            zip(
                zip(
                    zip(
                        zip(zip(zip(ADDRESSES, NAMES), MODES), DATA_TYPES),
                        MINIMUM_VALUES,
                    ),
                    MAXIMUM_VALUES,
                ),
                DESCRIPTIONS,
            )
            .map(
                |(
                    (((((&address, &name), &mode), &data_type), &minimum), &maximum),
                    &description,
                )| {
                    RegisterSchema {
                        address,
                        name,
                        mode,
                        signed: data_type.is_signed(),
                        scale: data_type.scale(),
                        minimum,
                        maximum,
                        description,
                    }
                },
            )
        }

        pub fn is_match(&self, pattern: &str) -> bool {
            let pattern = pattern.to_uppercase();
            if self.name.contains(&pattern) {
                return true;
            }
            if self.description.to_uppercase().contains(&pattern) {
                return true;
            }
            if self.address.to_string().contains(&pattern) {
                return true;
            }
            return false;
        }
    }

    pub fn run(args: Args) -> Result<(), Error> {
        let mut output = args.output.to_output().map_err(Error::CreateOutput)?;
        output
            .table_headers(vec![
                "Address",
                "Name",
                "Mode",
                "Type",
                "Scale",
                "Min",
                "Max",
                "Description",
            ])
            .map_err(Error::WriteOutput)?;
        for register in RegisterSchema::all_registers() {
            if let Some(pattern) = &args.filter {
                if !register.is_match(&pattern) {
                    continue;
                }
            }
            let register = &register;
            output
                .result(
                    || {
                        vec![
                            register.address.to_string(),
                            register.name.to_string(),
                            register.mode.to_string(),
                            if register.signed {
                                "i16".to_string()
                            } else {
                                "u16".to_string()
                            },
                            register.scale.to_string(),
                            register.minimum.map(|v| v.to_string()).unwrap_or_default(),
                            register.maximum.map(|v| v.to_string()).unwrap_or_default(),
                            register.description.to_string(),
                        ]
                    },
                    || register,
                )
                .map_err(Error::WriteOutput)?;
        }
        output.commit().map_err(Error::CommitOutput)
    }
}

pub mod read {
    use crate::connection::{self, Connection};
    use crate::modbus::{Operation, Request, ResponseKind};
    use crate::output;
    use crate::registers::{DataType, RegisterIndex};
    use futures::{StreamExt as _, TryStreamExt};
    use std::collections::VecDeque;
    use std::fmt::Write as _;
    use std::future::Future;
    use std::num::ParseIntError;

    #[derive(clap::ValueEnum, Clone)]
    pub enum Format {
        Simple,
        Table,
        Json,
        Csv,
    }

    /// Read the value stored in the specified register.
    #[derive(clap::Parser)]
    pub struct Args {
        #[arg(required = true)]
        pub(super) registers: Vec<String>,
        #[arg(long, short = 'i', default_value = "1")]
        pub(super) device_id: u8,
        #[clap(flatten)]
        pub(super) connection: connection::Args,
        #[clap(flatten)]
        pub(super) output: output::Args,
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("could not create an asynchronous execution runtime")]
        CreateAsyncRuntime(#[source] std::io::Error),
        #[error("could establish client connection with the device")]
        EstablishClient(#[source] crate::connection::Error),
        #[error("register range start `{1}` could not be parsed")]
        RegisterRangeStartParse(#[source] ParseIntError, String),
        #[error("register range end `{1}` could not be parsed")]
        RegisterRangeEndParse(#[source] ParseIntError, String),
        #[error("register range {0}..{1} is empty")]
        RegisterRangeEmpty(u16, u16),
        #[error("register `{0}` does not match any known register")]
        RegisterNotFound(String),
        #[error("communication with the device failed")]
        Communicate(#[source] crate::connection::Error),
        #[error(transparent)]
        CreateOutput(crate::output::Error),
        #[error(transparent)]
        WriteOutput(crate::output::Error),
        #[error(transparent)]
        CommitOutput(crate::output::Error),
    }

    #[derive(serde::Serialize)]
    struct OutputSchema {
        address: u16,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<&'static str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        values: Option<Vec<crate::registers::Value>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        exception: Option<u8>,
    }

    enum ReadRequest {
        SingleRegister {
            address: u16,
            index: Option<RegisterIndex>,
        },
        RegisterRange {
            address_start: u16,
            address_end: u16,
        },
    }

    impl ReadRequest {
        fn to_operation(&self) -> Operation {
            match self {
                ReadRequest::SingleRegister { address, index: _ } => Operation::GetHoldings {
                    address: *address,
                    count: 1,
                },
                ReadRequest::RegisterRange {
                    address_start,
                    address_end,
                } => Operation::GetHoldings {
                    address: *address_start,
                    count: address_end
                        .checked_sub(*address_start)
                        .expect("no overflow"),
                },
            }
        }
    }

    #[tokio::main(flavor = "current_thread")]
    pub async fn run(args: Args) -> Result<(), Error> {
        let Args {
            registers,
            device_id,
            connection,
            output,
        } = args;
        run_with_connection(&registers, device_id, output, async move {
            Connection::new(connection)
                .await
                .map_err(Error::Communicate)
        })
        .await
    }

    pub async fn run_with_connection(
        registers: &[String],
        device_id: u8,
        output: output::Args,
        connection: impl Future<Output = Result<Connection, Error>>,
    ) -> Result<(), Error> {
        let register_indices = registers
            .iter()
            .map(|register| {
                if let Ok(address) = register.parse::<u16>() {
                    let index = RegisterIndex::from_address(address);
                    return Ok(ReadRequest::SingleRegister { address, index });
                }
                if let Some((l, r)) = register.split_once("..") {
                    let address_start = l
                        .parse::<u16>()
                        .map_err(|e| Error::RegisterRangeStartParse(e, l.to_string()))?;
                    let address_end = r
                        .parse::<u16>()
                        .map_err(|e| Error::RegisterRangeEndParse(e, r.to_string()))?;
                    if address_end <= address_start {
                        return Err(Error::RegisterRangeEmpty(address_start, address_end));
                    }
                    return Ok(ReadRequest::RegisterRange {
                        address_start,
                        address_end,
                    });
                }
                if let Some(i) = RegisterIndex::from_name(register) {
                    return Ok(ReadRequest::SingleRegister {
                        address: i.address(),
                        index: Some(i),
                    });
                }

                Err(Error::RegisterNotFound(register.clone()))
            })
            .collect::<Result<VecDeque<_>, _>>()?;
        let mut output = output.to_output().map_err(Error::CreateOutput)?;
        let heads = vec!["Tx ID", "Address", "Name", "Response"];
        output.table_headers(heads).map_err(Error::WriteOutput)?;
        let connection = connection.await?;
        let mut stream = futures::stream::iter(register_indices.iter())
            .map(|read_request| {
                let connection = &connection;
                Ok::<_, Error>(async move {
                    let transaction_id = connection.new_transaction_id();
                    loop {
                        let outcome = connection
                            .send(Request {
                                device_id,
                                transaction_id,
                                operation: read_request.to_operation(),
                            })
                            .await
                            .map_err(Error::Communicate)?;
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
                        return Ok::<_, Error>((read_request, result));
                    }
                })
            })
            .try_buffered(2);
        while let Some(response) = stream.next().await {
            let (read_request, response) = &response?;
            let responses = match read_request {
                ReadRequest::SingleRegister { address, index } => vec![(*address, *index, 0)],
                ReadRequest::RegisterRange {
                    address_start,
                    address_end,
                } => (*address_start..*address_end)
                    .zip((0..).step_by(2))
                    .map(|(address, value_offset)| {
                        (address, RegisterIndex::from_address(address), value_offset)
                    })
                    .collect(),
            };
            for (address, register_index, value_offset) in responses {
                output
                    .result(
                        || {
                            let (dt, name) = register_index
                                .map(|r| (r.data_type(), r.name()))
                                .unwrap_or((DataType::U16, "???"));
                            let value = match &response.kind {
                                ResponseKind::ErrorCode(c) => format!("server exception {c}"),
                                ResponseKind::GetHoldings { values } => {
                                    let mut buf = String::new();
                                    let range = value_offset..(value_offset + dt.bytes());
                                    for value in dt.from_bytes(&values[range]) {
                                        buf.write_fmt(format_args!("{} ", value)).unwrap();
                                    }
                                    buf
                                }
                                ResponseKind::SetHolding { .. } => {
                                    "SetHolding response?".to_string()
                                }
                            };
                            vec![
                                response.transaction_id.to_string(),
                                address.to_string(),
                                name.to_string(),
                                value,
                            ]
                        },
                        || {
                            let name = register_index.map(|r| r.name());
                            let dt = register_index
                                .map(|r| r.data_type())
                                .unwrap_or(DataType::U16);
                            let (values, exception) = match &response.kind {
                                ResponseKind::ErrorCode(e) => (None, Some(*e)),
                                ResponseKind::GetHoldings { values } => {
                                    let range = value_offset..(value_offset + dt.bytes());
                                    (Some(dt.from_bytes(&values[range]).collect()), None)
                                }
                                ResponseKind::SetHolding { .. } => (None, None),
                            };
                            OutputSchema {
                                address,
                                name,
                                values,
                                exception,
                            }
                        },
                    )
                    .map_err(Error::WriteOutput)?;
            }
        }
        output.commit().map_err(Error::CommitOutput)
    }
}

pub mod write {
    use std::sync::Arc;

    use crate::{
        commands::registers,
        connection::{self, Connection},
        modbus::{Operation, Request, Response, ResponseKind},
        registers::{ParseValueError, RegisterIndex},
    };

    /// Write the values into specified registers.
    #[derive(clap::Parser)]
    pub struct Args {
        #[arg(required = true)]
        registers: Vec<String>,
        #[arg(long, short = 'i', default_value = "1")]
        device_id: u8,
        #[arg(long)]
        no_read_back: bool,
        #[clap(flatten)]
        output: crate::output::Args,
        #[clap(flatten)]
        connection: connection::Args,
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("could not read back the written registers")]
        Readback(#[source] super::read::Error),
        #[error("could not parse {0} into a register description and value to write (expected format `REG=VAL`)")]
        RegisterMalformed(String),
        #[error("register address {0} is not known")]
        RegisterAddressUnknown(u16),
        #[error("register {0} is not known")]
        RegisterNotFound(String),
        #[error("could not parse value {0} for register {1}")]
        ParseValue(String, String, #[source] ParseValueError),
        #[error("communication with the device failed")]
        Communicate(#[source] crate::connection::Error),
    }

    #[tokio::main(flavor = "current_thread")]
    pub async fn run(args: Args) -> Result<(), Error> {
        let mut readback_registers = vec![];
        let mut write_ops = vec![];
        for register in args.registers {
            let Some((register, value)) = register.split_once("=") else {
                return Err(Error::RegisterMalformed(register));
            };
            readback_registers.push(register.to_string());
            let register_index = if let Ok(address) = register.parse::<u16>() {
                let Some(i) = RegisterIndex::from_address(address) else {
                    return Err(Error::RegisterAddressUnknown(address));
                };
                i
            } else if let Some(i) = RegisterIndex::from_name(register) {
                i
            } else {
                return Err(Error::RegisterNotFound(register.to_string()));
            };
            if !register_index.mode().is_writable() {
                tracing::warn!(register, "not writable, will try writing anyway…!")
            }
            let value = register_index
                .data_type()
                .parse_string(value)
                .map_err(|e| Error::ParseValue(value.into(), register.into(), e))?;
            write_ops.push((register_index, value));
        }
        let connection = Connection::new(args.connection.clone()).await.unwrap();
        for (register, val) in write_ops {
            let transaction_id = connection.new_transaction_id();
            let outcome = connection
                .send(Request {
                    device_id: 1,
                    transaction_id,
                    operation: Operation::SetHolding {
                        address: register.address(),
                        value: val.into_inner(),
                    },
                })
                .await
                .map_err(Error::Communicate)?;
            let address = register.address();
            match outcome {
                Some(Response {
                    kind: ResponseKind::ErrorCode(c),
                    ..
                }) => {
                    tracing::warn!(
                        address,
                        exception = c,
                        "device responded with an exception code to a set command"
                    )
                }
                Some(Response {
                    kind: ResponseKind::SetHolding { value },
                    ..
                }) => {
                    // IAM seems to be returning garbage in `value` here? Maybe even a memory read
                    // primitive?
                    tracing::info!(address, value, "register set")
                }
                Some(Response { kind: _, .. }) => {
                    tracing::warn!(address, "unexpected response to a set command")
                }
                None => {
                    tracing::warn!(address, "no response to set register command")
                }
            }
        }

        if !args.no_read_back {
            super::read::run_with_connection(
                &readback_registers,
                args.device_id,
                args.output,
                async move { Ok(connection) },
            )
            .await
            .map_err(Error::Readback)?;
        }
        Ok(())
    }
}

pub mod mqtt {
    use crate::connection::Connection;
    use crate::homie::Command;
    use crate::{connection, homie};
    use rumqttc::v5::mqttbytes::v5::{LastWill, Publish};
    use rumqttc::v5::{AsyncClient, Incoming, MqttOptions};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    /// Start a SystemAIR to MQTT proxy which exposes a homie interface to the HVAC device
    #[derive(clap::Parser)]
    pub struct Args {
        #[clap(flatten)]
        connection: connection::Args,

        /// How to connect to the MQTT broker.
        ///
        /// The value is expected to be provided as an URL, such as:
        /// `mqtt://location:1883?client_id=hostname` for plain text connection or
        /// `mqtts://location:1883?client_id=hostname` for TLS protected connection.
        #[clap(short = 'm', long)]
        mqtt_broker: String,

        /// To be provided together with `--mqtt-password` to use password based authentication
        /// with the broker.
        #[clap(short = 'u', long, requires = "mqtt_password")]
        mqtt_user: Option<String>,

        /// To be provided together with `--mqtt-user` to use password based authentication with
        /// the broker.
        #[clap(short = 'p', long, requires = "mqtt_user")]
        mqtt_password: Option<String>,

        #[clap(long, default_value = "homie/systemair")]
        mqtt_topic_base: String,

        #[clap(long, default_value = "systemair-save-tools")]
        device_name: String,
    }

    impl Args {
        fn topic(&self, suffix: &str) -> String {
            format!("{}/{}", self.mqtt_topic_base, suffix)
        }
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {}

    #[tokio::main(flavor = "current_thread")]
    pub async fn run(args: Args) -> Result<(), Error> {
        let mut mqtt_options = MqttOptions::parse_url(&args.mqtt_broker).expect("TODO");
        if let Some((u, p)) = args
            .mqtt_user
            .as_ref()
            .and_then(|u| Some((u, args.mqtt_password.as_ref()?)))
        {
            mqtt_options.set_credentials(u, p);
        }

        let connection = Arc::new(Connection::new(args.connection).await.unwrap());

        let (protocol, last_will) = homie5::Homie5DeviceProtocol::new(
            "systemair-save".try_into().expect("TODO"),
            homie5::HomieDomain::Default,
        );
        mqtt_options.set_last_will(LastWill::new(
            last_will.topic,
            last_will.message,
            homie::convert_qos(last_will.qos),
            last_will.retain,
            None,
        ));
        let (client, mut client_loop) = AsyncClient::new(mqtt_options, 100);
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let mut device = homie::SystemAirDevice::new(client, protocol, connection, command_rx);

        {
            let mut publish_future = std::pin::pin!(device.publish_device());
            loop {
                tokio::select! {
                    biased;
                    result = client_loop.poll() => {
                        result.expect("TODO");
                    }
                    result = &mut publish_future => {
                        break result.expect("TODO");
                    }
                }
            }
        }
        loop {
            let mqtt_event = tokio::select! {
                biased;
                result = client_loop.poll() => result.expect("TODO"),
                result = device.step() => {
                    result.expect("TODO");
                    continue;
                }
            };
            match mqtt_event {
                rumqttc::v5::Event::Incoming(Incoming::Publish(msg)) => {
                    match Command::try_from_mqtt_command(msg) {
                        Ok(cmd) => command_tx.send(cmd).expect("TODO"),
                        Err(unexpected) => {
                            tracing::info!(?unexpected, "unexpected mqtt message received")
                        }
                    }
                }
                rumqttc::v5::Event::Incoming(event) => {
                    tracing::debug!(?event, "incoming mqtt event not handled");
                }
                rumqttc::v5::Event::Outgoing(_) => {}
            }
        }
    }
}

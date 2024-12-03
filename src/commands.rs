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
    use crate::registers::{DataType, ADDRESSES, DATA_TYPES, NAMES};
    use futures::{StreamExt as _, TryStreamExt};
    use std::collections::VecDeque;
    use std::fmt::Write as _;
    use tracing::warn;

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
        registers: Vec<String>,
        #[arg(long, short = 'i', default_value = "1")]
        device_id: u8,
        #[clap(flatten)]
        connection: connection::Args,
        #[clap(flatten)]
        output: output::Args,
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("could not create an asynchronous execution runtime")]
        CreateAsyncRuntime(#[source] std::io::Error),
        #[error("could establish client connection with the device")]
        EstablishClient(#[source] crate::connection::Error),
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

    pub fn run(args: Args) -> Result<(), Error> {
        let register_indices = args
            .registers
            .iter()
            .map(|register| {
                if let Ok(reg_address) = register.parse::<u16>() {
                    let index = ADDRESSES.partition_point(|v| *v < reg_address);
                    if ADDRESSES[index] != reg_address {
                        warn!(message = "register address unknown", reg_address);
                        return Ok((reg_address, None));
                    }
                    return Ok((reg_address, Some(index)));
                }
                Err(Error::RegisterNotFound(register.clone()))
            })
            .collect::<Result<VecDeque<_>, _>>()?;
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(Error::CreateAsyncRuntime)?;
        rt.block_on(async move {
            let mut output = args.output.to_output().map_err(Error::CreateOutput)?;
            let heads = vec!["Tx ID", "Address", "Name", "Response"];
            output.table_headers(heads).map_err(Error::WriteOutput)?;

            let connection = Connection::new(args.connection).await.unwrap();
            let mut stream = futures::stream::iter(register_indices.iter().enumerate())
                .map(|(i, (address, regi))| {
                    let connection = &connection;
                    Ok::<_, Error>(async move {
                        loop {
                            let outcome = connection
                                .send(Request {
                                    device_id: 1,
                                    transaction_id: i as u16,
                                    operation: Operation::GetHoldings {
                                        address: *address,
                                        count: 1,
                                    },
                                })
                                .await
                                .map_err(Error::Communicate)?;
                            let Some(result) = outcome else {
                                continue;
                            };
                            return Ok::<_, Error>((*address, *regi, result));
                        }
                    })
                })
                .try_buffered(5);
            while let Some(response) = stream.next().await {
                let (address, regi, response) = &response?;
                output
                    .result(
                        || {
                            let (dt, name) = regi
                                .map(|r| (DATA_TYPES[r], NAMES[r]))
                                .unwrap_or((DataType::U16, "???"));
                            let value = describe_response_kind(&response.kind, dt);
                            vec![
                                response.transaction_id.to_string(),
                                address.to_string(),
                                name.to_string(),
                                value,
                            ]
                        },
                        || {
                            let name = regi.map(|r| NAMES[r]);
                            let address = *address;
                            let dt = regi.map(|r| DATA_TYPES[r]).unwrap_or(DataType::U16);
                            let (values, exception) = match &response.kind {
                                ResponseKind::ErrorCode(e) => (None, Some(*e)),
                                ResponseKind::GetHoldings { values } => {
                                    (Some(dt.from_bytes(&values).collect()), None)
                                }
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
            output.commit().map_err(Error::CommitOutput)
        })
    }

    fn describe_response_kind(rk: &ResponseKind, dt: DataType) -> String {
        match rk {
            ResponseKind::ErrorCode(c) => format!("server exception {c}"),
            ResponseKind::GetHoldings { values } => {
                let mut buf = String::new();
                for value in dt.from_bytes(&values) {
                    buf.write_fmt(format_args!("{}, ", value)).unwrap();
                }
                buf
            }
        }
    }
}

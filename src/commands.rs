use crate::registers::DataType;

pub mod registers {
    use std::path::PathBuf;

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
        #[arg(long, short='f', value_enum, default_value_t = Format::Table)]
        format: Format,
        filter: Option<String>,
        #[arg(long, short = 'o')]
        file: Option<PathBuf>,
    }

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("could not open the specified output file at {1:?}")]
        OpenOutputFile(#[source] std::io::Error, PathBuf),
        #[error("could not write data to the output file at {1:?}")]
        WriteFile(#[source] std::io::Error, PathBuf),
        #[error("could not write data to the terminal")]
        WriteStdout(#[source] std::io::Error),
        #[error("could not serialize registers to JSON")]
        SerializeJson(#[source] serde_json::Error),
        #[error("could not serialize registers to CSV")]
        SerializeCsv(#[source] csv::Error),
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
        let mut output_writer: Box<dyn std::io::Write> = match &args.file {
            None => Box::new(std::io::stdout().lock()) as Box<_>,
            Some(path) => Box::new(
                std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(path)
                    .map_err(|e| Error::OpenOutputFile(e, path.clone()))?,
            ) as Box<_>,
        };

        let data = match args.format {
            Format::Table => {
                let mut table = comfy_table::Table::new();
                let header = vec![
                    "Address",
                    "Name",
                    "Mode",
                    "Type",
                    "Scale",
                    "Min",
                    "Max",
                    "Description",
                ];
                table
                    .set_header(header)
                    .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
                for register in RegisterSchema::all_registers() {
                    if let Some(pattern) = &args.filter {
                        if !register.is_match(&pattern) {
                            continue;
                        }
                    }
                    table.add_row(vec![
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
                    ]);
                }
                table.to_string().into_bytes()
            }
            Format::Json => {
                let value = RegisterSchema::all_registers().collect::<Vec<_>>();
                serde_json::to_vec(&value).map_err(Error::SerializeJson)?
            }
            Format::Csv => {
                let mut bytes = Vec::new();
                let mut writer = csv::Writer::from_writer(&mut bytes);
                for register in RegisterSchema::all_registers() {
                    writer.serialize(register).map_err(Error::SerializeCsv)?;
                }
                drop(writer);
                bytes
            }
        };
        output_writer.write(&data).map_err(|e| match args.file {
            None => Error::WriteStdout(e),
            Some(p) => Error::WriteFile(e, p),
        })?;
        Ok(())
    }
}

pub mod read {
    use crate::connection::{Connection, ConnectionArgs};
    use crate::modbus::{Operation, Request, Response, ResponseKind};
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
        #[arg(long, short='f', value_enum, default_value_t = Format::Table)]
        format: Format,
        #[arg(required = true)]
        registers: Vec<String>,
        #[arg(long, short = 'i', default_value = "1")]
        device_id: u8,
        #[clap(flatten)]
        connection: ConnectionArgs,
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

            match args.format {
                Format::Simple => {
                    while let Some(response) = stream.next().await {
                        let (address, regi, response) = response?;
                        let (dt, name) = regi
                            .map(|r| (DATA_TYPES[r], NAMES[r]))
                            .unwrap_or((DataType::U16, "???"));
                        let value = describe_response_kind(&response.kind, dt);
                        println!(
                            "{:},{address},{name},{value}",
                            response.transaction_id
                        );
                    }
                    return Ok(());
                }
                Format::Json => {}
                Format::Table => {
                    let mut table = comfy_table::Table::new();
                    let header = vec!["Address", "Name", "Value"];
                    table
                        .set_header(header)
                        .set_content_arrangement(comfy_table::ContentArrangement::Dynamic);
                    for result in stream.collect::<Vec<_>>().await {
                        let (address, regi, response) = result?;
                        let (dt, name) = regi
                            .map(|r| (DATA_TYPES[r], NAMES[r]))
                            .unwrap_or((DataType::U16, "???"));
                        let value = describe_response_kind(&response.kind, dt);
                        table.add_row(vec![format!("{address}"), format!("{name}"), value]);
                    }
                    println!("{table}");
                }
                Format::Csv => {}
            };

            Ok(())
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

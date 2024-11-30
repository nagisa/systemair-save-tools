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
                        signed: data_type.signed,
                        scale: data_type.scale,
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
                        if register.signed { "i16".to_string() } else { "u16".to_string() },
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

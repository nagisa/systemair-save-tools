use clap::Parser as _;
use std::error::Error as _;
use systemair_save_tools::commands;

#[derive(clap::Parser)]
#[clap(version, about, author)]
enum Commands {
    Registers(commands::registers::Args),
}

fn main() {
    let result = match Commands::parse() {
        Commands::Registers(args) => commands::registers::run(args),
    };
    std::process::exit(match result {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            let mut cause = e.source();
            while let Some(e) = cause {
                eprintln!("  because: {e}");
                cause = e.source();
            }
            1
        }
    });
}

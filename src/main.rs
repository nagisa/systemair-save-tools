use clap::Parser as _;
use systemair_save_tools::commands;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[derive(clap::Parser)]
#[clap(version, about, author)]
enum Commands {
    Registers(commands::registers::Args),
    Read(commands::read::Args),
}

fn end<E: std::error::Error>(r: Result<(), E>) {
    std::process::exit(match r {
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

fn main() {
    let filter_description = std::env::var("SYSTEMAIR_SAVE_TOOLS_LOG").expect("foo");
    let filter = filter_description
        .parse::<tracing_subscriber::filter::targets::Targets>()
        .expect("foo");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(filter)
        .init();
    match Commands::parse() {
        Commands::Registers(args) => end(commands::registers::run(args)),
        Commands::Read(args) => end(commands::read::run(args)),
    }
}

use clap::Parser as _;
use systemair_save_tools::commands;
use tracing_subscriber::EnvFilter;

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
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_env("SYSTEMAIR_SAVE_TOOLS_LOG"))
        .init();
    match Commands::parse() {
        Commands::Registers(args) => end(commands::registers::run(args)),
        Commands::Read(args) => end(commands::read::run(args)),
    }
}

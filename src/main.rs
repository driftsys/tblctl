mod cli;

use clap::Parser;
use cli::{Cli, Command};

const TRACKING: &str = "https://github.com/driftsys/tblctl/issues/1";

fn not_yet_implemented(name: &str) -> anyhow::Result<i32> {
    eprintln!("{name}: not yet implemented (tracking: {TRACKING})");
    Ok(2)
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Classify { .. } => not_yet_implemented("classify"),
        Command::Import { .. } => not_yet_implemented("import"),
        Command::Check { .. } => not_yet_implemented("check"),
        Command::Export { .. } => not_yet_implemented("export"),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("tblctl: {err:#}");
            std::process::exit(2);
        }
    }
}

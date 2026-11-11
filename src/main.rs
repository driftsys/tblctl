mod check;
mod cli;
mod markdown;

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
        Command::Check {
            files,
            json,
            strict,
            max_rows,
            max_cols,
            max_cell,
        } => check::run(
            &files,
            &check::Limits {
                max_rows,
                max_cols,
                max_cell,
            },
            json,
            strict,
        ),
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

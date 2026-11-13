mod check;
mod cli;
mod commands;
mod error;
mod markdown;
mod output;
mod read;
mod rules;
mod table;
mod write;

use clap::Parser;
use cli::{Cli, Command, LimitArgs};
use rules::Limits;

fn limits(a: LimitArgs) -> Limits {
    Limits {
        max_rows: a.max_rows,
        max_cols: a.max_cols,
        max_cell: a.max_cell,
    }
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Classify {
            files,
            json,
            limits: l,
        } => commands::classify(&files, &limits(l), json),
        Command::Import {
            file,
            sheet,
            out,
            as_format,
            json,
            limits: l,
        } => commands::import(
            &file,
            sheet.as_deref(),
            &out,
            as_format.map(|f| match f {
                cli::AsFormat::Md => rules::Target::Markdown,
                cli::AsFormat::Csv => rules::Target::Csv,
                cli::AsFormat::Link => rules::Target::Link,
            }),
            &limits(l),
            json,
        ),
        Command::Check {
            files,
            json,
            strict,
            limits: l,
        } => check::run(&files, &limits(l), json, strict),
        Command::Export { file, out, json } => commands::export(&file, &out, json),
    };
    std::process::exit(code);
}

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "tblctl",
    version,
    about = "Converts and checks tables for Markdown docs",
    after_help = "Examples:
  tblctl classify att/*.xlsx att/*.docx
  tblctl import att/limits.xlsx --sheet Limits --out docs/limits.md
  tblctl check docs/*.md

Documentation: https://github.com/driftsys/tblctl#readme"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Output format forced with `import --as`.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum AsFormat {
    Md,
    Csv,
    Link,
}

/// T1 size limits.
#[derive(Args)]
pub struct LimitArgs {
    /// Maximum data rows for a Markdown table
    #[arg(long, default_value_t = 30)]
    pub max_rows: usize,
    /// Maximum columns for a Markdown table
    #[arg(long, default_value_t = 8)]
    pub max_cols: usize,
    /// Maximum characters per cell (link URLs in Markdown are not counted)
    #[arg(long, default_value_t = 120)]
    pub max_cell: usize,
}

#[derive(Subcommand)]
pub enum Command {
    /// Classify tables and report the rule that applies (T0 skip, T1 Markdown, T2 CSV, T3 link)
    #[command(after_help = "Examples:
  tblctl classify att/limits.xlsx
  tblctl classify att/*.xlsx att/*.docx
  tblctl classify --json att/limits.xlsx")]
    Classify {
        /// Source files: xlsx, xlsm, xls, xlsb, ods, docx, csv, md
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        limits: LimitArgs,
    },
    /// Import a table as Markdown, CSV or a link stub (the rule selects, or --as)
    #[command(after_help = "Examples:
  tblctl import att/limits.xlsx --out docs/limits.md
  tblctl import att/limits.xlsx --sheet Limits --out docs/limits.md
  tblctl import att/large.xlsx --sheet Data --out docs/data.csv
  tblctl import att/large.xlsx --as md --out docs/data.md")]
    Import {
        /// Source file: xlsx, xlsm, xls, xlsb, ods, docx, csv, md
        file: PathBuf,
        /// Table to import: a sheet name, `line N` (Markdown), `table N` (DOCX)
        #[arg(long)]
        sheet: Option<String>,
        /// Output path (.md for Markdown or a link stub, .csv for CSV)
        #[arg(long)]
        out: PathBuf,
        /// Write this format instead of the one the rule selects
        #[arg(long = "as", value_enum)]
        as_format: Option<AsFormat>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        limits: LimitArgs,
    },
    /// Flag Markdown tables over the T1 size limits
    #[command(after_help = "Examples:
  tblctl check docs/*.md
  tblctl check --max-rows 50 docs/spec.md
  tblctl check --json docs/*.md")]
    Check {
        /// Markdown files to check
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
        /// Treat REVIEW as a failure (exit 1)
        #[arg(long)]
        strict: bool,
        #[command(flatten)]
        limits: LimitArgs,
    },
    /// Export the tables of a Markdown or CSV file to XLSX
    #[command(after_help = "Examples:
  tblctl export docs/limits.md --out limits.xlsx
  tblctl export data/limits.csv --out limits.xlsx")]
    Export {
        /// Markdown or CSV file to export
        file: PathBuf,
        /// Output XLSX path
        #[arg(long)]
        out: PathBuf,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

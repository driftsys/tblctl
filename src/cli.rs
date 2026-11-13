use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "tblctl",
    version,
    about = "Converts and checks tables for Markdown docs"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
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
    Classify {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        limits: LimitArgs,
    },
    /// Import a table as Markdown, CSV or a link stub, as its rule selects
    Import {
        file: PathBuf,
        /// Table to import: a sheet name, `line N` (Markdown), `table N` (DOCX)
        #[arg(long)]
        sheet: Option<String>,
        /// Output path (.md for Markdown or a link stub, .csv for CSV)
        #[arg(long)]
        out: PathBuf,
        #[command(flatten)]
        limits: LimitArgs,
    },
    /// Flag Markdown tables over the T1 size limits
    Check {
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
    Export {
        file: PathBuf,
        /// Output XLSX path
        #[arg(long)]
        out: PathBuf,
    },
}

use clap::{Parser, Subcommand};
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

#[derive(Subcommand)]
pub enum Command {
    /// (not yet implemented) Classify tables and report the rule that applies
    Classify {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
    /// (not yet implemented) Import a table as Markdown, CSV or a link stub
    Import {
        file: PathBuf,
        /// Sheet to import (workbooks only)
        #[arg(long)]
        sheet: Option<String>,
        /// Output path
        #[arg(long)]
        out: PathBuf,
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
        /// Maximum data rows for a Markdown table
        #[arg(long, default_value_t = 30)]
        max_rows: usize,
        /// Maximum columns for a Markdown table
        #[arg(long, default_value_t = 8)]
        max_cols: usize,
        /// Maximum characters per cell
        #[arg(long, default_value_t = 120)]
        max_cell: usize,
    },
    /// (not yet implemented) Export a Markdown or CSV table back to XLSX
    Export {
        file: PathBuf,
        /// Output XLSX path
        #[arg(long)]
        out: PathBuf,
    },
}

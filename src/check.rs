//! `tblctl check`: flags Markdown tables over the T1 limits.

use crate::markdown::{self, Table};
use serde::Serialize;
use std::path::PathBuf;

/// T1 size limits.
pub struct Limits {
    pub max_rows: usize,
    pub max_cols: usize,
    pub max_cell: usize,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Pass,
    Fail,
}

#[derive(Serialize)]
struct Report {
    file: String,
    line: usize,
    status: Status,
    rule: &'static str,
    rows: usize,
    columns: usize,
    reasons: Vec<String>,
}

fn evaluate(file: &str, table: &Table, limits: &Limits) -> Report {
    let rows = table.rows.len();
    let columns = std::iter::once(&table.header)
        .chain(&table.rows)
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    let longest = std::iter::once(&table.header)
        .chain(&table.rows)
        .flatten()
        .map(|c| c.chars().count())
        .max()
        .unwrap_or(0);
    let mut reasons = Vec::new();
    if rows > limits.max_rows {
        reasons.push(format!("rows {rows} > {}", limits.max_rows));
    }
    if columns > limits.max_cols {
        reasons.push(format!("columns {columns} > {}", limits.max_cols));
    }
    if longest > limits.max_cell {
        reasons.push(format!("cell {longest} > {}", limits.max_cell));
    }
    let status = if reasons.is_empty() {
        Status::Pass
    } else {
        Status::Fail
    };
    Report {
        file: file.to_string(),
        line: table.line,
        status,
        rule: "t1",
        rows,
        columns,
        reasons,
    }
}

fn render(r: &Report) -> String {
    let state = match r.status {
        Status::Pass => "PASS",
        Status::Fail => "FAIL",
    };
    let mut detail = format!("{}×{}", r.rows, r.columns);
    if !r.reasons.is_empty() {
        detail = format!("{detail} {}; suggest CSV", r.reasons.join(", "));
    }
    format!("{state:<8}[T1] {}:{}  {detail}", r.file, r.line)
}

/// Checks every table in `files` and returns the exit code: 2 if any file
/// could not be read, otherwise 1 if any table failed, otherwise 0.
///
/// `_strict` turns REVIEW into a failure. `check` reports no REVIEW state
/// yet, so the flag has no effect.
pub fn run(files: &[PathBuf], limits: &Limits, json: bool, _strict: bool) -> anyhow::Result<i32> {
    let mut reports = Vec::new();
    let mut read_error = false;
    for path in files {
        let name = path.display().to_string();
        match std::fs::read_to_string(path) {
            Ok(text) => {
                for table in markdown::tables(&text) {
                    let report = evaluate(&name, &table, limits);
                    if !json {
                        println!("{}", render(&report));
                    }
                    reports.push(report);
                }
            }
            Err(err) => {
                eprintln!("tblctl: {name}: {err}");
                read_error = true;
            }
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    }
    Ok(if read_error {
        2
    } else if reports.iter().any(|r| r.status == Status::Fail) {
        1
    } else {
        0
    })
}

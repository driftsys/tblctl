//! `tblctl check`: flags Markdown tables over the T1 limits.

use crate::error::Error;
use crate::markdown::{self, Table};
use crate::output::{paint, GREEN, RED};
use crate::rules::{self, Limits};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;

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

fn evaluate(file: &str, table: Table, limits: &Limits) -> Report {
    let line = table.line;
    let grid = crate::read::markdown_grid(file, table);
    let reasons = rules::size_reasons(&grid, limits);
    let status = if reasons.is_empty() {
        Status::Pass
    } else {
        Status::Fail
    };
    Report {
        file: file.to_string(),
        line,
        status,
        rule: "t1",
        rows: grid.data_rows(),
        columns: grid.columns(),
        reasons,
    }
}

fn render(r: &Report) -> String {
    let state = match r.status {
        Status::Pass => paint("PASS", GREEN, 8),
        Status::Fail => paint("FAIL", RED, 8),
    };
    let mut detail = format!("{}×{}", r.rows, r.columns);
    if !r.reasons.is_empty() {
        detail = format!("{detail} {}; suggest CSV", r.reasons.join(", "));
    }
    format!("{state}[T1] {}:{}  {detail}", r.file, r.line)
}

/// Checks every table in `files` and returns the exit code: 2 if any file
/// could not be read, otherwise 1 if any table failed, otherwise 0.
///
/// `_strict` turns REVIEW into a failure. `check` reports no REVIEW state
/// yet, so the flag has no effect.
pub fn run(files: &[PathBuf], limits: &Limits, json: bool, _strict: bool) -> i32 {
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for path in files {
        let name = path.display().to_string();
        match std::fs::read_to_string(path) {
            Ok(text) => {
                for table in markdown::tables(&text) {
                    reports.push(evaluate(&name, table, limits));
                }
            }
            Err(err) => errors.push(Error::ReadFailed {
                message: format!("{name}: {err}"),
                file: name,
            }),
        }
    }
    let failed = reports.iter().filter(|r| r.status == Status::Fail).count();
    let mut text: String = reports.iter().map(|r| render(r) + "\n").collect();
    // No summary line when every input failed to read: "0 tables" would read
    // as a clean result.
    if errors.len() < files.len() {
        text.push_str(&format!(
            "{}: {} passed, {failed} failed\n",
            crate::output::tables(reports.len()),
            reports.len() - failed
        ));
    }
    let summary = json!({
        "tables": reports.len(),
        "pass": reports.len() - failed,
        "fail": failed,
    });
    crate::output::finish(
        "check",
        json,
        json!(reports),
        summary,
        &errors,
        &text,
        failed > 0,
    )
}

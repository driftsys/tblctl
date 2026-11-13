//! `classify`, `import` and `export`.

use crate::output::emit;
use crate::read;
use crate::rules::{self, Limits, Rule, Target};
use crate::table::Grid;
use crate::write;
use anyhow::{bail, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct Classified {
    file: String,
    table: String,
    rule: Rule,
    target: Target,
    rows: usize,
    columns: usize,
    reasons: Vec<String>,
}

fn rule_label(rule: Rule) -> &'static str {
    match rule {
        Rule::T0 => "T0",
        Rule::T1 => "T1",
        Rule::T2 => "T2",
        Rule::T3 => "T3",
    }
}

fn target_label(target: Target) -> &'static str {
    match target {
        Target::Skip => "skip",
        Target::Markdown => "markdown",
        Target::Csv => "csv",
        Target::Link => "link",
    }
}

/// Classifies every table in `files`. Exit 2 if any file could not be read,
/// otherwise 0.
pub fn classify(files: &[PathBuf], limits: &Limits, json: bool) -> Result<i32> {
    let mut grids = Vec::new();
    let mut read_error = false;
    for path in files {
        match read::tables(path) {
            Ok(g) => grids.extend(g),
            Err(err) => {
                eprintln!("tblctl: {err:#}");
                read_error = true;
            }
        }
    }
    let verdicts = rules::classify(&grids, limits);
    let items: Vec<Classified> = grids
        .iter()
        .zip(verdicts)
        .map(|(g, v)| Classified {
            file: g.file.clone(),
            table: g.name.clone(),
            rule: v.rule,
            target: v.rule.target(),
            rows: g.data_rows(),
            columns: g.columns(),
            reasons: v.reasons,
        })
        .collect();
    let text = if json {
        serde_json::to_string_pretty(&items)? + "\n"
    } else {
        items
            .iter()
            .map(|c| {
                let mut line = format!(
                    "{}  {:<9}{}#{}  {}×{}",
                    rule_label(c.rule),
                    target_label(c.target),
                    c.file,
                    c.table,
                    c.rows,
                    c.columns
                );
                if !c.reasons.is_empty() {
                    line.push_str("  ");
                    line.push_str(&c.reasons.join(", "));
                }
                line + "\n"
            })
            .collect()
    };
    emit(&text)?;
    Ok(if read_error { 2 } else { 0 })
}

fn pick(mut grids: Vec<Grid>, sheet: Option<&str>, file: &Path) -> Result<Grid> {
    let names: Vec<String> = grids.iter().map(|g| g.name.clone()).collect();
    match sheet {
        Some(s) => match grids.iter().position(|g| g.name == s) {
            Some(i) => Ok(grids.swap_remove(i)),
            None => bail!(
                "{}: no table named {s:?} (tables: {})",
                file.display(),
                names.join(", ")
            ),
        },
        None if grids.len() == 1 => Ok(grids.remove(0)),
        None if grids.is_empty() => bail!("{}: no table found", file.display()),
        None => bail!(
            "{}: {} tables found; choose one with --sheet ({})",
            file.display(),
            grids.len(),
            names.join(", ")
        ),
    }
}

/// Imports one table from `file` in the format its rule selects.
pub fn import(file: &Path, sheet: Option<&str>, out: &Path, limits: &Limits) -> Result<i32> {
    if same_file(file, out) {
        bail!("{}: --out is the input file", out.display());
    }
    let grids = read::tables(file)?;
    let grid = pick(grids, sheet, file)?;
    let verdict = rules::classify(std::slice::from_ref(&grid), limits)
        .pop()
        .expect("one verdict per grid");
    let target = verdict.rule.target();
    let summary = format!(
        "{}  {:<9}{}#{}",
        rule_label(verdict.rule),
        target_label(target),
        grid.file,
        grid.name
    );
    if target == Target::Skip {
        emit(&format!(
            "{summary}  {}; nothing written\n",
            verdict.reasons.join(", ")
        ))?;
        return Ok(0);
    }
    let expected = if target == Target::Csv { "csv" } else { "md" };
    let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case(expected) {
        bail!(
            "{}: rule {} writes {}; use an --out path ending in .{expected}",
            out.display(),
            rule_label(verdict.rule),
            target_label(target)
        );
    }
    let body = match target {
        Target::Markdown => write::markdown(&grid),
        Target::Csv => write::csv(&grid)?,
        Target::Link => write::link_stub(&grid, file, out)?,
        Target::Skip => unreachable!("handled above"),
    };
    std::fs::write(out, body)?;
    emit(&format!("{summary}  -> {}\n", out.display()))?;
    Ok(0)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Exports the tables of a Markdown or CSV file to XLSX, one sheet per table.
pub fn export(file: &Path, out: &Path) -> Result<i32> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "md" | "markdown" | "csv") {
        bail!("{}: export reads Markdown or CSV only", file.display());
    }
    if same_file(file, out) {
        bail!("{}: --out is the input file", out.display());
    }
    let grids = read::tables(file)?;
    if grids.is_empty() {
        bail!("{}: no table found", file.display());
    }
    write::xlsx(&grids, out)?;
    emit(&format!("{} table(s) -> {}\n", grids.len(), out.display()))?;
    Ok(0)
}

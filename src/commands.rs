//! `classify`, `import` and `export`.

use crate::error::Error;
use crate::output::{finish, paint, DIM, GREEN, YELLOW};
use crate::read;
use crate::rules::{self, Limits, Rule, Target};
use crate::table::Grid;
use crate::write;
use serde::Serialize;
use serde_json::json;
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
pub fn classify(files: &[PathBuf], limits: &Limits, json: bool) -> i32 {
    let mut grids = Vec::new();
    let mut errors = Vec::new();
    for path in files {
        match read::tables(path) {
            Ok(g) => grids.extend(g),
            Err(err) => errors.push(err),
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
    let mut text: String = items
        .iter()
        .map(|c| {
            let style = match c.rule {
                Rule::T0 => DIM,
                Rule::T1 | Rule::T2 => GREEN,
                Rule::T3 => YELLOW,
            };
            let mut line = format!(
                "{}  {:<9}{}#{}  {}×{}",
                paint(rule_label(c.rule), style, 0),
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
        .collect();
    // No summary line when every input failed to read: "0 tables" would read
    // as a clean result.
    if errors.len() < files.len() {
        text.push_str(&classify_summary(&items));
    }
    let count = |rule: Rule| items.iter().filter(|c| c.rule == rule).count();
    let summary = json!({
        "tables": items.len(),
        "t0": count(Rule::T0),
        "t1": count(Rule::T1),
        "t2": count(Rule::T2),
        "t3": count(Rule::T3),
    });
    finish(
        "classify",
        json,
        json!(items),
        summary,
        &errors,
        &text,
        false,
    )
}

fn classify_summary(items: &[Classified]) -> String {
    let count = |rule: Rule| items.iter().filter(|c| c.rule == rule).count();
    format!(
        "{}: {} T0, {} T1, {} T2, {} T3\n",
        crate::output::tables(items.len()),
        count(Rule::T0),
        count(Rule::T1),
        count(Rule::T2),
        count(Rule::T3)
    )
}

fn pick(mut grids: Vec<Grid>, sheet: Option<&str>, file: &Path) -> Result<Grid, Error> {
    let file = file.display().to_string();
    let tables: Vec<String> = grids.iter().map(|g| g.name.clone()).collect();
    match sheet {
        Some(s) => match grids.iter().position(|g| g.name == s) {
            Some(i) => Ok(grids.swap_remove(i)),
            None => Err(Error::NoSuchTable {
                file,
                name: s.to_string(),
                tables,
            }),
        },
        None if grids.len() == 1 => Ok(grids.remove(0)),
        None if grids.is_empty() => Err(Error::NoTable { file }),
        None => Err(Error::SeveralTables { file, tables }),
    }
}

/// The result of `import`: the table, its rule and the file written, if any.
#[derive(Serialize)]
struct Imported {
    file: String,
    table: String,
    rule: Rule,
    target: Target,
    out: Option<String>,
    reasons: Vec<String>,
}

/// Imports one table from `file` in the format its rule selects.
pub fn import(file: &Path, sheet: Option<&str>, out: &Path, limits: &Limits, json: bool) -> i32 {
    let (results, errors) = match import_table(file, sheet, out, limits) {
        Ok(i) => {
            if !json {
                let status = format!(
                    "{}  {:<9}{}#{}",
                    rule_label(i.rule),
                    target_label(i.target),
                    i.file,
                    i.table
                );
                match &i.out {
                    Some(path) => eprintln!("{status}  -> {path}"),
                    None => eprintln!("{status}  {}; nothing written", i.reasons.join(", ")),
                }
            }
            (vec![i], vec![])
        }
        Err(err) => (vec![], vec![err]),
    };
    let written = results.iter().filter(|i| i.out.is_some()).count();
    finish(
        "import",
        json,
        json!(results),
        json!({ "written": written }),
        &errors,
        "",
        false,
    )
}

fn import_table(
    file: &Path,
    sheet: Option<&str>,
    out: &Path,
    limits: &Limits,
) -> Result<Imported, Error> {
    let out_name = out.display().to_string();
    if same_file(file, out) {
        return Err(Error::SameFile { file: out_name });
    }
    let grids = read::tables(file)?;
    let grid = pick(grids, sheet, file)?;
    let verdict = rules::classify(std::slice::from_ref(&grid), limits)
        .pop()
        .expect("one verdict per grid");
    let target = verdict.rule.target();
    let mut imported = Imported {
        file: grid.file.clone(),
        table: grid.name.clone(),
        rule: verdict.rule,
        target,
        out: None,
        reasons: verdict.reasons,
    };
    if target == Target::Skip {
        return Ok(imported);
    }
    let expected = if target == Target::Csv { "csv" } else { "md" };
    let ext = out.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case(expected) {
        return Err(Error::WrongExtension {
            message: format!(
                "{out_name}: rule {} writes {}; use an --out path ending in .{expected}",
                rule_label(verdict.rule),
                target_label(target)
            ),
            file: out_name,
            expected,
        });
    }
    let write_failed = |err: String| Error::WriteFailed {
        file: out_name.clone(),
        message: format!("{out_name}: {err}"),
    };
    let body = match target {
        Target::Markdown => write::markdown(&grid),
        Target::Csv => write::csv(&grid).map_err(|e| write_failed(format!("{e:#}")))?,
        Target::Link => {
            write::link_stub(&grid, file, out).map_err(|e| write_failed(format!("{e:#}")))?
        }
        Target::Skip => unreachable!("handled above"),
    };
    std::fs::write(out, body).map_err(|e| write_failed(e.to_string()))?;
    imported.out = Some(out_name);
    Ok(imported)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// One worksheet written by `export`.
#[derive(Serialize)]
struct Exported {
    table: String,
    sheet: String,
    rows: usize,
    columns: usize,
}

/// Exports the tables of a Markdown or CSV file to XLSX, one sheet per table.
pub fn export(file: &Path, out: &Path, json: bool) -> i32 {
    let (results, errors) = match export_tables(file, out) {
        Ok(grids) => {
            if !json {
                eprintln!("{} table(s) -> {}", grids.len(), out.display());
            }
            let results: Vec<Exported> = grids
                .iter()
                .enumerate()
                .map(|(i, g)| Exported {
                    table: g.name.clone(),
                    sheet: write::sheet_name(i, grids.len()),
                    rows: g.data_rows(),
                    columns: g.columns(),
                })
                .collect();
            (results, vec![])
        }
        Err(err) => (vec![], vec![err]),
    };
    let summary = json!({
        "out": (!results.is_empty()).then(|| out.display().to_string()),
        "sheets": results.len(),
    });
    finish("export", json, json!(results), summary, &errors, "", false)
}

fn export_tables(file: &Path, out: &Path) -> Result<Vec<Grid>, Error> {
    let file_name = file.display().to_string();
    let out_name = out.display().to_string();
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "md" | "markdown" | "csv") {
        return Err(Error::UnsupportedType {
            message: format!("{file_name}: export reads Markdown or CSV only"),
            file: file_name,
        });
    }
    if same_file(file, out) {
        return Err(Error::SameFile { file: out_name });
    }
    let out_ext = out.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !out_ext.eq_ignore_ascii_case("xlsx") {
        return Err(Error::WrongExtension {
            message: format!("{out_name}: export writes XLSX; use an --out path ending in .xlsx"),
            file: out_name,
            expected: "xlsx",
        });
    }
    let grids = read::tables(file)?;
    if grids.is_empty() {
        return Err(Error::NoTable { file: file_name });
    }
    write::xlsx(&grids, out).map_err(|e| Error::WriteFailed {
        message: format!("{out_name}: {e:#}"),
        file: out_name,
    })?;
    Ok(grids)
}

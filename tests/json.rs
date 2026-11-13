//! The `--json` envelope of every command.

use assert_cmd::Command;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn tblctl() -> Command {
    Command::cargo_bin("tblctl").unwrap()
}

/// Runs tblctl with `args`; returns the exit code, the parsed stdout and the
/// stderr text.
fn run(args: &[&Path]) -> (i32, Value, String) {
    let out = tblctl().args(args).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    (
        out.status.code().unwrap(),
        v,
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn p(s: &str) -> &Path {
    Path::new(s)
}

fn md(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, body).unwrap();
    path
}

const SMALL: &str = "| a | b |\n|---|---|\n| 1 | 2 |\n";

#[test]
fn classify_reports_results_summary_and_a_read_error_in_one_object() {
    let d = tempfile::tempdir().unwrap();
    let a = md(&d, "a.md", SMALL);
    let missing = d.path().join("missing.md");
    let (code, v, stderr) = run(&[p("classify"), p("--json"), &missing, &a]);
    assert_eq!(code, 2);
    assert_eq!(v["schema"], "tblctl/classify/v1");
    assert_eq!(v["results"].as_array().unwrap().len(), 1);
    assert_eq!(v["results"][0]["rule"], "t1");
    assert_eq!(v["results"][0]["target"], "markdown");
    assert_eq!(
        v["summary"],
        json!({"tables": 1, "t0": 0, "t1": 1, "t2": 0, "t3": 0})
    );
    assert_eq!(v["errors"][0]["code"], "read_failed");
    assert_eq!(v["errors"][0]["file"], missing.display().to_string());
    assert!(stderr.starts_with("tblctl: "), "{stderr}");
    assert!(stderr.contains("missing.md"));
}

#[test]
fn classify_reports_an_unsupported_type() {
    let d = tempfile::tempdir().unwrap();
    let txt = md(&d, "a.txt", "x");
    let (code, v, _) = run(&[p("classify"), p("--json"), &txt]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "unsupported_type");
    assert_eq!(v["results"], json!([]));
}

#[test]
fn check_envelope_has_results_summary_and_no_errors() {
    let d = tempfile::tempdir().unwrap();
    let big = format!("| a |\n|---|\n{}", "| 1 |\n".repeat(31));
    let a = md(&d, "a.md", &format!("{SMALL}\n{big}"));
    let (code, v, stderr) = run(&[p("check"), p("--json"), &a]);
    assert_eq!(code, 1);
    assert_eq!(v["schema"], "tblctl/check/v1");
    assert_eq!(v["results"][0]["status"], "pass");
    assert_eq!(v["results"][1]["status"], "fail");
    assert_eq!(v["summary"], json!({"tables": 2, "pass": 1, "fail": 1}));
    assert_eq!(v["errors"], json!([]));
    assert_eq!(stderr, "");
}

#[test]
fn check_reports_a_read_error() {
    let d = tempfile::tempdir().unwrap();
    let (code, v, _) = run(&[p("check"), p("--json"), &d.path().join("no.md")]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "read_failed");
    assert_eq!(v["summary"], json!({"tables": 0, "pass": 0, "fail": 0}));
}

#[test]
fn import_reports_the_written_file() {
    let d = tempfile::tempdir().unwrap();
    let a = md(&d, "a.md", SMALL);
    let out = d.path().join("out.md");
    let (code, v, stderr) = run(&[p("import"), p("--json"), &a, p("--out"), &out]);
    assert_eq!(code, 0);
    assert_eq!(v["schema"], "tblctl/import/v1");
    assert_eq!(v["results"][0]["table"], "line 1");
    assert_eq!(v["results"][0]["rule"], "t1");
    assert_eq!(v["results"][0]["target"], "markdown");
    assert_eq!(v["results"][0]["out"], out.display().to_string());
    assert_eq!(v["summary"], json!({"written": 1}));
    assert_eq!(stderr, "");
    assert!(out.exists());
}

#[test]
fn import_of_a_t0_table_has_a_null_out() {
    let d = tempfile::tempdir().unwrap();
    let a = md(&d, "a.md", "| Date | Author |\n|---|---|\n| 2020 | me |\n");
    let out = d.path().join("out.md");
    let (code, v, _) = run(&[p("import"), p("--json"), &a, p("--out"), &out]);
    assert_eq!(code, 0);
    assert_eq!(v["results"][0]["rule"], "t0");
    assert_eq!(v["results"][0]["out"], Value::Null);
    assert_eq!(v["results"][0]["reasons"], json!(["revision history"]));
    assert_eq!(v["summary"], json!({"written": 0}));
}

#[test]
fn import_errors_carry_codes_and_their_fields() {
    let d = tempfile::tempdir().unwrap();
    let two = md(&d, "two.md", &format!("{SMALL}\n| c |\n|---|\n| 3 |\n"));
    let out = d.path().join("out.md");

    let (code, v, stderr) = run(&[p("import"), p("--json"), &two, p("--out"), &out]);
    assert_eq!(code, 2);
    let e = &v["errors"][0];
    assert_eq!(e["code"], "several_tables");
    assert_eq!(e["file"], two.display().to_string());
    assert_eq!(e["tables"], json!(["line 1", "line 5"]));
    assert_eq!(e["hint"], "choose one with --sheet");
    assert_eq!(v["results"], json!([]));
    assert_eq!(v["summary"], json!({"written": 0}));
    assert!(stderr.starts_with("tblctl: "));

    let (_, v, _) = run(&[
        p("import"),
        p("--json"),
        &two,
        p("--sheet"),
        p("X"),
        p("--out"),
        &out,
    ]);
    assert_eq!(v["errors"][0]["code"], "no_such_table");
    assert_eq!(v["errors"][0]["tables"], json!(["line 1", "line 5"]));

    let empty = md(&d, "empty.md", "no table\n");
    let (_, v, _) = run(&[p("import"), p("--json"), &empty, p("--out"), &out]);
    assert_eq!(v["errors"][0]["code"], "no_table");

    let a = md(&d, "a.md", SMALL);
    let csv = d.path().join("out.csv");
    let (_, v, _) = run(&[p("import"), p("--json"), &a, p("--out"), &csv]);
    assert_eq!(v["errors"][0]["code"], "wrong_extension");
    assert_eq!(v["errors"][0]["expected"], "md");
    assert!(!csv.exists());

    let (_, v, _) = run(&[p("import"), p("--json"), &a, p("--out"), &a]);
    assert_eq!(v["errors"][0]["code"], "same_file");

    let deep = d.path().join("no/such/dir/out.md");
    let (code, v, _) = run(&[p("import"), p("--json"), &a, p("--out"), &deep]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "write_failed");
    assert_eq!(v["errors"][0]["file"], deep.display().to_string());
}

#[test]
fn export_reports_one_result_per_sheet() {
    let d = tempfile::tempdir().unwrap();
    let a = md(
        &d,
        "a.md",
        &format!("{SMALL}\n| c |\n|---|\n| 3 |\n| 4 |\n"),
    );
    let out = d.path().join("out.xlsx");
    let (code, v, stderr) = run(&[p("export"), p("--json"), &a, p("--out"), &out]);
    assert_eq!(code, 0);
    assert_eq!(v["schema"], "tblctl/export/v1");
    assert_eq!(
        v["results"],
        json!([
            {"table": "line 1", "sheet": "Table1", "rows": 1, "columns": 2},
            {"table": "line 5", "sheet": "Table2", "rows": 2, "columns": 1}
        ])
    );
    assert_eq!(
        v["summary"],
        json!({"out": out.display().to_string(), "sheets": 2})
    );
    assert_eq!(stderr, "");
}

#[test]
fn export_errors_carry_codes() {
    let d = tempfile::tempdir().unwrap();
    let a = md(&d, "a.md", SMALL);
    let (code, v, _) = run(&[
        p("export"),
        p("--json"),
        &a,
        p("--out"),
        &d.path().join("x.csv"),
    ]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "wrong_extension");
    assert_eq!(v["errors"][0]["expected"], "xlsx");
    assert_eq!(v["summary"], json!({"out": null, "sheets": 0}));

    let x = md(&d, "a.docx", "x");
    let (_, v, _) = run(&[
        p("export"),
        p("--json"),
        &x,
        p("--out"),
        &d.path().join("x.xlsx"),
    ]);
    assert_eq!(v["errors"][0]["code"], "unsupported_type");

    let (_, v, _) = run(&[
        p("export"),
        p("--json"),
        &a,
        p("--out"),
        &d.path().join("no/dir/x.xlsx"),
    ]);
    assert_eq!(v["errors"][0]["code"], "write_failed");
}

#[test]
fn argument_errors_stay_clap_text_with_exit_2() {
    let out = tblctl().args(["check", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}

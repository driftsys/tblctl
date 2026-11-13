use assert_cmd::Command;
use predicates::str::contains;
use std::fmt::Write as _;
use std::path::PathBuf;

fn tblctl() -> Command {
    Command::cargo_bin("tblctl").unwrap()
}

fn table(rows: usize, cols: usize, cell: &str) -> String {
    let mut s = String::new();
    let header: Vec<String> = (0..cols).map(|c| format!("h{c}")).collect();
    writeln!(s, "| {} |", header.join(" | ")).unwrap();
    writeln!(s, "|{}", "---|".repeat(cols)).unwrap();
    for _ in 0..rows {
        writeln!(s, "| {} |", vec![cell; cols].join(" | ")).unwrap();
    }
    s
}

fn write(dir: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn small_table_passes_with_exit_0() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(&dir, "a.md", &format!("# Title\n\n{}", table(3, 2, "x")));
    tblctl()
        .arg("check")
        .arg(&md)
        .assert()
        .code(0)
        .stdout(contains("PASS    [T1] "))
        .stdout(contains("a.md:3  3×2"));
}

#[test]
fn too_many_rows_fails_with_exit_1_and_suggests_csv() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(&dir, "a.md", &table(31, 2, "x"));
    tblctl()
        .arg("check")
        .arg(&md)
        .assert()
        .code(1)
        .stdout(contains("FAIL    [T1] "))
        .stdout(contains("a.md:1  31×2 rows 31 > 30; suggest CSV"));
}

#[test]
fn too_many_columns_and_long_cells_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let long = "y".repeat(121);
    let md = write(&dir, "a.md", &table(1, 9, &long));
    tblctl()
        .arg("check")
        .arg(&md)
        .assert()
        .code(1)
        .stdout(contains("columns 9 > 8"))
        .stdout(contains("cell 121 > 120"));
}

#[test]
fn threshold_flags_override_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(&dir, "a.md", &table(31, 2, "x"));
    tblctl()
        .args(["check", "--max-rows", "40"])
        .arg(&md)
        .assert()
        .code(0);
    tblctl()
        .args(["check", "--max-cols", "1"])
        .arg(&md)
        .assert()
        .code(1);
    tblctl()
        .args(["check", "--max-cell", "0"])
        .arg(&md)
        .assert()
        .code(1);
}

#[test]
fn tables_in_fenced_code_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(&dir, "a.md", &format!("```\n{}```\n", table(31, 2, "x")));
    tblctl().arg("check").arg(&md).assert().code(0).stdout("");
}

#[test]
fn many_files_are_checked_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(&dir, "a.md", &table(1, 1, "x"));
    let b = write(&dir, "b.md", &table(31, 1, "x"));
    let out = tblctl().arg("check").arg(&a).arg(&b).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("PASS") && lines[0].contains("a.md:1"));
    assert!(lines[1].starts_with("FAIL") && lines[1].contains("b.md:1"));
}

#[test]
fn unreadable_file_exits_2_and_still_checks_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(&dir, "a.md", &table(1, 1, "x"));
    tblctl()
        .arg("check")
        .arg(dir.path().join("missing.md"))
        .arg(&a)
        .assert()
        .code(2)
        .stdout(contains("PASS"))
        .stderr(contains("missing.md"));
}

#[test]
fn json_is_an_array_with_one_object_per_table() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(
        &dir,
        "a.md",
        &format!("{}\n{}", table(1, 1, "x"), table(31, 1, "x")),
    );
    let out = tblctl()
        .args(["check", "--json"])
        .arg(&md)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["status"], "pass");
    assert_eq!(arr[0]["rule"], "t1");
    assert_eq!(arr[0]["line"], 1);
    assert_eq!(arr[1]["status"], "fail");
    assert_eq!(arr[1]["rows"], 31);
    assert_eq!(arr[1]["columns"], 1);
    assert_eq!(arr[1]["reasons"][0], "rows 31 > 30");
}

#[test]
fn strict_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(&dir, "a.md", &table(1, 1, "x"));
    tblctl()
        .args(["check", "--strict"])
        .arg(&md)
        .assert()
        .code(0);
}

#[test]
fn extra_body_cells_do_not_add_columns() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(
        &dir,
        "a.md",
        "| a | b |\n|---|---|\n| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |\n",
    );
    tblctl()
        .arg("check")
        .arg(&md)
        .assert()
        .code(0)
        .stdout(contains("1×2"));
}

#[test]
fn closed_stdout_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let md = write(&dir, "a.md", &table(31, 1, "x").repeat(2000));
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("tblctl"))
        .arg("check")
        .arg(&md)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
}

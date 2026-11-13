use assert_cmd::Command;
use predicates::str::contains;
use rust_xlsxwriter::{Formula, Workbook};
use std::io::Write as _;
use std::path::{Path, PathBuf};

fn tblctl() -> Command {
    Command::cargo_bin("tblctl").unwrap()
}

fn rows(n: usize) -> Vec<Vec<String>> {
    let mut r = vec![vec!["id".to_string(), "name".to_string()]];
    for i in 0..n {
        r.push(vec![i.to_string(), format!("item {i}")]);
    }
    r
}

fn xlsx(path: &Path, sheets: &[(&str, Vec<Vec<String>>)], edit: impl Fn(&mut Workbook)) {
    let mut wb = Workbook::new();
    for (name, data) in sheets {
        let ws = wb.add_worksheet();
        ws.set_name(*name).unwrap();
        for (r, row) in data.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                ws.write_string(r as u32, c as u16, v).unwrap();
            }
        }
    }
    edit(&mut wb);
    wb.save(path).unwrap();
}

fn docx(path: &Path, body: &str) {
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    z.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    write!(z, "<w:document><w:body>{body}</w:body></w:document>").unwrap();
    z.finish().unwrap();
}

fn classify_json(files: &[PathBuf]) -> serde_json::Value {
    let out = tblctl()
        .arg("classify")
        .arg("--json")
        .args(files)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn classify_applies_t1_t2_t3_and_t0() {
    let d = tempfile::tempdir().unwrap();
    let small = d.path().join("small.xlsx");
    xlsx(&small, &[("S", rows(3))], |_| {});
    let large = d.path().join("large.xlsx");
    xlsx(&large, &[("L", rows(40))], |_| {});
    let formulas = d.path().join("formulas.xlsx");
    xlsx(&formulas, &[("F", rows(40))], |wb| {
        wb.worksheet_from_index(0)
            .unwrap()
            .write_formula(41, 0, Formula::new("=1+1"))
            .unwrap();
    });
    let two = d.path().join("two.xlsx");
    xlsx(&two, &[("A", rows(1)), ("B", rows(2))], |_| {});
    let hidden = d.path().join("hidden.xlsx");
    xlsx(&hidden, &[("A", rows(4)), ("H", rows(2))], |wb| {
        wb.worksheet_from_index(1).unwrap().set_hidden(true);
    });
    let dup = d.path().join("dup.csv");
    std::fs::write(&dup, "id,name\n0,item 0\n1,item 1\n2,item 2\n").unwrap();

    let v = classify_json(&[small, large, formulas, two, hidden, dup]);
    let got: Vec<(String, String, String)> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            (
                o["table"].as_str().unwrap().to_string(),
                o["rule"].as_str().unwrap().to_string(),
                o["target"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let want = [
        ("S", "t1", "markdown"),
        ("L", "t2", "csv"),
        ("F", "t3", "link"),
        ("A", "t3", "link"),
        ("B", "t3", "link"),
        ("A", "t1", "markdown"),
        ("data", "t0", "skip"),
    ];
    let want: Vec<(String, String, String)> = want
        .iter()
        .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
        .collect();
    assert_eq!(got, want);
    assert_eq!(v[0]["rows"], 3);
    assert_eq!(v[0]["columns"], 2);
    assert!(v[6]["reasons"][0]
        .as_str()
        .unwrap()
        .starts_with("duplicate of "));
}

#[test]
fn merged_header_is_flattened_and_merged_data_is_t3() {
    let d = tempfile::tempdir().unwrap();
    let head = d.path().join("head.xlsx");
    xlsx(&head, &[("S", rows(2))], |wb| {
        wb.worksheet_from_index(0)
            .unwrap()
            .merge_range(0, 0, 0, 1, "Both", &rust_xlsxwriter::Format::new())
            .unwrap();
    });
    let data = d.path().join("data.xlsx");
    xlsx(&data, &[("S", rows(2))], |wb| {
        wb.worksheet_from_index(0)
            .unwrap()
            .merge_range(1, 0, 2, 0, "m", &rust_xlsxwriter::Format::new())
            .unwrap();
    });
    let v = classify_json(&[head.clone(), data]);
    assert_eq!(v[0]["rule"], "t1");
    assert_eq!(v[1]["rule"], "t3");
    assert_eq!(v[1]["reasons"][0], "merged data cells");

    let out = d.path().join("head.md");
    tblctl()
        .arg("import")
        .arg(&head)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert!(std::fs::read_to_string(&out)
        .unwrap()
        .starts_with("| Both | Both |\n|---|---|\n"));
}

#[test]
fn import_writes_markdown_csv_or_link_stub() {
    let d = tempfile::tempdir().unwrap();
    let small = d.path().join("small.xlsx");
    xlsx(&small, &[("S", rows(1))], |_| {});
    let md = d.path().join("small.md");
    tblctl()
        .arg("import")
        .arg(&small)
        .arg("--out")
        .arg(&md)
        .assert()
        .code(0)
        .stdout(contains("T1  markdown"));
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        "| id | name |\n|---|---|\n| 0 | item 0 |\n"
    );

    let large = d.path().join("large.xlsx");
    xlsx(&large, &[("L", rows(40))], |_| {});
    let csv = d.path().join("large.csv");
    tblctl()
        .arg("import")
        .arg(&large)
        .arg("--out")
        .arg(&csv)
        .assert()
        .code(0);
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.starts_with("id,name\n0,item 0\n"));
    assert_eq!(text.lines().count(), 41);

    // Wrong extension for the selected rule: operational error, nothing written.
    let wrong = d.path().join("large.md");
    tblctl()
        .arg("import")
        .arg(&large)
        .arg("--out")
        .arg(&wrong)
        .assert()
        .code(2)
        .stderr(contains(".csv"));
    assert!(!wrong.exists());

    let two = d.path().join("two.xlsx");
    xlsx(&two, &[("A", rows(1)), ("B", rows(2))], |_| {});
    tblctl()
        .arg("import")
        .arg(&two)
        .arg("--out")
        .arg(d.path().join("x.md"))
        .assert()
        .code(2)
        .stderr(contains("--sheet"));
    let docs = d.path().join("docs");
    std::fs::create_dir(&docs).unwrap();
    let stub = docs.join("b.md");
    tblctl()
        .arg("import")
        .arg(&two)
        .args(["--sheet", "B"])
        .arg("--out")
        .arg(&stub)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&stub).unwrap(),
        "[two.xlsx — B, 2 rows × 2 columns](../two.xlsx)\n"
    );
}

#[test]
fn docx_tables_are_read() {
    let d = tempfile::tempdir().unwrap();
    let doc = d.path().join("a.docx");
    docx(
        &doc,
        "<w:p/><w:tbl>\
         <w:tr><w:tc><w:p><w:r><w:t>k</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>v</w:t></w:r></w:p></w:tc></w:tr>\
         <w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr>\
         </w:tbl>",
    );
    let v = classify_json(std::slice::from_ref(&doc));
    assert_eq!(v[0]["table"], "table 1");
    assert_eq!(v[0]["rule"], "t1");
    let out = d.path().join("a.md");
    tblctl()
        .arg("import")
        .arg(&doc)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "| k | v |\n|---|---|\n| a | 1<br>2 |\n"
    );
}

#[test]
fn export_round_trips_markdown_through_xlsx() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    std::fs::write(&md, "| k | v |\n|---|---|\n| a \\| b | 1<br>2 |\n").unwrap();
    let x1 = d.path().join("a.xlsx");
    let x2 = d.path().join("b.xlsx");
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&x1)
        .assert()
        .code(0);
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&x2)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read(&x1).unwrap(),
        std::fs::read(&x2).unwrap(),
        "deterministic"
    );

    let back = d.path().join("back.md");
    tblctl()
        .arg("import")
        .arg(&x1)
        .arg("--out")
        .arg(&back)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&back).unwrap(),
        std::fs::read_to_string(&md).unwrap()
    );
}

#[test]
fn export_rejects_workbook_input_and_unreadable_files_exit_2() {
    let d = tempfile::tempdir().unwrap();
    let x = d.path().join("a.xlsx");
    xlsx(&x, &[("S", rows(1))], |_| {});
    tblctl()
        .arg("export")
        .arg(&x)
        .arg("--out")
        .arg(d.path().join("b.xlsx"))
        .assert()
        .code(2);
    tblctl()
        .args(["classify"])
        .arg(d.path().join("missing.xlsx"))
        .arg(&x)
        .assert()
        .code(2)
        .stdout(contains("T1"));
}

#[test]
fn dates_import_as_iso_and_formatting_merges_outside_data_are_ignored() {
    let d = tempfile::tempdir().unwrap();
    let x = d.path().join("a.xlsx");
    xlsx(&x, &[], |wb| {
        let ws = wb.add_worksheet();
        let date = rust_xlsxwriter::Format::new().set_num_format("yyyy-mm-dd");
        ws.write_string(0, 1, "when").unwrap();
        ws.write_string(0, 2, "what").unwrap();
        let dt = rust_xlsxwriter::ExcelDateTime::from_ymd(2024, 3, 15).unwrap();
        ws.write_datetime_with_format(1, 1, &dt, &date).unwrap();
        ws.write_string(1, 2, "x").unwrap();
        // Empty merged block left of the data (column A): formatting only.
        ws.merge_range(4, 0, 9, 0, "", &rust_xlsxwriter::Format::new())
            .unwrap();
    });
    let v = classify_json(std::slice::from_ref(&x));
    assert_eq!(v[0]["rule"], "t1", "{v}");
    let out = d.path().join("a.md");
    tblctl()
        .arg("import")
        .arg(&x)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert!(std::fs::read_to_string(&out)
        .unwrap()
        .contains("| 2024-03-15 | x |"));
}

#[test]
fn import_refuses_to_overwrite_its_input() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    std::fs::write(&md, "# Doc\n\n| a |\n|---|\n| 1 |\n").unwrap();
    tblctl()
        .arg("import")
        .arg(&md)
        .arg("--out")
        .arg(&md)
        .assert()
        .code(2)
        .stderr(contains("input file"));
    assert!(std::fs::read_to_string(&md).unwrap().starts_with("# Doc"));
}

#[test]
fn export_writes_canonical_numbers_as_numbers_and_header_br_as_newline() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    std::fs::write(&md, "| Unit<br/>(ms) | code |\n|---|---|\n| 12 | 007 |\n").unwrap();
    let x = d.path().join("a.xlsx");
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&x)
        .assert()
        .code(0);
    use calamine::{Data, Reader};
    let mut wb = calamine::open_workbook_auto(&x).unwrap();
    let name = wb.sheet_names()[0].clone();
    let r = wb.worksheet_range(&name).unwrap();
    assert_eq!(r.get((0, 0)), Some(&Data::String("Unit\n(ms)".into())));
    assert_eq!(r.get((1, 0)), Some(&Data::Float(12.0)));
    assert_eq!(r.get((1, 1)), Some(&Data::String("007".into())));
}

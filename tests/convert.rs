use assert_cmd::Command;
use predicates::str::contains;
use rust_xlsxwriter::{Formula, Workbook};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The binary, with the colour variables of the parent environment removed so
/// that text output is plain unless a test sets them.
fn tblctl() -> Command {
    let mut cmd = Command::cargo_bin("tblctl").unwrap();
    cmd.env_remove("CLICOLOR_FORCE").env_remove("NO_COLOR");
    cmd
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
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["results"].clone()
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
        .stdout("")
        .stderr(contains("T1  markdown"));
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

#[test]
fn formatting_merges_above_below_or_right_of_data_are_ignored() {
    let d = tempfile::tempdir().unwrap();
    let blank = rust_xlsxwriter::Format::new();
    let mut files = Vec::new();
    for (name, (r1, c1, r2, c2)) in [
        ("above", (0, 1, 0, 3)),
        ("below", (8, 1, 12, 2)),
        ("right", (2, 6, 5, 6)),
        ("partly", (3, 2, 9, 2)),
    ] {
        let x = d.path().join(format!("{name}.xlsx"));
        xlsx(&x, &[], |wb| {
            let ws = wb.add_worksheet();
            for r in 2..5u32 {
                ws.write_string(r, 1, format!("k{r}")).unwrap();
                ws.write_string(r, 2, format!("v{r}")).unwrap();
            }
            ws.merge_range(r1, c1, r2, c2, "", &blank).unwrap();
        });
        files.push(x);
    }
    // One run per file, so the equal tables are not duplicates.
    let v: Vec<serde_json::Value> = files
        .iter()
        .map(|f| classify_json(std::slice::from_ref(f))[0].clone())
        .collect();
    for t in &v[..3] {
        assert_eq!(t["rule"], "t1", "{t}");
        assert_eq!(t["columns"], 2, "{t}");
        assert_eq!(t["rows"], 2, "{t}");
    }
    // A merge that covers data cells is clipped to the data and still counts.
    assert_eq!(v[3]["rule"], "t3", "{}", v[3]);
    assert_eq!(v[3]["columns"], 2, "{}", v[3]);
}

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

#[test]
fn workbook_and_docx_images_are_t3() {
    let d = tempfile::tempdir().unwrap();
    let x = d.path().join("img.xlsx");
    xlsx(&x, &[("S", rows(2))], |wb| {
        let img = rust_xlsxwriter::Image::new_from_buffer(PNG).unwrap();
        wb.worksheet_from_index(0)
            .unwrap()
            .insert_image(5, 5, &img)
            .unwrap();
    });
    let doc = d.path().join("img.docx");
    docx(
        &doc,
        "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>k</w:t></w:r></w:p></w:tc></w:tr>\
         <w:tr><w:tc><w:p><w:r><w:drawing/></w:r></w:p></w:tc></w:tr></w:tbl>",
    );
    let v = classify_json(&[x, doc]);
    assert_eq!(v[0]["rule"], "t3", "{v}");
    assert_eq!(v[0]["reasons"][0], "images");
    assert_eq!(v[1]["rule"], "t3", "{v}");
    assert_eq!(v[1]["reasons"][0], "images");
}

/// Writes a minimal ODS file whose `content.xml` body is `table`.
fn ods(path: &Path, table: &str) {
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    z.start_file("mimetype", stored).unwrap();
    write!(z, "application/vnd.oasis.opendocument.spreadsheet").unwrap();
    z.start_file("META-INF/manifest.xml", stored).unwrap();
    write!(
        z,
        r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#
    )
    .unwrap();
    z.start_file("content.xml", stored).unwrap();
    write!(
        z,
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2"><office:body><office:spreadsheet>{table}</office:spreadsheet></office:body></office:document-content>"#
    )
    .unwrap();
    z.finish().unwrap();
}

fn ods_cell(text: &str, attrs: &str) -> String {
    format!(
        r#"<table:table-cell office:value-type="string" {attrs}><text:p>{text}</text:p></table:table-cell>"#
    )
}

#[test]
fn ods_merged_data_cell_is_t3_and_merged_header_is_flattened() {
    let d = tempfile::tempdir().unwrap();
    let covered = "<table:covered-table-cell/>";
    let head = d.path().join("head.ods");
    ods(
        &head,
        &format!(
            r#"<table:table table:name="S"><table:table-row>{}{covered}</table:table-row><table:table-row>{}{}</table:table-row></table:table>"#,
            ods_cell("Both", r#"table:number-columns-spanned="2""#),
            ods_cell("1", ""),
            ods_cell("2", ""),
        ),
    );
    let data = d.path().join("data.ods");
    ods(
        &data,
        &format!(
            r#"<table:table table:name="S"><table:table-row>{}{}</table:table-row><table:table-row>{}{}</table:table-row><table:table-row>{covered}{}</table:table-row></table:table>"#,
            ods_cell("k", ""),
            ods_cell("v", ""),
            ods_cell("m", r#"table:number-rows-spanned="2""#),
            ods_cell("1", ""),
            ods_cell("2", ""),
        ),
    );
    let v = classify_json(&[head.clone(), data.clone()]);
    assert_eq!(v[0]["rule"], "t1", "{v}");
    assert_eq!(v[1]["rule"], "t3", "{v}");
    assert_eq!(v[1]["reasons"][0], "merged data cells");
    let out = d.path().join("head.md");
    tblctl()
        .arg("import")
        .arg(&head)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "| Both | Both |\n|---|---|\n| 1 | 2 |\n"
    );
}

#[test]
fn ods_dates_import_as_iso() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("d.ods");
    ods(
        &f,
        &format!(
            r#"<table:table table:name="S"><table:table-row>{}{}</table:table-row><table:table-row><table:table-cell office:value-type="date" office:date-value="2024-03-15"><text:p>15/03/24</text:p></table:table-cell><table:table-cell office:value-type="date" office:date-value="2024-03-15T10:30:00"><text:p>x</text:p></table:table-cell></table:table-row></table:table>"#,
            ods_cell("when", ""),
            ods_cell("at", ""),
        ),
    );
    let out = d.path().join("d.md");
    tblctl()
        .arg("import")
        .arg(&f)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "| when | at |\n|---|---|\n| 2024-03-15 | 2024-03-15 10:30:00 |\n"
    );
}

#[test]
fn import_of_a_t0_table_writes_nothing_and_exits_0() {
    let d = tempfile::tempdir().unwrap();
    let csv = d.path().join("rev.csv");
    std::fs::write(&csv, "Version,Date,Author\n1,2020-01-01,me\n").unwrap();
    let out = d.path().join("rev.md");
    tblctl()
        .arg("import")
        .arg(&csv)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0)
        .stdout("")
        .stderr(contains("T0  skip"))
        .stderr(contains("nothing written"));
    assert!(!out.exists());
}

#[test]
fn export_refuses_its_input_and_a_non_xlsx_out() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    std::fs::write(&md, "| a |\n|---|\n| 1 |\n").unwrap();
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&md)
        .assert()
        .code(2)
        .stderr(contains("input file"));
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        "| a |\n|---|\n| 1 |\n"
    );
    let csv = d.path().join("a.csv");
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&csv)
        .assert()
        .code(2)
        .stderr(contains(".xlsx"));
    assert!(!csv.exists());
}

fn sheets(path: &Path) -> Vec<(String, calamine::Range<calamine::Data>)> {
    use calamine::Reader;
    let mut wb = calamine::open_workbook_auto(path).unwrap();
    wb.sheet_names()
        .into_iter()
        .map(|n| {
            let r = wb.worksheet_range(&n).unwrap();
            (n, r)
        })
        .collect()
}

#[test]
fn export_reads_csv_and_writes_one_sheet_per_markdown_table() {
    use calamine::Data;
    let d = tempfile::tempdir().unwrap();
    let csv = d.path().join("a.csv");
    std::fs::write(&csv, "k,v\nx,1\n").unwrap();
    let x = d.path().join("csv.xlsx");
    tblctl()
        .arg("export")
        .arg(&csv)
        .arg("--out")
        .arg(&x)
        .assert()
        .code(0);
    let s = sheets(&x);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].1.get((1, 0)), Some(&Data::String("x".into())));

    let md = d.path().join("m.md");
    std::fs::write(&md, "| a |\n|---|\n| 1 |\n\n| b |\n|---|\n| 2 |\n").unwrap();
    let x = d.path().join("md.xlsx");
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&x)
        .assert()
        .code(0)
        .stdout("")
        .stderr(contains("2 table(s)"));
    let s = sheets(&x);
    let names: Vec<&str> = s.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Table1", "Table2"]);
    assert_eq!(s[1].1.get((0, 0)), Some(&Data::String("b".into())));
}

#[test]
fn export_fixes_the_creation_time() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    std::fs::write(&md, "| a |\n|---|\n| 1 |\n").unwrap();
    let x = d.path().join("a.xlsx");
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&x)
        .assert()
        .code(0);
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&x).unwrap()).unwrap();
    let mut core = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("docProps/core.xml").unwrap(), &mut core)
        .unwrap();
    assert!(core.contains("2000-01-01T00:00:00Z"), "{core}");
}

#[test]
fn imported_text_renders_literally_and_round_trips() {
    let d = tempfile::tempdir().unwrap();
    let csv = d.path().join("a.csv");
    std::fs::write(&csv, "k,v\n<value>,*required*\n").unwrap();
    let md = d.path().join("a.md");
    tblctl()
        .arg("import")
        .arg(&csv)
        .arg("--out")
        .arg(&md)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        "| k | v |\n|---|---|\n| \\<value\\> | \\*required\\* |\n"
    );
    let x = d.path().join("a.xlsx");
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&x)
        .assert()
        .code(0);
    assert_eq!(
        sheets(&x)[0].1.get((1, 0)),
        Some(&calamine::Data::String("<value>".into()))
    );
}

#[test]
fn csv_cell_of_120_escaped_stars_imports_and_passes_check() {
    let d = tempfile::tempdir().unwrap();
    let csv = d.path().join("s.csv");
    std::fs::write(&csv, format!("h\n{}\n", "*".repeat(120))).unwrap();
    let out = d.path().join("s.md");
    tblctl()
        .arg("import")
        .arg(&csv)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert!(std::fs::read_to_string(&out)
        .unwrap()
        .contains(&"\\*".repeat(120)));
    let v = classify_json(std::slice::from_ref(&out));
    assert_eq!(v[0]["rule"], "t1", "{v}");
    tblctl().arg("check").arg(&out).assert().code(0);
}

#[test]
fn docx_tab_stop_in_a_later_paragraph_adds_no_tab() {
    let d = tempfile::tempdir().unwrap();
    let doc = d.path().join("t.docx");
    docx(
        &doc,
        "<w:tbl>\
         <w:tr><w:tc><w:p><w:r><w:t>k</w:t></w:r></w:p></w:tc></w:tr>\
         <w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p><w:p><w:pPr><w:tabs><w:tab w:val=\"left\" w:pos=\"720\"/></w:tabs></w:pPr><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr>\
         </w:tbl>",
    );
    let out = d.path().join("t.md");
    tblctl()
        .arg("import")
        .arg(&doc)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "| k |\n|---|\n| 1<br>2 |\n"
    );
}

#[test]
fn ods_merges_land_on_the_right_cells_when_data_does_not_start_at_a1() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("o.ods");
    let pad = r#"<table:table-cell table:number-columns-repeated="2"/>"#;
    let covered = "<table:covered-table-cell/>";
    ods(
        &f,
        &format!(
            r#"<table:table table:name="S"><table:table-row/><table:table-row table:number-rows-repeated="2"><table:table-cell table:number-columns-repeated="6"/></table:table-row><table:table-row>{pad}{}{covered}{}{covered}</table:table-row><table:table-row>{pad}{}{}{}{}</table:table-row></table:table>"#,
            ods_cell("A", r#"table:number-columns-spanned="2""#),
            ods_cell("B", r#"table:number-columns-spanned="2""#),
            ods_cell("1", ""),
            ods_cell("2", ""),
            ods_cell("3", ""),
            ods_cell("4", ""),
        ),
    );
    let v = classify_json(std::slice::from_ref(&f));
    assert_eq!(v[0]["rule"], "t1", "{v}");
    let out = d.path().join("o.md");
    tblctl()
        .arg("import")
        .arg(&f)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "| A | A | B | B |\n|---|---|---|---|\n| 1 | 2 | 3 | 4 |\n"
    );
}

#[test]
fn ods_merges_of_a_second_sheet_with_an_escaped_name_are_found() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("two.ods");
    let pad = r#"<table:table-cell table:number-columns-repeated="1"/>"#;
    let covered = r#"<table:covered-table-cell table:number-columns-repeated="1"/>"#;
    let first = format!(
        r#"<table:table table:name="First"><table:table-row>{}</table:table-row><table:table-row table:number-rows-repeated="5">{}</table:table-row></table:table>"#,
        ods_cell("k", ""),
        ods_cell("1", ""),
    );
    let second = format!(
        r#"<table:table table:name="R&amp;D"><table:table-row/><table:table-row>{pad}{}{}</table:table-row><table:table-row>{pad}{}{}</table:table-row><table:table-row>{pad}{covered}{}</table:table-row></table:table>"#,
        ods_cell("k", ""),
        ods_cell("v", ""),
        ods_cell("m", r#"table:number-rows-spanned="2""#),
        ods_cell("1", ""),
        ods_cell("2", ""),
    );
    ods(&f, &format!("{first}{second}"));
    let v = classify_json(std::slice::from_ref(&f));
    assert_eq!(v[1]["table"], "R&D", "{v}");
    assert_eq!(v[1]["rule"], "t3", "{v}");
    assert!(
        v[1]["reasons"]
            .as_array()
            .unwrap()
            .contains(&"merged data cells".into()),
        "{v}"
    );
}

#[test]
fn export_accepts_an_uppercase_xlsx_extension() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    std::fs::write(&md, "| a |\n|---|\n| 1 |\n").unwrap();
    let out = d.path().join("A.XLSX");
    tblctl()
        .arg("export")
        .arg(&md)
        .arg("--out")
        .arg(&out)
        .assert()
        .code(0);
    assert!(out.exists());
}

#[test]
fn classify_text_ends_with_a_summary_and_colours_the_rule_on_demand() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    let t0 = "| Date | Author |\n|---|---|\n| 2020 | me |\n";
    // Tables differ from each other: a duplicate table is T0.
    let t1 = |c: &str| format!("| a |\n|---|\n| {c} |\n");
    let t2 = |c: &str| format!("| b |\n|---|\n{}", format!("| {c} |\n").repeat(31));
    let body = [t0.to_string(), t1("x"), t1("y"), t1("z"), t2("1"), t2("2")].join("\n");
    std::fs::write(&md, body).unwrap();
    let out = tblctl().arg("classify").arg(&md).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        text.lines().last(),
        Some("6 tables: 1 T0, 3 T1, 2 T2, 0 T3")
    );
    assert!(!text.contains('\x1b'));

    let t3 = d.path().join("f.xlsx");
    xlsx(&t3, &[("F", rows(40))], |wb| {
        wb.worksheet_from_index(0)
            .unwrap()
            .write_formula(41, 0, Formula::new("=1+1"))
            .unwrap();
    });
    let out = tblctl()
        .arg("classify")
        .arg(&md)
        .arg(&t3)
        .env("CLICOLOR_FORCE", "1")
        .env_remove("NO_COLOR")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("\x1b[32mT1\x1b[0m  markdown "), "{text:?}");
    assert!(text.contains("\x1b[2mT0\x1b[0m  skip "), "{text:?}");
    assert!(text.contains("\x1b[32mT2\x1b[0m  csv "), "{text:?}");
    assert!(text.contains("\x1b[33mT3\x1b[0m  link "), "{text:?}");
}

#[test]
fn import_and_export_do_not_panic_when_stderr_is_closed() {
    let d = tempfile::tempdir().unwrap();
    let md = d.path().join("a.md");
    std::fs::write(&md, "| a |\n|---|\n| 1 |\n").unwrap();
    // A revision history is T0: import writes nothing and prints the
    // "nothing written" status line.
    let rev = d.path().join("rev.csv");
    std::fs::write(&rev, "Version,Date,Author\n1,2020-01-01,me\n").unwrap();
    let path = |name: &str| d.path().join(name).display().to_string();
    for args in [
        vec![
            "import".to_string(),
            md.display().to_string(),
            "--out".into(),
            path("b.md"),
        ],
        vec![
            "import".to_string(),
            rev.display().to_string(),
            "--out".into(),
            path("rev.md"),
        ],
        vec![
            "export".to_string(),
            md.display().to_string(),
            "--out".into(),
            path("b.xlsx"),
        ],
    ] {
        // The read end is dropped before the child starts, so every write to
        // stderr fails.
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let status = std::process::Command::new(assert_cmd::cargo::cargo_bin("tblctl"))
            .args(&args)
            .stderr(writer)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(0), "{args:?}");
    }
}

#[test]
fn import_as_forces_the_format_and_keeps_the_rule() {
    let d = tempfile::tempdir().unwrap();
    let large = d.path().join("large.xlsx");
    xlsx(&large, &[("L", rows(40))], |_| {});
    let md = d.path().join("large.md");
    let out = tblctl()
        .arg("import")
        .arg(&large)
        .args(["--as", "md", "--json"])
        .arg("--out")
        .arg(&md)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["results"][0]["rule"], "t2");
    assert_eq!(v["results"][0]["target"], "markdown");
    let reasons = v["results"][0]["reasons"].as_array().unwrap();
    assert_eq!(reasons.last().unwrap(), "target forced by --as");
    let text = std::fs::read_to_string(&md).unwrap();
    assert!(text.starts_with("| id | name |\n"));
    assert_eq!(text.lines().count(), 42);
}

#[test]
fn import_as_checks_the_extension_of_the_forced_format() {
    let d = tempfile::tempdir().unwrap();
    let small = d.path().join("small.xlsx");
    xlsx(&small, &[("S", rows(1))], |_| {});
    let wrong = d.path().join("small.md");
    tblctl()
        .arg("import")
        .arg(&small)
        .args(["--as", "csv"])
        .arg("--out")
        .arg(&wrong)
        .assert()
        .code(2)
        .stderr(contains("--as csv"))
        .stderr(contains(".csv"));
    assert!(!wrong.exists());
    let csv = d.path().join("small.csv");
    tblctl()
        .arg("import")
        .arg(&small)
        .args(["--as", "csv"])
        .arg("--out")
        .arg(&csv)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&csv).unwrap(),
        "id,name\n0,item 0\n"
    );
    let stub = d.path().join("link.md");
    tblctl()
        .arg("import")
        .arg(&small)
        .args(["--as", "link"])
        .arg("--out")
        .arg(&stub)
        .assert()
        .code(0);
    assert_eq!(
        std::fs::read_to_string(&stub).unwrap(),
        "[small.xlsx — S, 1 rows × 2 columns](small.xlsx)\n"
    );
}

#[test]
fn import_as_writes_a_t0_table() {
    let d = tempfile::tempdir().unwrap();
    let hist = d.path().join("h.csv");
    std::fs::write(&hist, "Version,Date,Author\n1,2024-01-01,A\n").unwrap();
    let md = d.path().join("h.md");
    tblctl()
        .arg("import")
        .arg(&hist)
        .args(["--as", "md"])
        .arg("--out")
        .arg(&md)
        .assert()
        .code(0);
    assert!(std::fs::read_to_string(&md)
        .unwrap()
        .starts_with("| Version | Date | Author |"));
}

/// Runs `import` with `--json` and returns the exit code, the envelope and
/// stderr.
fn import_json(args: &[&Path]) -> (Option<i32>, serde_json::Value, String) {
    let out = tblctl()
        .arg("import")
        .arg("--json")
        .args(args)
        .output()
        .unwrap();
    let v = serde_json::from_slice(&out.stdout).unwrap();
    (
        out.status.code(),
        v,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn import_without_as_names_the_rule_in_the_extension_error() {
    let d = tempfile::tempdir().unwrap();
    let large = d.path().join("large.xlsx");
    xlsx(&large, &[("L", rows(40))], |_| {});
    tblctl()
        .arg("import")
        .arg(&large)
        .arg("--out")
        .arg(d.path().join("large.md"))
        .assert()
        .code(2)
        .stderr(contains("rule T2 writes csv"));
}

#[test]
fn import_as_link_and_md_name_the_flag_in_the_extension_error() {
    let d = tempfile::tempdir().unwrap();
    let small = d.path().join("small.xlsx");
    xlsx(&small, &[("S", rows(1))], |_| {});
    for (flag, label) in [("link", "--as link"), ("md", "--as md")] {
        let wrong = d.path().join(format!("{flag}.csv"));
        tblctl()
            .arg("import")
            .arg(&small)
            .args(["--as", flag])
            .arg("--out")
            .arg(&wrong)
            .assert()
            .code(2)
            .stderr(contains(label))
            .stderr(contains("use an --out path ending in .md"));
        assert!(!wrong.exists());
    }
}

#[test]
fn import_as_reports_the_wrong_extension_as_json() {
    let d = tempfile::tempdir().unwrap();
    let small = d.path().join("small.xlsx");
    xlsx(&small, &[("S", rows(1))], |_| {});
    let wrong = d.path().join("small.md");
    let (code, v, _) = import_json(&[
        &small,
        Path::new("--as"),
        Path::new("csv"),
        Path::new("--out"),
        &wrong,
    ]);
    assert_eq!(code, Some(2));
    assert_eq!(v["errors"][0]["code"], "wrong_extension");
    assert_eq!(v["errors"][0]["expected"], "csv");
}

#[test]
fn import_as_adds_its_reason_to_a_table_the_rule_already_sends_there() {
    let d = tempfile::tempdir().unwrap();
    let small = d.path().join("small.xlsx");
    xlsx(&small, &[("S", rows(1))], |_| {});
    let md = d.path().join("small.md");
    let (code, v, _) = import_json(&[
        &small,
        Path::new("--as"),
        Path::new("md"),
        Path::new("--out"),
        &md,
    ]);
    assert_eq!(code, Some(0));
    assert_eq!(v["results"][0]["rule"], "t1");
    assert_eq!(
        v["results"][0]["reasons"],
        serde_json::json!(["target forced by --as"])
    );
}

#[test]
fn import_as_reports_the_whole_reasons_array_and_the_output_path() {
    let d = tempfile::tempdir().unwrap();
    let large = d.path().join("large.xlsx");
    xlsx(&large, &[("L", rows(40))], |_| {});
    let md = d.path().join("large.md");
    let (code, v, _) = import_json(&[
        &large,
        Path::new("--as"),
        Path::new("md"),
        Path::new("--out"),
        &md,
    ]);
    assert_eq!(code, Some(0));
    assert_eq!(
        v["results"][0]["reasons"],
        serde_json::json!(["rows 40 > 30", "target forced by --as"])
    );
    assert_eq!(v["results"][0]["out"], md.display().to_string());
    assert_eq!(v["summary"]["written"], 1);

    let text_md = d.path().join("text.md");
    tblctl()
        .arg("import")
        .arg(&large)
        .args(["--as", "md"])
        .arg("--out")
        .arg(&text_md)
        .assert()
        .code(0)
        .stderr(contains(format!("-> {}", text_md.display())));
}

#[test]
fn import_as_reports_a_forced_t0_table_as_written() {
    let d = tempfile::tempdir().unwrap();
    let hist = d.path().join("h.csv");
    std::fs::write(&hist, "Version,Date,Author\n1,2024-01-01,A\n").unwrap();
    let md = d.path().join("h.md");
    let (code, v, _) = import_json(&[
        &hist,
        Path::new("--as"),
        Path::new("md"),
        Path::new("--out"),
        &md,
    ]);
    assert_eq!(code, Some(0));
    assert_eq!(v["results"][0]["rule"], "t0");
    assert_eq!(v["results"][0]["target"], "markdown");
    assert_eq!(
        v["results"][0]["reasons"],
        serde_json::json!(["revision history", "target forced by --as"])
    );
    assert_eq!(v["results"][0]["out"], md.display().to_string());
    assert_eq!(v["summary"]["written"], 1);

    let text_md = d.path().join("h2.md");
    tblctl()
        .arg("import")
        .arg(&hist)
        .args(["--as", "md"])
        .arg("--out")
        .arg(&text_md)
        .assert()
        .code(0)
        .stderr(contains(format!("-> {}", text_md.display())));
}

#[test]
fn import_as_never_writes_an_empty_table() {
    let d = tempfile::tempdir().unwrap();
    let empty = d.path().join("e.csv");
    std::fs::write(&empty, ",\n,\n").unwrap();
    let md = d.path().join("e.md");
    let (code, v, _) = import_json(&[
        &empty,
        Path::new("--as"),
        Path::new("md"),
        Path::new("--out"),
        &md,
    ]);
    assert_eq!(code, Some(0));
    assert_eq!(v["results"][0]["rule"], "t0");
    assert!(v["results"][0]["out"].is_null());
    assert_eq!(v["results"][0]["reasons"], serde_json::json!(["empty"]));
    assert_eq!(v["summary"]["written"], 0);
    assert!(!md.exists());
    tblctl()
        .arg("import")
        .arg(&empty)
        .args(["--as", "md"])
        .arg("--out")
        .arg(&md)
        .assert()
        .code(0)
        .stderr(contains("nothing written"));
    assert!(!md.exists());
}

#[test]
fn import_rejects_an_unknown_as_value() {
    let d = tempfile::tempdir().unwrap();
    let small = d.path().join("small.xlsx");
    xlsx(&small, &[("S", rows(1))], |_| {});
    tblctl()
        .arg("import")
        .arg(&small)
        .args(["--as", "html"])
        .arg("--out")
        .arg(d.path().join("o.md"))
        .assert()
        .code(2);
}

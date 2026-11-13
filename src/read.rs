//! Reads tables from Markdown, CSV, workbook and DOCX files.

use crate::markdown;
use crate::table::{Facts, Grid};
use anyhow::{bail, Context, Result};
use calamine::{Data, Dimensions, Reader, SheetType, SheetVisible, Sheets};
use std::io::Read as _;
use std::path::Path;

/// Returns every table in `path`, in file order. Hidden sheets are skipped.
pub fn tables(path: &Path) -> Result<Vec<Grid>> {
    let file = path.display().to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut grids = match ext.as_str() {
        "md" | "markdown" => markdown_tables(path, &file)?,
        "csv" => vec![csv_table(path, &file)?],
        "xlsx" | "xlsm" | "xls" | "xlsb" | "ods" => workbook_tables(path, &file)?,
        "docx" => docx_tables(path, &file)?,
        _ => {
            bail!("{file}: unsupported file type (expected md, csv, xlsx, xls, xlsb, ods or docx)")
        }
    };
    for g in &mut grids {
        g.normalise();
    }
    Ok(grids)
}

fn markdown_tables(path: &Path, file: &str) -> Result<Vec<Grid>> {
    let text = std::fs::read_to_string(path).with_context(|| file.to_string())?;
    Ok(markdown::tables(&text)
        .into_iter()
        .map(|t| markdown_grid(file, t))
        .collect())
}

/// Replaces each `<br>`, `<br/>` or `<br />` (any case) with a newline and
/// removes the backslash of each CommonMark backslash escape (a backslash
/// before ASCII punctuation).
fn unescape_cell(cell: &str) -> String {
    let mut out = String::with_capacity(cell.len());
    let mut rest = cell;
    while let Some(i) = rest.find(['<', '\\']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(rest_after) = tail.strip_prefix('\\') {
            match rest_after.chars().next() {
                Some(c) if c.is_ascii_punctuation() => {
                    out.push(c);
                    rest = &rest_after[1..];
                }
                _ => {
                    out.push('\\');
                    rest = rest_after;
                }
            }
            continue;
        }
        let end = tail.find('>').map(|e| e + 1);
        let tag = end.map(|e| tail[..e].to_ascii_lowercase().replace(' ', ""));
        match (end, tag.as_deref()) {
            (Some(e), Some("<br>" | "<br/>")) => {
                out.push('\n');
                rest = &tail[e..];
            }
            _ => {
                out.push('<');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Converts a parsed Markdown table to a grid. `<br>` becomes a newline and
/// backslash escapes are removed. The longest cell is measured on the raw
/// source cells, before this unescaping.
pub fn markdown_grid(file: &str, t: markdown::Table) -> Grid {
    let mut rows = vec![t.header];
    rows.extend(t.rows);
    let longest = rows
        .iter()
        .flatten()
        .map(|c| markdown::visible_len(c))
        .max()
        .unwrap_or(0);
    let rows = rows
        .into_iter()
        .map(|r| r.iter().map(|c| unescape_cell(c)).collect())
        .collect();
    Grid {
        file: file.to_string(),
        name: format!("line {}", t.line),
        rows,
        facts: Facts {
            visible_sheets: 1,
            markdown_longest_cell: Some(longest),
            ..Facts::default()
        },
    }
}

fn csv_table(path: &Path, file: &str) -> Result<Grid> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)
        .with_context(|| file.to_string())?;
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.with_context(|| file.to_string())?;
        rows.push(record.iter().map(str::to_string).collect());
    }
    Ok(Grid {
        file: file.to_string(),
        name: "data".to_string(),
        rows,
        facts: Facts {
            visible_sheets: 1,
            ..Facts::default()
        },
    })
}

fn cell_text(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::DateTime(dt) if !dt.is_duration() => match dt.as_datetime() {
            Some(t) if t.time() == chrono::NaiveTime::MIN => t.format("%Y-%m-%d").to_string(),
            Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => d.to_string(),
        },
        // ODS dates: `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS`.
        Data::DateTimeIso(s) => match s.split_once('T') {
            Some((date, "00:00:00")) => date.to_string(),
            Some((date, time)) => format!("{date} {time}"),
            None => s.clone(),
        },
        other => other.to_string(),
    }
}

fn workbook_tables(path: &Path, file: &str) -> Result<Vec<Grid>> {
    let mut wb: Sheets<_> = calamine::open_workbook_auto(path).with_context(|| file.to_string())?;
    let visible: Vec<String> = wb
        .sheets_metadata()
        .iter()
        .filter(|s| s.visible == SheetVisible::Visible && s.typ == SheetType::WorkSheet)
        .map(|s| s.name.clone())
        .collect();
    let pictures: Vec<String> = wb
        .pictures_with_metadata()
        .into_iter()
        .map(|p| p.sheet_name)
        .collect();
    let any_unmatched_picture = pictures.iter().any(String::is_empty);
    let mut ods_merges = match wb {
        Sheets::Ods(_) => ods::merges(path).with_context(|| file.to_string())?,
        _ => Default::default(),
    };
    let mut out = Vec::new();
    for name in &visible {
        let range = wb
            .worksheet_range(name)
            .with_context(|| format!("{file}: sheet {name}"))?;
        let (r0, c0) = range.start().unwrap_or((0, 0));
        let mut rows: Vec<Vec<String>> = range
            .rows()
            .map(|r| r.iter().map(cell_text).collect())
            .collect();
        let formulas = wb
            .worksheet_formula(name)
            .map(|f| f.used_cells().any(|(_, _, s)| !s.is_empty()))
            .unwrap_or(false);
        let merges = match &wb {
            Sheets::Ods(_) => ods_merges.remove(name).unwrap_or_default(),
            _ => merges(&mut wb, name),
        };
        let (r1, c1) = range.end().unwrap_or((0, 0));
        let mut data_merges = false;
        for m in &merges {
            // A merge wholly outside the used range (formatting only) holds
            // no data; a merge partly outside is clipped to the range.
            if range.is_empty() || m.end.0 < r0 || m.end.1 < c0 || m.start.0 > r1 || m.start.1 > c1
            {
                continue;
            }
            let (sr, sc) = (
                (m.start.0.max(r0) - r0) as usize,
                (m.start.1.max(c0) - c0) as usize,
            );
            let (er, ec) = (
                (m.end.0.min(r1) - r0) as usize,
                (m.end.1.min(c1) - c0) as usize,
            );
            if er > 0 {
                data_merges = true;
            }
            // Flatten by repeating the top-left value over the merged range.
            let value = rows
                .get(sr)
                .and_then(|r| r.get(sc))
                .cloned()
                .unwrap_or_default();
            for row in rows.iter_mut().take(er + 1).skip(sr) {
                for cell in row.iter_mut().take(ec + 1).skip(sc) {
                    cell.clone_from(&value);
                }
            }
        }
        out.push(Grid {
            file: file.to_string(),
            name: name.clone(),
            rows,
            facts: Facts {
                data_merges,
                formulas,
                images: any_unmatched_picture || pictures.iter().any(|p| p == name),
                visible_sheets: visible.len(),
                markdown_longest_cell: None,
            },
        });
    }
    Ok(out)
}

fn merges<RS: std::io::Read + std::io::Seek>(wb: &mut Sheets<RS>, name: &str) -> Vec<Dimensions> {
    match wb {
        Sheets::Xlsx(x) => x.merge_cells_by_sheet_name(name).unwrap_or_default(),
        Sheets::Xls(x) => x.merge_cells_by_sheet_name(name).unwrap_or_default(),
        // The XLSB reader does not expose merged ranges; ODS is read by
        // `ods::merges`.
        _ => Vec::new(),
    }
}

fn docx_tables(path: &Path, file: &str) -> Result<Vec<Grid>> {
    let f = std::fs::File::open(path).with_context(|| file.to_string())?;
    let mut zip = zip::ZipArchive::new(f).with_context(|| file.to_string())?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .with_context(|| format!("{file}: no word/document.xml"))?
        .read_to_string(&mut xml)?;
    Ok(docx::parse(&xml)?
        .into_iter()
        .enumerate()
        .map(|(i, (rows, facts))| Grid {
            file: file.to_string(),
            name: format!("table {}", i + 1),
            rows,
            facts,
        })
        .collect())
}

mod ods {
    use anyhow::Result;
    use calamine::Dimensions;
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::collections::HashMap;
    use std::io::Read as _;
    use std::path::Path;

    fn number(e: &quick_xml::events::BytesStart, key: &str) -> u32 {
        e.attributes()
            .flatten()
            .find(|a| a.key.as_ref() == key)
            .and_then(|a| a.value.parse().ok())
            .unwrap_or(1)
    }

    /// Merged ranges of each sheet of an ODS file, read from `content.xml`
    /// (`table:number-rows-spanned` and `table:number-columns-spanned`), in
    /// the same absolute positions calamine uses.
    pub fn merges(path: &Path) -> Result<HashMap<String, Vec<Dimensions>>> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
        let mut xml = String::new();
        zip.by_name("content.xml")?.read_to_string(&mut xml)?;
        let mut reader = Reader::from_str(&xml);
        let mut out: HashMap<String, Vec<Dimensions>> = HashMap::new();
        let mut sheet = String::new();
        let (mut row, mut col, mut row_repeat) = (0u32, 0u32, 1u32);
        loop {
            let event = reader.read_event()?;
            let empty = matches!(event, Event::Empty(_));
            match event {
                Event::Eof => break,
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    "table:table" => {
                        sheet = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == "table:name")
                            .map(|a| a.value.into_owned())
                            .unwrap_or_default();
                        row = 0;
                    }
                    "table:table-row" => {
                        col = 0;
                        row_repeat = number(&e, "table:number-rows-repeated");
                        if empty {
                            row = row.saturating_add(row_repeat);
                        }
                    }
                    name @ ("table:table-cell" | "table:covered-table-cell") => {
                        if name == "table:table-cell" {
                            let rows = number(&e, "table:number-rows-spanned");
                            let cols = number(&e, "table:number-columns-spanned");
                            if rows > 1 || cols > 1 {
                                out.entry(sheet.clone()).or_default().push(Dimensions::new(
                                    (row, col),
                                    (row.saturating_add(rows - 1), col.saturating_add(cols - 1)),
                                ));
                            }
                        }
                        col = col.saturating_add(number(&e, "table:number-columns-repeated"));
                    }
                    _ => {}
                },
                Event::End(e) if e.name().as_ref() == "table:table-row" => {
                    row = row.saturating_add(row_repeat);
                }
                _ => {}
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_cells_are_unescaped_and_br_becomes_a_newline() {
        let t = markdown::tables("| a |\n|---|\n| \\<v\\> \\*r\\* a\\\\b \\q 1<br>2 \\<br> |\n")
            .pop()
            .unwrap();
        let g = markdown_grid("f.md", t);
        assert_eq!(g.rows[1], ["<v> *r* a\\b \\q 1\n2 <br>"]);
    }
}

mod docx {
    use crate::table::Facts;
    use anyhow::Result;
    use quick_xml::events::Event;
    use quick_xml::Reader;

    type Table = (Vec<Vec<String>>, Facts);

    fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
        e.attributes()
            .flatten()
            .find(|a| a.key.as_ref() == key)
            .map(|a| a.value.into_owned())
    }

    /// Returns the top-level tables of a DOCX `document.xml`. Nested tables
    /// add their text to the enclosing cell.
    pub fn parse(xml: &str) -> Result<Vec<Table>> {
        let mut reader = Reader::from_str(xml);
        let mut out = Vec::new();
        let mut depth = 0usize;
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut facts = Facts::default();
        let mut cell = String::new();
        let mut in_text = false;
        let mut span = 1usize;
        let mut vmerge_continue = false;
        let mut cell_starts_par = true;
        loop {
            let event = reader.read_event()?;
            let empty = matches!(event, Event::Empty(_));
            match event {
                Event::Eof => break,
                Event::Start(e) | Event::Empty(e) if depth > 0 || e.name().as_ref() == "w:tbl" => {
                    match e.name().as_ref() {
                        "w:tbl" => {
                            depth += 1;
                            if depth == 1 {
                                rows.clear();
                                facts = Facts {
                                    visible_sheets: 1,
                                    ..Facts::default()
                                };
                            }
                        }
                        "w:tr" if depth == 1 => rows.push(Vec::new()),
                        "w:tc" if depth == 1 => {
                            cell.clear();
                            span = 1;
                            vmerge_continue = false;
                            cell_starts_par = true;
                        }
                        "w:gridSpan" if depth == 1 => {
                            span = attr(&e, "w:val").and_then(|v| v.parse().ok()).unwrap_or(1);
                        }
                        "w:vMerge" if depth == 1 => {
                            vmerge_continue = attr(&e, "w:val").as_deref() != Some("restart");
                        }
                        "w:p" => {
                            if !cell_starts_par {
                                cell.push('\n');
                            }
                            cell_starts_par = false;
                        }
                        "w:t" if !empty => in_text = true,
                        "w:br" => cell.push('\n'),
                        // A tab stop definition in `w:tabs` has `w:val`; a tab
                        // character in a run does not.
                        "w:tab" if attr(&e, "w:val").is_none() => cell.push('\t'),
                        "w:drawing" | "w:pict" => facts.images = true,
                        _ => {}
                    }
                }
                Event::Text(t) if in_text && depth > 0 => {
                    cell.push_str(&t);
                }
                Event::GeneralRef(r) if in_text && depth > 0 => {
                    let name: &str = &r;
                    if let Some(c) = r.resolve_char_ref()? {
                        cell.push(c);
                    } else if let Some(s) = quick_xml::escape::resolve_predefined_entity(name) {
                        cell.push_str(s);
                    }
                }
                Event::End(e) if depth > 0 => match e.name().as_ref() {
                    "w:t" => in_text = false,
                    "w:tc" if depth == 1 => {
                        let row_index = rows.len() - 1;
                        let row = rows.last_mut().expect("w:tc inside w:tr");
                        let col = row.len();
                        let text = cell.trim().to_string();
                        if span > 1 && row_index > 0 {
                            facts.data_merges = true;
                        }
                        if vmerge_continue {
                            facts.data_merges = true;
                        }
                        for _ in 0..span {
                            row.push(text.clone());
                        }
                        if vmerge_continue {
                            // Repeat the value from the cell above.
                            let above = rows
                                .get(row_index.wrapping_sub(1))
                                .and_then(|r| r.get(col))
                                .cloned()
                                .unwrap_or_default();
                            let row = rows.last_mut().expect("row");
                            for c in row.iter_mut().skip(col).take(span) {
                                c.clone_from(&above);
                            }
                        }
                    }
                    "w:tbl" => {
                        depth -= 1;
                        if depth == 0 {
                            out.push((std::mem::take(&mut rows), std::mem::take(&mut facts)));
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reads_cells_spans_and_entities() {
            let xml = r#"<w:document><w:body><w:p/><w:tbl>
                <w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>H</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>a &amp; b</w:t></w:r></w:p><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc>
                      <w:tc><w:p><w:r><w:t>d</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl></w:body></w:document>"#;
            let t = parse(xml).unwrap();
            assert_eq!(t.len(), 1);
            assert_eq!(t[0].0, [vec!["H", "H"], vec!["a & b\nc", "d"]]);
            assert!(!t[0].1.data_merges);
        }

        #[test]
        fn breaks_tabs_and_empty_text() {
            let xml = r#"<w:tbl>
                <w:tr><w:tc><w:p><w:pPr><w:tabs><w:tab w:val="left" w:pos="720"/></w:tabs></w:pPr><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:p><w:r><w:t>first</w:t><w:br/><w:t>second</w:t><w:tab/><w:t>third</w:t><w:t/></w:r>junk</w:p></w:tc></w:tr>
                </w:tbl>"#;
            let t = parse(xml).unwrap();
            assert_eq!(t[0].0, [vec!["h"], vec!["first\nsecond\tthird"]]);
        }

        #[test]
        fn horizontal_merge_in_a_body_row_is_a_data_merge() {
            let xml = r#"<w:tbl>
                <w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr>
                <w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>"#;
            let t = parse(xml).unwrap();
            assert_eq!(t[0].0[1], ["x", "x"]);
            assert!(t[0].1.data_merges);
        }

        #[test]
        fn drawing_marks_images() {
            let xml =
                r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:drawing/></w:r></w:p></w:tc></w:tr></w:tbl>"#;
            assert!(parse(xml).unwrap()[0].1.images);
        }

        #[test]
        fn vertical_merge_is_a_data_merge() {
            let xml = r#"<w:tbl>
                <w:tr><w:tc><w:p><w:r><w:t>h</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr>
                <w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc></w:tr>
                </w:tbl>"#;
            let t = parse(xml).unwrap();
            assert_eq!(t[0].0, [vec!["h"], vec!["x"], vec!["x"]]);
            assert!(t[0].1.data_merges);
        }
    }
}

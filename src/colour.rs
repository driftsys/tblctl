//! Decides whether cell colour carries meaning.
//!
//! Colour is decorative when every data cell's colour depends only on its
//! column and the parity of its row (column colours and row bands).

use anyhow::{Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path;

/// Cell colour as (fill, font). Each value is the colour as written in the
/// file: every attribute that identifies it (`rgb`, `theme`, `tint`,
/// `indexed`, `auto`), for example `rgb=FFFF0000` or `theme=4 tint=0.4`. A
/// solid fill with no `fgColor` is written as the sentinel `default`. Values
/// are compared as strings and no theme is resolved.
pub type Colour = (Option<String>, Option<String>);

/// Returns `true` when two data cells with the same column and the same row
/// parity have different colours. Each item is (row, col, colour).
pub fn varies(cells: &[(u32, u32, Colour)]) -> bool {
    let mut seen: HashMap<(u32, u32), &Colour> = HashMap::new();
    for (row, col, colour) in cells {
        match seen.get(&(*col, row % 2)) {
            Some(first) if *first != colour => return true,
            Some(_) => {}
            None => {
                seen.insert((*col, row % 2), colour);
            }
        }
    }
    false
}

/// Rectangle of zero-based cells: (first row, first col, last row, last col).
type Area = (u32, u32, u32, u32);

/// Colour facts of one sheet: enough to resolve the colour of any cell.
pub struct SheetColour {
    /// Colour of each cell style index (`cellXfs` order).
    styles: Vec<Colour>,
    /// Style index of each cell present in the sheet XML.
    cells: HashMap<(u32, u32), usize>,
    /// Style index of each row with `customFormat`.
    rows: HashMap<u32, usize>,
    /// Column ranges (first, last, style index), zero-based.
    cols: Vec<(u32, u32, usize)>,
    /// Ranges covered by conditional formatting.
    conditional: Vec<Area>,
}

impl SheetColour {
    fn colour_of(&self, style: usize) -> Colour {
        self.styles.get(style).cloned().unwrap_or((None, None))
    }

    /// Colour of one cell: its own style, else its row style, else its
    /// column style, else style 0.
    fn cell_colour(&self, row: u32, col: u32) -> Colour {
        let style = self
            .cells
            .get(&(row, col))
            .or_else(|| self.rows.get(&row))
            .copied()
            .or_else(|| {
                self.cols
                    .iter()
                    .find(|(a, b, _)| (*a..=*b).contains(&col))
                    .map(|(_, _, s)| *s)
            })
            .unwrap_or(0);
        self.colour_of(style)
    }

    /// Returns `true` when the data cells carry meaningful colour. `start`
    /// and `end` are the zero-based (row, col) corners of the used range;
    /// row `start.0` is the header. Colour is meaningful when a
    /// conditional-format range overlaps a data cell or `varies` is true.
    pub fn meaningful(&self, start: (u32, u32), end: (u32, u32)) -> bool {
        let first_row = start.0.saturating_add(1);
        if first_row > end.0 {
            return false;
        }
        let overlaps = self.conditional.iter().any(|&(r0, c0, r1, c1)| {
            r0 <= end.0 && r1 >= first_row && c0 <= end.1 && c1 >= start.1
        });
        if overlaps {
            return true;
        }
        let mut cells = Vec::new();
        for row in first_row..=end.0 {
            for col in start.1..=end.1 {
                cells.push((row, col, self.cell_colour(row, col)));
            }
        }
        varies(&cells)
    }
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    attr_by(e, |k| k == key)
}

/// Like `attr`, but matches the attribute name without its namespace prefix.
fn attr_local(e: &BytesStart, key: &str) -> Option<String> {
    attr_by(e, |k| k.rsplit(':').next() == Some(key))
}

fn attr_by(e: &BytesStart, matches: impl Fn(&str) -> bool) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| matches(a.key.as_ref()))
        .map(|a| {
            let raw = a.value.into_owned();
            quick_xml::escape::unescape(&raw).map_or_else(|_| raw.clone(), |v| v.into_owned())
        })
}

/// Encodes a `color`, `fgColor` or `bgColor` element as a string holding
/// each attribute that identifies the colour.
fn colour_text(e: &BytesStart) -> String {
    ["rgb", "theme", "tint", "indexed", "auto"]
        .iter()
        .filter_map(|k| attr(e, k).map(|v| format!("{k}={v}")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn zip_text<R: std::io::Read + std::io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<Option<String>> {
    match zip.by_name(name) {
        Ok(mut f) => {
            let mut xml = String::new();
            f.read_to_string(&mut xml)
                .with_context(|| name.to_string())?;
            Ok(Some(xml))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(e).with_context(|| name.to_string()),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    None,
    Fonts,
    Fills,
    CellXfs,
}

/// Reads `xl/styles.xml` into one `Colour` per `cellXfs` entry.
fn styles(xml: &str) -> Result<Vec<Colour>> {
    let mut reader = Reader::from_str(xml);
    let mut section = Section::None;
    let mut fonts: Vec<Option<String>> = Vec::new();
    let mut fills: Vec<Option<String>> = Vec::new();
    let mut xfs: Vec<(usize, usize)> = Vec::new();
    let mut filled = false;
    loop {
        let event = reader.read_event()?;
        match event {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                "fonts" => section = Section::Fonts,
                "fills" => section = Section::Fills,
                "cellXfs" => section = Section::CellXfs,
                "font" if section == Section::Fonts => fonts.push(None),
                "fill" if section == Section::Fills => {
                    fills.push(None);
                    filled = false;
                }
                "color" if section == Section::Fonts => {
                    if attr(&e, "auto").as_deref() != Some("1") {
                        if let Some(last) = fonts.last_mut() {
                            *last = Some(colour_text(&e));
                        }
                    }
                }
                "patternFill" if section == Section::Fills => {
                    filled = !matches!(attr(&e, "patternType").as_deref(), None | Some("none"));
                    if filled {
                        if let Some(last) = fills.last_mut() {
                            *last = Some("default".to_string());
                        }
                    }
                }
                "fgColor" if section == Section::Fills && filled => {
                    if let Some(last) = fills.last_mut() {
                        *last = Some(colour_text(&e));
                    }
                }
                "xf" if section == Section::CellXfs => {
                    let id = |k: &str| attr(&e, k).and_then(|v| v.parse().ok()).unwrap_or(0);
                    xfs.push((id("fillId"), id("fontId")));
                }
                _ => {}
            },
            Event::End(e) if matches!(e.local_name().as_ref(), "fonts" | "fills" | "cellXfs") => {
                section = Section::None;
            }
            _ => {}
        }
    }
    Ok(xfs
        .into_iter()
        .map(|(fill, font)| {
            (
                fills.get(fill).cloned().flatten(),
                fonts.get(font).cloned().flatten(),
            )
        })
        .collect())
}

/// Parses `A1` into zero-based (row, col). A column-only reference (`A`)
/// has no row; a row-only reference (`1`) has no column.
fn parse_ref(text: &str) -> Option<(Option<u32>, Option<u32>)> {
    let letters = text.chars().take_while(|c| c.is_ascii_alphabetic()).count();
    let (l, d) = text.split_at(letters);
    if (l.is_empty() && d.is_empty()) || !d.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let col = (!l.is_empty()).then(|| {
        l.to_ascii_uppercase().bytes().fold(0u32, |n, b| {
            n.saturating_mul(26).saturating_add(u32::from(b - b'A' + 1))
        }) - 1
    });
    let row = if d.is_empty() {
        None
    } else {
        Some(d.parse::<u32>().ok()?.checked_sub(1)?)
    };
    Some((row, col))
}

/// Parses a `sqref` such as `A1:A1 B3` into areas.
fn parse_sqref(sqref: &str) -> Vec<Area> {
    sqref
        .split_whitespace()
        .filter_map(|part| {
            let (a, b) = part.split_once(':').unwrap_or((part, part));
            let (ar, ac) = parse_ref(a)?;
            let (br, bc) = parse_ref(b)?;
            Some((
                ar.unwrap_or(0),
                ac.unwrap_or(0),
                br.unwrap_or(u32::MAX),
                bc.unwrap_or(u32::MAX),
            ))
        })
        .collect()
}

fn sheet_colour(xml: &str, styles: &[Colour]) -> Result<SheetColour> {
    let mut reader = Reader::from_str(xml);
    let mut out = SheetColour {
        styles: styles.to_vec(),
        cells: HashMap::new(),
        rows: HashMap::new(),
        cols: Vec::new(),
        conditional: Vec::new(),
    };
    let mut row = 0u32;
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) => {
                let num = |k: &str| attr(&e, k).and_then(|v| v.parse::<u32>().ok());
                match e.local_name().as_ref() {
                    "col" => {
                        if let (Some(min), Some(max), Some(style)) =
                            (num("min"), num("max"), num("style"))
                        {
                            out.cols.push((
                                min.saturating_sub(1),
                                max.saturating_sub(1),
                                style as usize,
                            ));
                        }
                    }
                    "row" => {
                        row = num("r").map_or(row.saturating_add(1), |r| r.saturating_sub(1));
                        let custom =
                            matches!(attr(&e, "customFormat").as_deref(), Some("1" | "true"));
                        if let (true, Some(s)) = (custom, num("s")) {
                            out.rows.insert(row, s as usize);
                        }
                    }
                    "c" => {
                        if let Some((Some(r), Some(c))) =
                            attr(&e, "r").as_deref().and_then(parse_ref)
                        {
                            out.cells.insert((r, c), num("s").unwrap_or(0) as usize);
                        }
                    }
                    "conditionalFormatting" => {
                        if let Some(sqref) = attr(&e, "sqref") {
                            out.conditional.extend(parse_sqref(&sqref));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Resolves a relationship target to a path inside the zip.
fn part_path(target: &str) -> String {
    match target.strip_prefix('/') {
        Some(abs) => abs.to_string(),
        None => format!("xl/{target}"),
    }
}

/// Colour facts of each sheet of an XLSX or XLSM file, keyed by sheet name,
/// read from the styles part, the workbook and the sheet parts.
pub fn xlsx_meaningful(path: &Path) -> Result<HashMap<String, SheetColour>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let styles = match zip_text(&mut zip, "xl/styles.xml")? {
        Some(xml) => self::styles(&xml).context("xl/styles.xml")?,
        None => Vec::new(),
    };
    let workbook = zip_text(&mut zip, "xl/workbook.xml")?.context("no xl/workbook.xml")?;
    let rels = zip_text(&mut zip, "xl/_rels/workbook.xml.rels")?
        .context("no xl/_rels/workbook.xml.rels")?;

    let mut targets: HashMap<String, String> = HashMap::new();
    let mut reader = Reader::from_str(&rels);
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == "Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, "Id"), attr(&e, "Target")) {
                    targets.insert(id, part_path(&target));
                }
            }
            _ => {}
        }
    }

    let mut sheets: Vec<(String, String)> = Vec::new();
    let mut reader = Reader::from_str(&workbook);
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == "sheet" => {
                if let (Some(name), Some(id)) = (attr(&e, "name"), attr_local(&e, "id")) {
                    sheets.push((name, id));
                }
            }
            _ => {}
        }
    }

    let mut out = HashMap::new();
    for (name, id) in sheets {
        let Some(part) = targets.get(&id) else {
            continue;
        };
        let Some(xml) = zip_text(&mut zip, part)? else {
            continue;
        };
        out.insert(
            name.clone(),
            sheet_colour(&xml, &styles).with_context(|| format!("sheet {name}"))?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red() -> Colour {
        (Some("FFFF0000".to_string()), None)
    }

    fn none() -> Colour {
        (None, None)
    }

    fn grid(f: impl Fn(u32, u32) -> Colour) -> Vec<(u32, u32, Colour)> {
        let mut cells = Vec::new();
        for row in 1..=6 {
            for col in 0..=2 {
                cells.push((row, col, f(row, col)));
            }
        }
        cells
    }

    #[test]
    fn uniform_colour_is_decorative() {
        assert!(!varies(&grid(|_, _| red())));
    }

    #[test]
    fn column_colour_is_decorative() {
        let cells = grid(|_, col| if col == 1 { red() } else { none() });
        assert!(!varies(&cells));
    }

    #[test]
    fn row_bands_are_decorative() {
        let cells = grid(|row, _| if row % 2 == 0 { red() } else { none() });
        assert!(!varies(&cells));
    }

    #[test]
    fn bands_plus_column_colour_is_decorative() {
        let cells = grid(|row, col| {
            let fill = (row % 2 == 0).then(|| "FFEEEEEE".to_string());
            let font = (col == 2).then(|| "FF0000FF".to_string());
            (fill, font)
        });
        assert!(!varies(&cells));
    }

    #[test]
    fn one_red_cell_is_meaningful() {
        let cells = grid(|row, col| if (row, col) == (3, 1) { red() } else { none() });
        assert!(varies(&cells));
    }

    #[test]
    fn coloured_total_row_is_meaningful() {
        let cells = grid(|row, _| if row == 6 { red() } else { none() });
        assert!(varies(&cells));
    }

    use std::io::Write as _;

    fn zip_xlsx(parts: &[(&str, &str)]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut z = zip::ZipWriter::new(file.reopen().unwrap());
        for (name, body) in parts {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        file
    }

    const WORKBOOK: &str = r#"<workbook xmlns:r="r"><sheets>
        <sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    const RELS: &str = r#"<Relationships>
        <Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;

    /// Styles: 0 plain, 1 red fill, 2 red font, 3 fill with patternType none,
    /// 4 theme fill tint 0.4, 5 theme fill tint 0.6.
    const STYLES: &str = r#"<styleSheet>
      <fonts count="2">
        <font><sz val="11"/></font>
        <font><color rgb="FFFF0000"/></font>
      </fonts>
      <fills count="5">
        <fill><patternFill patternType="none"/></fill>
        <fill><patternFill patternType="solid"><fgColor rgb="FFFF0000"/></patternFill></fill>
        <fill><patternFill patternType="none"><fgColor rgb="FFFF0000"/></patternFill></fill>
        <fill><patternFill patternType="solid"><fgColor theme="4" tint="0.4"/></patternFill></fill>
        <fill><patternFill patternType="solid"><fgColor theme="4" tint="0.6"/></patternFill></fill>
      </fills>
      <cellXfs count="6">
        <xf fontId="0" fillId="0"/>
        <xf fontId="0" fillId="1"/>
        <xf fontId="1" fillId="0"/>
        <xf fontId="0" fillId="2"/>
        <xf fontId="0" fillId="3"/>
        <xf fontId="0" fillId="4"/>
      </cellXfs>
      <dxfs count="1"><dxf><fill><patternFill><bgColor rgb="FF00FF00"/></patternFill></fill></dxf></dxfs>
    </styleSheet>"#;

    fn sheet(body: &str) -> String {
        format!("<worksheet>{body}</worksheet>")
    }

    fn read(sheet_xml: &str) -> SheetColour {
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", STYLES),
            ("xl/worksheets/sheet1.xml", sheet_xml),
        ]);
        xlsx_meaningful(f.path()).unwrap().remove("S").unwrap()
    }

    /// Header row 1, data rows 2..=5, columns A..=C (zero-based (0,0)-(4,2)).
    fn plain_rows(styled: &[(&str, u32)]) -> String {
        let mut out = String::from("<sheetData>");
        for r in 1..=5 {
            out.push_str(&format!("<row r=\"{r}\">"));
            for c in ["A", "B", "C"] {
                let s = styled
                    .iter()
                    .find(|(a, _)| *a == format!("{c}{r}"))
                    .map_or(0, |(_, s)| *s);
                out.push_str(&format!("<c r=\"{c}{r}\" s=\"{s}\"><v>1</v></c>"));
            }
            out.push_str("</row>");
        }
        out.push_str("</sheetData>");
        out
    }

    const START: (u32, u32) = (0, 0);
    const END: (u32, u32) = (4, 2);

    #[test]
    fn fill_on_one_data_cell_is_meaningful() {
        let sc = read(&sheet(&plain_rows(&[("B3", 1)])));
        assert!(sc.meaningful(START, END));
    }

    #[test]
    fn pattern_type_none_is_no_fill() {
        let sc = read(&sheet(&plain_rows(&[("B3", 3)])));
        assert!(!sc.meaningful(START, END));
    }

    #[test]
    fn font_colour_counts() {
        let sc = read(&sheet(&plain_rows(&[("C4", 2)])));
        assert!(sc.meaningful(START, END));
    }

    #[test]
    fn missing_s_uses_style_zero() {
        // A2 is present without `s` inside a filled row: it is style 0, not
        // the row style. Rows 2 and 4 share a parity.
        let body = r#"<sheetData><row r="1"><c r="A1"/></row>
            <row r="2" s="1" customFormat="1"><c r="A2"><v>1</v></c></row>
            <row r="3"><c r="A3" s="0"><v>1</v></c></row>
            <row r="4"><c r="A4" s="0"><v>1</v></c></row></sheetData>"#;
        assert!(!read(&sheet(body)).meaningful((0, 0), (3, 0)));
    }

    #[test]
    fn header_colour_is_ignored() {
        let sc = read(&sheet(&plain_rows(&[("A1", 1), ("B1", 2)])));
        assert!(!sc.meaningful(START, END));
    }

    #[test]
    fn empty_cell_takes_row_then_column_style() {
        // Row 3 has a fill; A3 has no `s`, and B3 and C3 are absent. Rows 3
        // and 5 share a parity, so the fill is not a row band.
        let body = r#"<sheetData>
            <row r="1"><c r="A1"/></row>
            <row r="2"><c r="A2"/><c r="B2"/><c r="C2"/></row>
            <row r="3" s="1" customFormat="1"><c r="A3"/></row>
            <row r="4"><c r="A4"/><c r="B4"/><c r="C4"/></row>
            <row r="5"><c r="A5"/><c r="B5"/><c r="C5"/></row></sheetData>"#;
        // A3 has no `s`, so only B3 and C3 can carry the row style: confirm
        // by checking a range whose only coloured cells are absent ones.
        assert!(read(&sheet(body)).meaningful((0, 1), (4, 2)));

        // A column style covers every row of one column: decorative.
        let body = r#"<cols><col min="2" max="2" style="1"/></cols><sheetData>
            <row r="1"><c r="A1"/></row>
            <row r="2"><c r="A2"/></row>
            <row r="3"><c r="A3"/></row>
            <row r="4"><c r="A4"/></row></sheetData>"#;
        assert!(!read(&sheet(body)).meaningful((0, 0), (3, 2)));
    }

    #[test]
    fn cell_style_beats_column_style() {
        // Column B is red. B3 is present with s="0", so it is plain and the
        // column is not uniform: meaningful only if the cell style wins.
        let body = r#"<cols><col min="2" max="2" style="1"/></cols><sheetData>
            <row r="1"><c r="A1"/></row>
            <row r="2"><c r="A2"/></row>
            <row r="3"><c r="B3" s="0"/></row>
            <row r="4"><c r="A4"/></row>
            <row r="5"><c r="A5"/></row></sheetData>"#;
        assert!(read(&sheet(body)).meaningful((0, 1), (4, 1)));
    }

    #[test]
    fn row_style_beats_column_style() {
        // Column B is red; row 3 is plain (style 0, customFormat) and B3 is
        // absent. Row 3 has the same parity as row 5, whose B5 is absent and
        // takes the column style: meaningful only if the row style wins.
        let body = r#"<cols><col min="2" max="2" style="1"/></cols><sheetData>
            <row r="1"><c r="A1"/></row>
            <row r="2"><c r="A2"/></row>
            <row r="3" s="0" customFormat="1"><c r="A3"/></row>
            <row r="4"><c r="A4"/></row>
            <row r="5"><c r="A5"/></row></sheetData>"#;
        assert!(read(&sheet(body)).meaningful((0, 1), (4, 1)));
    }

    #[test]
    fn prefixed_spreadsheetml_is_read() {
        let workbook = r#"<x:workbook xmlns:x="m" xmlns:rel="r"><x:sheets>
            <x:sheet name="S" sheetId="1" rel:id="rId1"/></x:sheets></x:workbook>"#;
        let styles = r#"<x:styleSheet xmlns:x="m">
            <x:fonts count="1"><x:font><x:sz val="11"/></x:font></x:fonts>
            <x:fills count="2">
              <x:fill><x:patternFill patternType="none"/></x:fill>
              <x:fill><x:patternFill patternType="solid"><x:fgColor rgb="FFFF0000"/></x:patternFill></x:fill>
            </x:fills>
            <x:cellXfs count="2"><x:xf fontId="0" fillId="0"/><x:xf fontId="0" fillId="1"/></x:cellXfs>
            </x:styleSheet>"#;
        let sheet_xml = r#"<x:worksheet xmlns:x="m"><x:sheetData>
            <x:row r="1"><x:c r="A1"/></x:row>
            <x:row r="2"><x:c r="A2" s="1"/></x:row>
            <x:row r="3"><x:c r="A3" s="0"/></x:row>
            <x:row r="4"><x:c r="A4" s="0"/></x:row></x:sheetData></x:worksheet>"#;
        let rels = r#"<Relationships>
            <Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#;
        let f = zip_xlsx(&[
            ("xl/workbook.xml", workbook),
            ("xl/_rels/workbook.xml.rels", rels),
            ("xl/styles.xml", styles),
            ("xl/worksheets/sheet1.xml", sheet_xml),
        ]);
        let sc = xlsx_meaningful(f.path()).unwrap().remove("S").unwrap();
        assert!(sc.meaningful((0, 0), (3, 0)));
    }

    #[test]
    fn tint_makes_a_different_colour() {
        let diff = read(&sheet(&plain_rows(&[("B2", 4), ("B4", 5)])));
        assert!(diff.meaningful(START, END));
        let same = read(&sheet(&plain_rows(&[("B2", 4), ("B4", 4)])));
        assert!(!same.meaningful(START, END));
    }

    #[test]
    fn conditional_format_on_data_is_meaningful() {
        let body = format!(
            "{}<conditionalFormatting sqref=\"A1:A1 B3\"><cfRule type=\"x\"/></conditionalFormatting>",
            plain_rows(&[])
        );
        assert!(read(&sheet(&body)).meaningful(START, END));
    }

    #[test]
    fn conditional_format_on_header_only_is_not() {
        let body = format!(
            "{}<conditionalFormatting sqref=\"A1:C1\"><cfRule type=\"x\"/></conditionalFormatting>",
            plain_rows(&[])
        );
        assert!(!read(&sheet(&body)).meaningful(START, END));
    }

    #[test]
    fn absolute_and_relative_targets_resolve() {
        let rels = r#"<Relationships>
            <Relationship Id="rId1" Target="/xl/worksheets/sheet1.xml"/></Relationships>"#;
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", rels),
            ("xl/styles.xml", STYLES),
            (
                "xl/worksheets/sheet1.xml",
                &sheet(&plain_rows(&[("B3", 1)])),
            ),
        ]);
        let sc = xlsx_meaningful(f.path()).unwrap().remove("S").unwrap();
        assert!(sc.meaningful(START, END));
        // The relative form is exercised by every other test.
    }

    #[test]
    fn missing_styles_part_means_no_colour() {
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", RELS),
            (
                "xl/worksheets/sheet1.xml",
                &sheet(&plain_rows(&[("B3", 1)])),
            ),
        ]);
        let sc = xlsx_meaningful(f.path()).unwrap().remove("S").unwrap();
        assert!(!sc.meaningful(START, END));
    }

    #[test]
    fn missing_sheet_part_is_skipped() {
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", STYLES),
        ]);
        assert!(xlsx_meaningful(f.path()).unwrap().is_empty());
    }
}

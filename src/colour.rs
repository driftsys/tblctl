//! Decides whether cell colour carries meaning.
//!
//! With 4 or more data rows, colour is decorative when every data cell's
//! colour depends only on its column and the parity of its row (column
//! colours and row bands). With 2 or 3 data rows, cells are compared by column
//! only. With 1 data row, any data cell whose colour differs from the colour
//! of style 0 is meaningful.

use anyhow::{Context, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path;

/// Cell colour as (fill, font). Each value is the colour as written in the
/// file: every attribute that identifies it (`rgb`, `theme`, `tint`,
/// `indexed`, `auto`), for example `rgb=FFFF0000` or `theme=4 tint=0.4`. A
/// fill whose `patternType` is not `none` and that has no `fgColor` is written
/// as the sentinel `default`. `auto` only reaches the fill string: a font
/// `color` with `auto="1"` becomes `None`. Values are compared as strings and
/// no theme is resolved.
pub type Colour = (Option<String>, Option<String>);

/// Returns `true` when the data cells carry colour that is not decoration.
/// Each item is (row, col, colour).
///
/// With 4 or more data rows, colour is decorative when it depends only on the
/// column and the parity of the row, so two cells with the same column and
/// the same row parity but different colours make it meaningful. With fewer
/// than 4 data rows, row parity is not evidence of banding, so cells are
/// compared by column only. With exactly 1 data row, any coloured cell makes
/// it meaningful.
pub fn varies(cells: &[(u32, u32, Colour)]) -> bool {
    let mut rows: Vec<u32> = cells.iter().map(|(row, _, _)| *row).collect();
    rows.sort_unstable();
    rows.dedup();
    if rows.len() == 1 {
        return cells.iter().any(|(_, _, colour)| *colour != (None, None));
    }
    let banded = rows.len() >= 4;
    let mut seen: HashMap<(u32, u32), &Colour> = HashMap::new();
    for (row, col, colour) in cells {
        let key = (*col, if banded { row % 2 } else { 0 });
        match seen.get(&key) {
            Some(first) if *first != colour => return true,
            Some(_) => {}
            None => {
                seen.insert(key, colour);
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

    /// Colour of one cell. A `<c>` element without `s` uses style 0. A cell
    /// with no `<c>` element uses its row style (with `customFormat`), else
    /// its column style, else style 0.
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
    /// Cells with the colour of style 0 are treated as uncoloured.
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
        // The colour of style 0 is the workbook default (writers such as
        // Excel give the default font an explicit theme colour), so a cell
        // with that colour counts as having none.
        let plain = self.colour_of(0);
        let mut cells = Vec::new();
        for row in first_row..=end.0 {
            for col in start.1..=end.1 {
                let colour = self.cell_colour(row, col);
                let colour = if colour == plain {
                    (None, None)
                } else {
                    colour
                };
                cells.push((row, col, colour));
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
    // Position of the next row or cell without `r`, numbered as `calamine`
    // numbers it, so that colour and text land on the same cell.
    let (mut row, mut col) = (0u32, 0u32);
    let mut conditional = false;
    loop {
        let event = reader.read_event()?;
        let empty = matches!(event, Event::Empty(_));
        match event {
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
                        row = num("r").map_or(row, |r| r.saturating_sub(1));
                        col = 0;
                        let custom =
                            matches!(attr(&e, "customFormat").as_deref(), Some("1" | "true"));
                        if let (true, Some(s)) = (custom, num("s")) {
                            out.rows.insert(row, s as usize);
                        }
                        if empty {
                            row = row.saturating_add(1);
                        }
                    }
                    "c" => {
                        let (r, c) = match attr(&e, "r").as_deref().and_then(parse_ref) {
                            Some((Some(r), Some(c))) => (r, c),
                            _ => (row, col),
                        };
                        out.cells.insert((r, c), num("s").unwrap_or(0) as usize);
                        col = c.saturating_add(1);
                    }
                    "conditionalFormatting" => {
                        conditional = !empty;
                        if let Some(sqref) = attr(&e, "sqref") {
                            out.conditional.extend(parse_sqref(&sqref));
                        }
                    }
                    // The Excel 2010 form (`x14` in `extLst`) holds the range
                    // in a child element instead of an attribute.
                    "sqref" if conditional && !empty => {
                        let text = reader.read_text(e.name())?;
                        out.conditional.extend(parse_sqref(&text));
                    }
                    _ => {}
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                "row" => row = row.saturating_add(1),
                "conditionalFormatting" => conditional = false,
                _ => {}
            },
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
///
/// Colour is extra information: a part that is missing or cannot be parsed
/// never makes the file unreadable. The map is empty when the zip cannot be
/// opened, when the styles part exists but cannot be read or parsed, or when
/// the workbook or relationships part is missing, unreadable or not
/// parseable. A missing styles part is not an error: reading continues with
/// no styles, and conditional formats are still read. A sheet whose relationship
/// is missing, whose part is missing or cannot be read as text, or whose XML
/// cannot be parsed has no entry; the other sheets are still read.
pub fn xlsx_meaningful(path: &Path) -> HashMap<String, SheetColour> {
    try_xlsx_meaningful(path).unwrap_or_default()
}

fn try_xlsx_meaningful(path: &Path) -> Result<HashMap<String, SheetColour>> {
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
        let Ok(Some(xml)) = zip_text(&mut zip, part) else {
            continue;
        };
        if let Ok(colour) = sheet_colour(&xml, &styles) {
            out.insert(name, colour);
        }
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

    #[test]
    fn one_data_row_with_one_red_cell_is_meaningful() {
        let cells = vec![(1, 0, none()), (1, 1, red())];
        assert!(varies(&cells));
    }

    #[test]
    fn one_data_row_without_colour_is_not_meaningful() {
        let cells = vec![(1, 0, none()), (1, 1, none())];
        assert!(!varies(&cells));
    }

    #[test]
    fn two_data_rows_with_different_colours_in_a_column_are_meaningful() {
        let green = (Some("FF00FF00".to_string()), None);
        let cells = vec![(1, 0, green), (2, 0, red())];
        assert!(varies(&cells));
    }

    #[test]
    fn two_data_rows_with_one_column_colour_are_not_meaningful() {
        let cells = vec![(1, 0, red()), (2, 0, red()), (1, 1, none()), (2, 1, none())];
        assert!(!varies(&cells));
    }

    #[test]
    fn three_data_rows_green_red_green_are_meaningful() {
        let green = || (Some("FF00FF00".to_string()), None);
        let cells = vec![(1, 0, green()), (2, 0, red()), (3, 0, green())];
        assert!(varies(&cells));
    }

    #[test]
    fn four_data_rows_with_row_bands_are_decorative() {
        let cells: Vec<_> = (1..=4)
            .map(|row| (row, 0, if row % 2 == 0 { red() } else { none() }))
            .collect();
        assert!(!varies(&cells));
    }

    use std::io::Write as _;

    fn zip_xlsx(parts: &[(&str, &str)]) -> tempfile::NamedTempFile {
        let parts: Vec<(&str, &[u8])> = parts.iter().map(|(n, b)| (*n, b.as_bytes())).collect();
        zip_xlsx_bytes(&parts)
    }

    fn zip_xlsx_bytes(parts: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut z = zip::ZipWriter::new(file.reopen().unwrap());
        for (name, body) in parts {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(body).unwrap();
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
        xlsx_meaningful(f.path()).remove("S").unwrap()
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
        let sc = xlsx_meaningful(f.path()).remove("S").unwrap();
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
        let sc = xlsx_meaningful(f.path()).remove("S").unwrap();
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
        let sc = xlsx_meaningful(f.path()).remove("S").unwrap();
        assert!(!sc.meaningful(START, END));
    }

    #[test]
    fn missing_sheet_part_is_skipped() {
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", STYLES),
        ]);
        assert!(xlsx_meaningful(f.path()).is_empty());
    }

    #[test]
    fn missing_workbook_part_gives_no_facts() {
        let f = zip_xlsx(&[
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", STYLES),
            (
                "xl/worksheets/sheet1.xml",
                &sheet(&plain_rows(&[("B3", 1)])),
            ),
        ]);
        assert!(xlsx_meaningful(f.path()).is_empty());
    }

    #[test]
    fn malformed_workbook_or_styles_part_gives_no_facts() {
        let sheet_xml = sheet(&plain_rows(&[("B3", 1)]));
        let bad = "<a></b>";
        let f = zip_xlsx(&[
            ("xl/workbook.xml", bad),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", STYLES),
            ("xl/worksheets/sheet1.xml", &sheet_xml),
        ]);
        assert!(xlsx_meaningful(f.path()).is_empty());
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", bad),
            ("xl/styles.xml", STYLES),
            ("xl/worksheets/sheet1.xml", &sheet_xml),
        ]);
        assert!(xlsx_meaningful(f.path()).is_empty());
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", bad),
            ("xl/worksheets/sheet1.xml", &sheet_xml),
        ]);
        assert!(xlsx_meaningful(f.path()).is_empty());
    }

    #[test]
    fn malformed_sheet_part_is_skipped_and_others_are_read() {
        // The bad sheet comes first in workbook order, so the valid sheet
        // after it is read only if the loop continues past the bad one.
        let workbook = r#"<workbook xmlns:r="r"><sheets>
            <sheet name="Bad" sheetId="2" r:id="rId2"/>
            <sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        let rels = r#"<Relationships>
            <Relationship Id="rId1" Target="worksheets/sheet1.xml"/>
            <Relationship Id="rId2" Target="worksheets/sheet2.xml"/></Relationships>"#;
        let f = zip_xlsx(&[
            ("xl/workbook.xml", workbook),
            ("xl/_rels/workbook.xml.rels", rels),
            ("xl/styles.xml", STYLES),
            (
                "xl/worksheets/sheet1.xml",
                &sheet(&plain_rows(&[("B3", 1)])),
            ),
            ("xl/worksheets/sheet2.xml", "<worksheet><sheetData></row>"),
        ]);
        let mut facts = xlsx_meaningful(f.path());
        assert!(!facts.contains_key("Bad"));
        assert!(facts.remove("S").unwrap().meaningful(START, END));
    }

    #[test]
    fn sheet_part_that_is_not_text_is_skipped_and_others_are_read() {
        // Bytes ff fe fd are not valid UTF-8, so the part cannot be read as
        // text. Only that sheet loses its colour result.
        let workbook = r#"<workbook xmlns:r="r"><sheets>
            <sheet name="Bad" sheetId="2" r:id="rId2"/>
            <sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
        let rels = r#"<Relationships>
            <Relationship Id="rId1" Target="worksheets/sheet1.xml"/>
            <Relationship Id="rId2" Target="worksheets/sheet2.xml"/></Relationships>"#;
        let good = sheet(&plain_rows(&[("B3", 1)]));
        let f = zip_xlsx_bytes(&[
            ("xl/workbook.xml", workbook.as_bytes()),
            ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
            ("xl/styles.xml", STYLES.as_bytes()),
            ("xl/worksheets/sheet1.xml", good.as_bytes()),
            ("xl/worksheets/sheet2.xml", &[0xff, 0xfe, 0xfd]),
        ]);
        let mut facts = xlsx_meaningful(f.path());
        assert!(!facts.contains_key("Bad"));
        assert!(facts.remove("S").unwrap().meaningful(START, END));
    }

    /// Styles with one fill per entry of `fgs`, each a `fgColor` attribute
    /// list; style 0 is plain and style n uses fill n.
    fn read_fills(fgs: &[&str], sheet_xml: &str) -> SheetColour {
        let mut fills = String::from(r#"<fill><patternFill patternType="none"/></fill>"#);
        let mut xfs = String::from(r#"<xf fontId="0" fillId="0"/>"#);
        for (i, fg) in fgs.iter().enumerate() {
            fills.push_str(&format!(
                r#"<fill><patternFill patternType="solid"><fgColor {fg}/></patternFill></fill>"#
            ));
            xfs.push_str(&format!(r#"<xf fontId="0" fillId="{}"/>"#, i + 1));
        }
        let styles = format!(
            "<styleSheet><fonts><font><sz val=\"11\"/></font></fonts>\
             <fills>{fills}</fills><cellXfs>{xfs}</cellXfs></styleSheet>"
        );
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", &styles),
            ("xl/worksheets/sheet1.xml", sheet_xml),
        ]);
        xlsx_meaningful(f.path()).remove("S").unwrap()
    }

    #[test]
    fn different_theme_colours_without_tint_differ() {
        let fgs = [r#"theme="4""#, r#"theme="5""#];
        let diff = read_fills(&fgs, &sheet(&plain_rows(&[("B2", 1), ("B4", 2)])));
        assert!(diff.meaningful(START, END));
        let same = read_fills(&fgs, &sheet(&plain_rows(&[("B2", 1), ("B4", 1)])));
        assert!(!same.meaningful(START, END));
    }

    #[test]
    fn different_indexed_colours_differ() {
        let fgs = [r#"indexed="10""#, r#"indexed="11""#];
        let diff = read_fills(&fgs, &sheet(&plain_rows(&[("B2", 1), ("B4", 2)])));
        assert!(diff.meaningful(START, END));
        let same = read_fills(&fgs, &sheet(&plain_rows(&[("B2", 1), ("B4", 1)])));
        assert!(!same.meaningful(START, END));
    }

    fn with_cf(sqref: &str) -> SheetColour {
        read(&sheet(&format!(
            "{}<conditionalFormatting sqref=\"{sqref}\"><cfRule type=\"x\"/></conditionalFormatting>",
            plain_rows(&[])
        )))
    }

    #[test]
    fn conditional_format_outside_the_table_is_not_meaningful() {
        // Table C2:E6 is zero-based (1,2)-(5,4) with header row 2.
        let (start, end) = ((1, 2), (5, 4));
        // Left of the table.
        assert!(!with_cf("A3:B6").meaningful(start, end));
        // Right of the table.
        assert!(!with_cf("F3:G6").meaningful(start, end));
        // Below the table.
        assert!(!with_cf("C7:E9").meaningful(start, end));
        // Above the table (header row and rows before it).
        assert!(!with_cf("C1:E2").meaningful(start, end));
        // Control: inside the data area.
        assert!(with_cf("D4").meaningful(start, end));
    }

    #[test]
    fn conditional_format_on_an_edge_data_cell_is_meaningful() {
        // Table C2:E6 is zero-based (1,2)-(5,4), header row 2, data rows 3-6.
        let (start, end) = ((1, 2), (5, 4));
        // Last data row only.
        assert!(with_cf("C6").meaningful(start, end));
        // First data row only.
        assert!(with_cf("C3").meaningful(start, end));
        // Last column only.
        assert!(with_cf("E4").meaningful(start, end));
        // First column only.
        assert!(with_cf("C4").meaningful(start, end));
    }

    #[test]
    fn column_only_sqref_covers_every_row() {
        // Table A1:C6.
        assert!(with_cf("B:B").meaningful((0, 0), (5, 2)));
        // Column E is outside table A1:C6.
        assert!(!with_cf("E:E").meaningful((0, 0), (5, 2)));
    }

    #[test]
    fn row_only_sqref_covers_every_column() {
        // Table B1:D6 is (0,1)-(5,3); row 3 is a data row, row 1 the header.
        assert!(with_cf("3:3").meaningful((0, 1), (5, 3)));
        assert!(!with_cf("1:1").meaningful((0, 1), (5, 3)));
    }

    /// Header row and 4 data rows, columns A and B, no `r` on any `<c>`.
    /// Only the third row's second cell is red.
    fn rows_without_cell_refs(row_attr: impl Fn(u32) -> String) -> String {
        let mut out = String::from("<sheetData>");
        for r in 1..=5 {
            let s = u32::from(r == 3);
            out.push_str(&format!("<row{}><c/><c s=\"{s}\"/></row>", row_attr(r)));
        }
        out + "</sheetData>"
    }

    #[test]
    fn cells_without_a_reference_are_numbered_by_position() {
        let body = rows_without_cell_refs(|r| format!(" r=\"{r}\""));
        let sc = read(&sheet(&body));
        assert!(sc.meaningful((0, 0), (4, 1)));
        assert_eq!(sc.cell_colour(2, 1), sc.colour_of(1));
        assert_eq!(sc.cell_colour(2, 0), sc.colour_of(0));
    }

    #[test]
    fn rows_without_a_reference_are_numbered_from_the_first() {
        let sc = read(&sheet(&rows_without_cell_refs(|_| String::new())));
        assert_eq!(sc.cell_colour(2, 1), sc.colour_of(1));
        assert_eq!(sc.cell_colour(3, 1), sc.colour_of(0));
    }

    #[test]
    fn numbering_continues_after_empty_rows_and_referenced_cells() {
        // An empty `<row/>` takes one row number. A cell with `r` sets the
        // column of the cells after it.
        let body = r#"<sheetData><row/><row/>
            <row><c r="C3"/><c s="1"/></row>
            <row r="7"><c s="1"/></row><row><c s="1"/></row></sheetData>"#;
        let sc = read(&sheet(body));
        let red = sc.colour_of(1);
        assert_eq!(sc.cell_colour(2, 3), red);
        assert_eq!(sc.cell_colour(6, 0), red);
        assert_eq!(sc.cell_colour(7, 0), red);
        assert_eq!(sc.cells.len(), 4);
    }

    /// An Excel 2010 extension block holding one rule on `sqref`.
    fn x14(wrapper: &str, sqref: &str) -> String {
        format!(
            r#"<extLst><ext xmlns:x14="x14"><x14:{wrapper}s><x14:{wrapper} xmlns:xm="xm">
            <x14:cfRule type="iconSet"/><xm:sqref>{sqref}</xm:sqref>
            </x14:{wrapper}></x14:{wrapper}s></ext></extLst>"#
        )
    }

    fn with_x14(wrapper: &str, sqref: &str) -> SheetColour {
        read(&sheet(&(plain_rows(&[]) + &x14(wrapper, sqref))))
    }

    #[test]
    fn extension_conditional_format_on_data_is_meaningful() {
        let sc = with_x14("conditionalFormatting", "A1 B3:B4");
        assert!(sc.meaningful(START, END));
    }

    #[test]
    fn extension_conditional_format_on_header_only_is_not() {
        let sc = with_x14("conditionalFormatting", "A1:C1");
        assert!(!sc.meaningful(START, END));
    }

    #[test]
    fn sqref_outside_conditional_formatting_is_ignored() {
        // Data validations and sparklines also use `<xm:sqref>`.
        let sc = with_x14("dataValidation", "B3");
        assert!(!sc.meaningful(START, END));
        // Also after a conditional-format block has closed.
        let body =
            plain_rows(&[]) + &x14("conditionalFormatting", "A1") + &x14("dataValidation", "B3");
        assert!(!read(&sheet(&body)).meaningful(START, END));
    }

    /// Reads a one-sheet workbook with its own styles part.
    fn read_with_styles(styles: &str, sheet_xml: &str) -> SheetColour {
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", styles),
            ("xl/worksheets/sheet1.xml", sheet_xml),
        ]);
        xlsx_meaningful(f.path()).remove("S").unwrap()
    }

    #[test]
    fn row_style_without_custom_format_is_ignored() {
        let body = r#"<sheetData><row r="3" s="1"/></sheetData>"#;
        let sc = read(&sheet(body));
        assert_eq!(sc.cell_colour(2, 0), sc.colour_of(0));
    }

    #[test]
    fn automatic_font_colour_is_no_colour() {
        let styles = r#"<styleSheet>
            <fonts><font/><font><color auto="1"/></font></fonts>
            <fills><fill/></fills>
            <cellXfs><xf fontId="0" fillId="0"/><xf fontId="1" fillId="0"/></cellXfs>
            </styleSheet>"#;
        assert_eq!(read_with_styles(styles, &sheet("")).colour_of(1), none());
    }

    #[test]
    fn a_pattern_without_a_colour_is_the_default_fill() {
        // A pattern with a type and no `fgColor` is a fill; one without
        // `patternType` is not.
        let styles = r#"<styleSheet><fonts><font/></fonts>
            <fills><fill/><fill><patternFill patternType="gray125"/></fill>
            <fill><patternFill/></fill></fills>
            <cellXfs><xf fontId="0" fillId="0"/><xf fontId="0" fillId="1"/>
            <xf fontId="0" fillId="2"/></cellXfs></styleSheet>"#;
        let sc = read_with_styles(styles, &sheet(""));
        assert_eq!(sc.colour_of(1), (Some("default".to_string()), None));
        assert_eq!(sc.colour_of(2), none());
    }

    #[test]
    fn only_the_colour_of_style_zero_counts_as_uncoloured() {
        // Style 0 has a theme font colour. Style 1 repeats it; style 2 adds
        // a fill to it.
        let styles = r#"<styleSheet>
            <fonts><font><color theme="1"/></font></fonts>
            <fills><fill/><fill><patternFill patternType="solid"><fgColor rgb="FFFF0000"/></patternFill></fill></fills>
            <cellXfs><xf fontId="0" fillId="0"/><xf fontId="0" fillId="0"/>
            <xf fontId="0" fillId="1"/></cellXfs></styleSheet>"#;
        let one_row = |s: u32| {
            sheet(&format!(
                r#"<sheetData><row r="2"><c r="A2" s="{s}"/></row></sheetData>"#
            ))
        };
        let data = ((0, 0), (1, 0));
        assert!(!read_with_styles(styles, &one_row(1)).meaningful(data.0, data.1));
        assert!(read_with_styles(styles, &one_row(2)).meaningful(data.0, data.1));
    }

    #[test]
    fn font_colour_on_a_one_row_table_is_meaningful() {
        let body = r#"<sheetData><row r="2"><c r="A2" s="2"/></row></sheetData>"#;
        assert!(read(&sheet(body)).meaningful((0, 0), (1, 0)));
    }

    #[test]
    fn cells_left_of_an_offset_table_are_ignored() {
        // Table C1:E5. A3 is red and outside it.
        let body = r#"<sheetData><row r="3"><c r="A3" s="1"/></row></sheetData>"#;
        assert!(!read(&sheet(body)).meaningful((0, 2), (4, 4)));
    }

    #[test]
    fn a_column_style_covers_its_whole_range() {
        let sc = read(&sheet(r#"<cols><col min="1" max="3" style="1"/></cols>"#));
        let red = sc.colour_of(1);
        for col in 0..=2 {
            assert_eq!(sc.cell_colour(1, col), red, "column {col}");
        }
        assert_eq!(sc.cell_colour(1, 3), sc.colour_of(0));
    }

    #[test]
    fn each_sheet_keeps_its_own_colour_under_its_unescaped_name() {
        let workbook = r#"<workbook xmlns:r="r"><sheets>
            <sheet name="Plain" r:id="rId1"/>
            <sheet name="R&amp;D" r:id="rId2"/></sheets></workbook>"#;
        let rels = r#"<Relationships>
            <Relationship Id="rId1" Target="worksheets/sheet1.xml"/>
            <Relationship Id="rId2" Target="worksheets/sheet2.xml"/></Relationships>"#;
        let f = zip_xlsx(&[
            ("xl/workbook.xml", workbook),
            ("xl/_rels/workbook.xml.rels", rels),
            ("xl/styles.xml", STYLES),
            ("xl/worksheets/sheet1.xml", &sheet(&plain_rows(&[]))),
            (
                "xl/worksheets/sheet2.xml",
                &sheet(&plain_rows(&[("B3", 1)])),
            ),
        ]);
        let facts = xlsx_meaningful(f.path());
        assert!(!facts["Plain"].meaningful(START, END));
        assert!(facts["R&D"].meaningful(START, END));
    }

    #[test]
    fn a_part_that_breaks_after_a_listed_entry_gives_no_facts() {
        // The partial list read before the error must not be kept.
        let workbook = r#"<workbook xmlns:r="r"><sheets>
            <sheet name="S" r:id="rId1"/></workbook>"#;
        let rels = r#"<Relationships>
            <Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Other>"#;
        for (workbook, rels) in [(workbook, RELS), (WORKBOOK, rels)] {
            let f = zip_xlsx(&[
                ("xl/workbook.xml", workbook),
                ("xl/_rels/workbook.xml.rels", rels),
                ("xl/styles.xml", STYLES),
                (
                    "xl/worksheets/sheet1.xml",
                    &sheet(&plain_rows(&[("B3", 1)])),
                ),
            ]);
            assert!(xlsx_meaningful(f.path()).is_empty());
        }
    }

    #[test]
    fn missing_relationships_part_gives_no_facts() {
        let f = zip_xlsx(&[
            ("xl/workbook.xml", WORKBOOK),
            ("xl/styles.xml", STYLES),
            (
                "xl/worksheets/sheet1.xml",
                &sheet(&plain_rows(&[("B3", 1)])),
            ),
        ]);
        assert!(xlsx_meaningful(f.path()).is_empty());
    }

    #[test]
    fn a_sheet_without_a_relationship_does_not_stop_the_others() {
        let workbook = r#"<workbook xmlns:r="r"><sheets>
            <sheet name="Lost" r:id="rId9"/>
            <sheet name="S" r:id="rId1"/></sheets></workbook>"#;
        let f = zip_xlsx(&[
            ("xl/workbook.xml", workbook),
            ("xl/_rels/workbook.xml.rels", RELS),
            ("xl/styles.xml", STYLES),
            (
                "xl/worksheets/sheet1.xml",
                &sheet(&plain_rows(&[("B3", 1)])),
            ),
        ]);
        let mut facts = xlsx_meaningful(f.path());
        assert!(!facts.contains_key("Lost"));
        assert!(facts.remove("S").unwrap().meaningful(START, END));
    }
}

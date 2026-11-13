//! Writes a table as Markdown, CSV, a link stub or XLSX.

use crate::table::Grid;
use anyhow::Result;
use std::path::{Component, Path, PathBuf};

/// Escapes a cell for a pipe table: Markdown punctuation that would change
/// how the text renders gets a backslash, and a line break becomes `<br>`.
fn md_cell(cell: &str) -> String {
    let mut out = String::new();
    for c in cell.trim().replace("\r\n", "\n").chars() {
        match c {
            '\n' => out.push_str("<br>"),
            '\\' | '|' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '&' | '~' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Renders `grid` as a GFM pipe table. The first row is the header.
pub fn markdown(grid: &Grid) -> String {
    let mut out = String::new();
    for (i, row) in grid.rows.iter().enumerate() {
        let cells: Vec<String> = row.iter().map(|c| md_cell(c)).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
        if i == 0 {
            out.push_str(&format!("|{}\n", "---|".repeat(row.len())));
        }
    }
    out
}

/// Renders `grid` as CSV (RFC 4180, `\n` line endings).
pub fn csv(grid: &Grid) -> Result<String> {
    let mut w = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    for row in &grid.rows {
        w.write_record(row)?;
    }
    Ok(String::from_utf8(w.into_inner()?)?)
}

/// Renders a one-line Markdown link to the source file, with a summary.
/// `out` is the path the stub is written to; the link is relative to it.
pub fn link_stub(grid: &Grid, source: &Path, out: &Path) -> Result<String> {
    let target = relative_link(source, out)?;
    let label = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| grid.file.clone());
    let escape = |s: &str| s.replace('[', "\\[").replace(']', "\\]");
    let label = escape(&label);
    let sheet = escape(&grid.name);
    let target = target.replace('%', "%25").replace('#', "%23");
    let target = if target.contains([' ', '(', ')', '<', '>']) {
        format!("<{}>", target.replace('<', "%3C").replace('>', "%3E"))
    } else {
        target
    };
    Ok(format!(
        "[{label} — {sheet}, {} rows × {} columns]({target})\n",
        grid.data_rows(),
        grid.columns()
    ))
}

fn absolute(p: &Path) -> Result<PathBuf> {
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()?.join(p)
    };
    // Resolve `.` and `..` without touching the file system.
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    Ok(out)
}

/// Path from the directory of `out` to `source`, with `/` separators.
fn relative_link(source: &Path, out: &Path) -> Result<String> {
    let source = absolute(source)?;
    let out = absolute(out)?;
    let from: Vec<_> = out
        .parent()
        .map(|d| d.components().collect())
        .unwrap_or_default();
    let to: Vec<_> = source.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); from.len() - common];
    parts.extend(
        to[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Ok(parts.join("/"))
}

/// Returns the number in `cell` if writing it as a number and reading it
/// back gives the same text (so `007` and `1.50` stay text).
fn number(cell: &str) -> Option<f64> {
    let n: f64 = cell.parse().ok()?;
    let back = if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    };
    (n.is_finite() && back == cell).then_some(n)
}

/// The name of worksheet `i` of `n`: `Sheet1` when there is one, otherwise
/// `Table1`, `Table2`, ...
pub fn sheet_name(i: usize, n: usize) -> String {
    if n > 1 {
        format!("Table{}", i + 1)
    } else {
        "Sheet1".into()
    }
}

/// Writes each grid as one worksheet of an XLSX file. Output is
/// deterministic: the document creation time is fixed.
pub fn xlsx(grids: &[Grid], out: &Path) -> Result<()> {
    use rust_xlsxwriter::{DocProperties, ExcelDateTime, Workbook};
    let mut wb = Workbook::new();
    let created = ExcelDateTime::from_ymd(2000, 1, 1)?;
    wb.set_properties(&DocProperties::new().set_creation_datetime(&created));
    for (i, g) in grids.iter().enumerate() {
        let ws = wb.add_worksheet();
        if grids.len() > 1 {
            ws.set_name(sheet_name(i, grids.len()))?;
        }
        for (r, row) in g.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                match number(cell) {
                    Some(n) => {
                        ws.write_number(r as u32, c as u16, n)?;
                    }
                    None if !cell.is_empty() => {
                        ws.write_string(r as u32, c as u16, cell)?;
                    }
                    None => {}
                }
            }
        }
    }
    wb.save(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: &[&[&str]]) -> Grid {
        Grid {
            file: "a.xlsx".into(),
            name: "Sheet1".into(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
            ..Grid::default()
        }
    }

    #[test]
    fn markdown_escapes_pipes_and_line_breaks() {
        let g = grid(&[&["a", "b"], &["x|y", "1\n2"]]);
        assert_eq!(markdown(&g), "| a | b |\n|---|---|\n| x\\|y | 1<br>2 |\n");
    }

    #[test]
    fn markdown_escapes_punctuation_that_changes_rendering() {
        let g = grid(&[&["a"], &["<v> *r* _u_ `c` [l] a\\b &amp; ~s~"]]);
        assert_eq!(
            markdown(&g),
            "| a |\n|---|\n| \\<v\\> \\*r\\* \\_u\\_ \\`c\\` \\[l\\] a\\\\b \\&amp; \\~s\\~ |\n"
        );
    }

    #[test]
    fn csv_quotes_when_needed() {
        let g = grid(&[&["a", "b"], &["x,y", "z"]]);
        assert_eq!(csv(&g).unwrap(), "a,b\n\"x,y\",z\n");
    }

    #[test]
    fn only_canonical_numbers_are_numbers() {
        assert_eq!(number("12"), Some(12.0));
        assert_eq!(number("-1.5"), Some(-1.5));
        assert_eq!(number("007"), None);
        assert_eq!(number("1.50"), None);
        assert_eq!(number("NaN"), None);
        assert_eq!(number(""), None);
    }

    #[test]
    fn link_with_spaces_is_wrapped_in_angle_brackets() {
        let g = grid(&[&["a"], &["1"]]);
        let s = link_stub(&g, Path::new("/r/Plan (v2).xlsx"), Path::new("/r/t.md")).unwrap();
        assert_eq!(
            s,
            "[Plan (v2).xlsx — Sheet1, 1 rows × 1 columns](<Plan (v2).xlsx>)\n"
        );
    }

    #[test]
    fn link_encodes_hash_and_percent_and_escapes_the_sheet_name() {
        let mut g = grid(&[&["a"], &["1"]]);
        g.name = "A]".into();
        let s = link_stub(&g, Path::new("/r/req#1 50%.xlsx"), Path::new("/r/t.md")).unwrap();
        assert_eq!(
            s,
            "[req#1 50%.xlsx — A\\], 1 rows × 1 columns](<req%231 50%25.xlsx>)\n"
        );
    }

    #[test]
    fn link_escapes_an_opening_bracket_in_the_sheet_name() {
        let mut g = grid(&[&["a"], &["1"]]);
        g.name = "[A".into();
        let s = link_stub(&g, Path::new("/r/a.xlsx"), Path::new("/r/t.md")).unwrap();
        assert_eq!(s, "[a.xlsx — \\[A, 1 rows × 1 columns](a.xlsx)\n");
    }

    #[test]
    fn link_is_relative_to_the_output_file() {
        let g = grid(&[&["a"], &["1"], &["2"]]);
        let s = link_stub(
            &g,
            Path::new("/r/att/a.xlsx"),
            Path::new("/r/docs/spec/t.md"),
        )
        .unwrap();
        assert_eq!(
            s,
            "[a.xlsx — Sheet1, 2 rows × 1 columns](../../att/a.xlsx)\n"
        );
    }
}

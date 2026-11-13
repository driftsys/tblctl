//! Writes a table as Markdown, CSV, a link stub or XLSX.

use crate::table::Grid;
use anyhow::{Context, Result};
use std::path::{Component, Path, PathBuf};

fn md_cell(cell: &str) -> String {
    cell.trim()
        .replace('|', "\\|")
        .replace("\r\n", "\n")
        .replace('\n', "<br>")
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
    Ok(format!(
        "[{label} — {}, {} rows × {} columns]({target})\n",
        grid.name,
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
            ws.set_name(format!("Table{}", i + 1))?;
        }
        for (r, row) in g.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if !cell.is_empty() {
                    ws.write_string(r as u32, c as u16, cell)?;
                }
            }
        }
    }
    wb.save(out).with_context(|| out.display().to_string())?;
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
    fn csv_quotes_when_needed() {
        let g = grid(&[&["a", "b"], &["x,y", "z"]]);
        assert_eq!(csv(&g).unwrap(), "a,b\n\"x,y\",z\n");
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

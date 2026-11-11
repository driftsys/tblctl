//! Finds GitHub-flavoured Markdown pipe tables in a document.

/// A pipe table found in a Markdown document.
#[derive(Debug, PartialEq)]
pub struct Table {
    /// 1-based line number of the header row.
    pub line: usize,
    /// Header cells.
    pub header: Vec<String>,
    /// Data rows, without the header row and the delimiter row.
    pub rows: Vec<Vec<String>>,
}

/// Returns every pipe table in `text`, in document order. Tables inside
/// fenced code blocks are skipped.
pub fn tables(text: &str) -> Vec<Table> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            i += 1;
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```");
            i += 1;
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~");
            i += 1;
            continue;
        }
        if i + 1 < lines.len() && lines[i].contains('|') && is_delimiter(lines[i + 1]) {
            let header = cells(lines[i]);
            let mut rows = Vec::new();
            let mut j = i + 2;
            while j < lines.len() && !lines[j].trim().is_empty() && lines[j].contains('|') {
                rows.push(cells(lines[j]));
                j += 1;
            }
            out.push(Table {
                line: i + 1,
                header,
                rows,
            });
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

fn is_delimiter(line: &str) -> bool {
    let line = line.trim();
    if !line.contains('-') {
        return false;
    }
    let inner = line.strip_prefix('|').unwrap_or(line);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').all(|cell| {
        let cell = cell.trim();
        let cell = cell.strip_prefix(':').unwrap_or(cell);
        let cell = cell.strip_suffix(':').unwrap_or(cell);
        !cell.is_empty() && cell.chars().all(|c| c == '-')
    })
}

/// Splits a table row into trimmed cells. `\|` is an escaped pipe.
fn cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = match line.strip_suffix('|') {
        Some(rest) if !rest.ends_with('\\') => rest,
        _ => line,
    };
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cell).trim().to_string()),
            _ => cell.push(c),
        }
    }
    out.push(cell.trim().to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_table_with_line_header_and_rows() {
        let t = tables("intro\n\n| a | b |\n|:--|--:|\n| 1 | 2 |\n\nafter | x\n");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].line, 3);
        assert_eq!(t[0].header, ["a", "b"]);
        assert_eq!(t[0].rows, [["1", "2"]]);
    }

    #[test]
    fn accepts_tables_without_outer_pipes() {
        let t = tables("a | b\n--- | ---\n1 | 2\n");
        assert_eq!(t[0].header, ["a", "b"]);
        assert_eq!(t[0].rows, [["1", "2"]]);
    }

    #[test]
    fn escaped_pipe_stays_in_the_cell() {
        let t = tables("| a |\n|---|\n| x \\| y |\n");
        assert_eq!(t[0].rows, [["x | y"]]);
    }

    #[test]
    fn skips_fenced_code() {
        assert!(tables("~~~\n| a |\n|---|\n~~~\n").is_empty());
    }

    #[test]
    fn line_without_delimiter_is_not_a_table() {
        assert!(tables("| a |\n| b |\n").is_empty());
    }
}

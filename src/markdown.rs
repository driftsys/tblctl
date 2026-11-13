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
/// fenced or indented code blocks are skipped.
///
/// As in GFM, the header and delimiter rows must have the same number of
/// cells, body rows are cut or padded to that number, and the table ends at
/// a blank line or at the start of another block.
pub fn tables(text: &str) -> Vec<Table> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    // Open fence: marker character and length.
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        if let Some((ch, len)) = fence {
            if let Some((c, n, rest)) = fence_marker(lines[i]) {
                if c == ch && n >= len && rest.trim().is_empty() {
                    fence = None;
                }
            }
            i += 1;
            continue;
        }
        if let Some((c, n, _)) = fence_marker(lines[i]) {
            fence = Some((c, n));
            i += 1;
            continue;
        }
        if i + 1 < lines.len()
            && indent(lines[i]) < 4
            && lines[i].contains('|')
            && is_delimiter(lines[i + 1])
            && cells(lines[i]).len() == cells(lines[i + 1]).len()
        {
            let header = cells(lines[i]);
            let width = header.len();
            let mut rows = Vec::new();
            let mut j = i + 2;
            while j < lines.len() && !ends_table(lines[j]) {
                let mut row = cells(lines[j]);
                row.resize(width, String::new());
                rows.push(row);
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

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Returns the fence character, the fence length and the text after the
/// fence, if `line` opens or closes a fenced code block.
fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    if indent(line) >= 4 {
        return None;
    }
    let t = line.trim_start();
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|x| *x == c).count();
    (n >= 3).then(|| (c, n, &t[n..]))
}

/// True if `line` ends a table body: a blank line or the start of another
/// block (block quote, heading, fence).
fn ends_table(line: &str) -> bool {
    let t = line.trim_start();
    t.is_empty() || t.starts_with('>') || t.starts_with('#') || fence_marker(line).is_some()
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
    fn header_and_delimiter_cell_counts_must_match() {
        assert!(tables("---\ntitle: a | b\n---\n").is_empty());
        assert!(tables("Use x | y\n---\n").is_empty());
    }

    #[test]
    fn shorter_inner_fence_does_not_close_outer_fence() {
        assert!(tables("````md\n```\n| a |\n|---|\n```\n````\n").is_empty());
    }

    #[test]
    fn skips_indented_code() {
        assert!(tables("    | a |\n    |---|\n    | 1 |\n").is_empty());
    }

    #[test]
    fn body_rows_take_the_header_width() {
        let t = tables("| a | b |\n|---|---|\n| 1 | 2 | 3 | 4 |\n| 5 |\n");
        assert_eq!(t[0].rows, [["1", "2"], ["5", ""]]);
    }

    #[test]
    fn row_without_pipe_continues_the_table_until_another_block() {
        let t = tables("a | b\n--- | ---\nfoo\n> quote | x\n");
        assert_eq!(t[0].rows, [["foo", ""]]);
    }

    #[test]
    fn line_without_delimiter_is_not_a_table() {
        assert!(tables("| a |\n| b |\n").is_empty());
    }
}

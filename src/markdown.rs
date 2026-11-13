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
/// fenced or indented code blocks or HTML comments are skipped. Tables inside
/// block quotes are found; line numbers are those of the file.
///
/// As in GFM, the header and delimiter rows must have the same number of
/// cells, body rows are cut or padded to that number, and the table ends at
/// a blank line, at a change of block-quote depth or at the start of another
/// block.
pub fn tables(text: &str) -> Vec<Table> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<(usize, &str)> = text.lines().map(unquote).collect();
    let mut out = Vec::new();
    // Open fence: marker character and length.
    let mut fence: Option<(char, usize)> = None;
    let mut comment = false;
    let mut i = 0;
    while i < lines.len() {
        let (depth, line) = lines[i];
        if comment {
            comment = !line.contains("-->");
            i += 1;
            continue;
        }
        if let Some((ch, len)) = fence {
            if let Some((c, n, rest)) = fence_marker(line) {
                if c == ch && n >= len && rest.trim().is_empty() {
                    fence = None;
                }
            }
            i += 1;
            continue;
        }
        if let Some((c, n, _)) = fence_marker(line) {
            fence = Some((c, n));
            i += 1;
            continue;
        }
        if indent(line) < 4 {
            if let Some(rest) = line.trim_start().strip_prefix("<!--") {
                comment = !rest.contains("-->");
                i += 1;
                continue;
            }
        }
        if i + 1 < lines.len()
            && lines[i + 1].0 == depth
            && indent(line) < 4
            && line.contains('|')
            && is_delimiter(lines[i + 1].1)
            && cells(line).len() == cells(lines[i + 1].1).len()
        {
            let header = cells(line);
            let width = header.len();
            let mut rows = Vec::new();
            let mut j = i + 2;
            while j < lines.len() && lines[j].0 == depth && !ends_table(lines[j].1) {
                let mut row = cells(lines[j].1);
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

/// Returns the block-quote depth of `line` and the line without its `>`
/// markers.
fn unquote(line: &str) -> (usize, &str) {
    let mut depth = 0;
    let mut rest = line;
    while indent(rest) < 4 {
        match rest.trim_start().strip_prefix('>') {
            Some(after) => {
                depth += 1;
                rest = after.strip_prefix(' ').unwrap_or(after);
            }
            None => break,
        }
    }
    (depth, rest)
}

/// Number of characters in a cell's Markdown source, with each link or
/// image `[text](url)` counted as `text` only. Text inside a code span and
/// a backslash escape are counted as written.
pub fn visible_len(cell: &str) -> usize {
    let mut n = 0;
    let mut rest = cell;
    while let Some(c) = rest.chars().next() {
        if c == '\\' {
            let len = 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
            n += rest[..len].chars().count();
            rest = &rest[len..];
            continue;
        }
        if c == '`' {
            let run = rest.chars().take_while(|x| *x == '`').count();
            let len = code_span_end(rest, run).unwrap_or(run);
            n += rest[..len].chars().count();
            rest = &rest[len..];
            continue;
        }
        let bang = usize::from(c == '!' && rest[1..].starts_with('['));
        if c == '[' || bang == 1 {
            let after = &rest[bang + 1..];
            let link = after.find("](").and_then(|close| {
                url_end(&after[close + 2..]).map(|end| (close, bang + 1 + close + 2 + end + 1))
            });
            if let Some((close, skip)) = link {
                n += after[..close].chars().count();
                rest = &rest[skip..];
                continue;
            }
        }
        n += 1;
        rest = &rest[c.len_utf8()..];
    }
    n
}

/// Byte length of the code span that opens `s` with `run` backticks, if a
/// closing run of the same length follows.
fn code_span_end(s: &str, run: usize) -> Option<usize> {
    let mut i = run;
    while let Some(off) = s[i..].find('`') {
        let start = i + off;
        let len = s[start..].chars().take_while(|x| *x == '`').count();
        if len == run {
            return Some(start + len);
        }
        i = start + len;
    }
    None
}

/// Index of the `)` that closes a link destination, allowing balanced
/// parentheses inside the URL.
fn url_end(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Indentation in columns; a tab advances to the next multiple of 4.
fn indent(line: &str) -> usize {
    let mut col = 0;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += 4 - col % 4,
            _ => break,
        }
    }
    col
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
/// block (heading, fence, HTML block, list item, thematic break).
fn ends_table(line: &str) -> bool {
    let t = line.trim_start();
    t.is_empty()
        || t.starts_with('#')
        || t.starts_with('<')
        || fence_marker(line).is_some()
        || is_list_item(t)
        || is_thematic_break(t)
}

fn is_list_item(t: &str) -> bool {
    let marker = if t.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        if !(1..=9).contains(&digits) || !t[digits..].starts_with(['.', ')']) {
            return false;
        }
        digits + 1
    };
    t[marker..].is_empty() || t[marker..].starts_with([' ', '\t'])
}

fn is_thematic_break(t: &str) -> bool {
    let marks: Vec<char> = t.chars().filter(|c| *c != ' ' && *c != '\t').collect();
    marks.len() >= 3 && ['*', '-', '_'].contains(&marks[0]) && marks.iter().all(|c| *c == marks[0])
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
    fn visible_len_drops_link_urls() {
        assert_eq!(visible_len("plain"), 5);
        assert_eq!(visible_len("[spec](https://example.com/a/b/c)"), 4);
        assert_eq!(visible_len("see ![logo](x.png) and [a](b)"), 14);
        assert_eq!(visible_len("[not a link"), 11);
        assert_eq!(visible_len("Done!! [x](y)"), 8);
        assert_eq!(visible_len("[w](https://e.org/A_(b))"), 1);
    }

    #[test]
    fn list_item_thematic_break_or_html_ends_the_table() {
        for next in [
            "- item", "* item", "+ item", "1. item", "2) item", "***", "---", "___", "<div>",
        ] {
            let t = tables(&format!("| a |\n|---|\n| 1 |\n{next}\n"));
            assert_eq!(t[0].rows, [["1"]], "{next}");
        }
    }

    #[test]
    fn leading_bom_is_ignored() {
        let t = tables("\u{feff}| a |\n|---|\n| 1 |\n");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].header, ["a"]);
    }

    #[test]
    fn finds_tables_in_block_quotes() {
        let t = tables("text\n> | a | b |\n> |---|---|\n> | 1 | 2 |\n\n> > | c |\n> > |---|\n> > | 3 |\n> > x\n");
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].line, 2);
        assert_eq!(t[0].header, ["a", "b"]);
        assert_eq!(t[0].rows, [["1", "2"]]);
        assert_eq!(t[1].line, 6);
        assert_eq!(t[1].rows, [vec!["3"], vec!["x"]]);
    }

    #[test]
    fn tab_indented_table_is_code() {
        assert!(tables("\t| a |\n\t|---|\n\t| 1 |\n").is_empty());
        assert!(tables("  \t| a |\n  \t|---|\n").is_empty());
    }

    #[test]
    fn skips_html_comments() {
        assert!(tables("<!--\n| a |\n|---|\n| 1 |\n-->\n").is_empty());
        assert!(tables("<!-- | a | -->\n").is_empty());
        let t = tables("<!-- x -->\n| a |\n|---|\n");
        assert_eq!(t[0].line, 2);
    }

    #[test]
    fn visible_len_counts_code_spans_and_escaped_brackets_literally() {
        assert_eq!(visible_len("`[a](bcdef)`"), 12);
        assert_eq!(visible_len("``x`[a](b)``"), 12);
        assert_eq!(visible_len("\\[a](bcdef)"), 11);
        assert_eq!(visible_len("`open [a](b)"), 7);
    }

    #[test]
    fn line_without_delimiter_is_not_a_table() {
        assert!(tables("| a |\n| b |\n").is_empty());
    }
}

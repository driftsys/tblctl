//! Writes command output to stdout: text lines, or the JSON envelope of
//! AD-0005.

use crate::error::Error;
use anstyle::{AnsiColor, Style};
use serde_json::{json, Value};
use std::io::Write;

/// "1 table" or "N tables".
pub fn tables(n: usize) -> String {
    if n == 1 {
        "1 table".to_string()
    } else {
        format!("{n} tables")
    }
}

/// Writes `text` to stdout. ANSI styles in `text` are kept only when stdout
/// is a terminal or `CLICOLOR_FORCE` is set; `NO_COLOR` removes them in all
/// cases. A closed stdout (for example `| head`) is not an error: the exit
/// code still reports the result.
pub fn emit(text: &str) -> anyhow::Result<()> {
    let mut out = anstream::AutoStream::auto(std::io::stdout().lock());
    match out.write_all(text.as_bytes()) {
        Err(err) if err.kind() != std::io::ErrorKind::BrokenPipe => Err(err.into()),
        _ => Ok(()),
    }
}

/// Writes `line` to stderr. A write error (for example a closed stderr) is
/// ignored, so that the envelope is still written and the exit code still
/// reports the result.
fn stderr_line(line: &str) {
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// The styles used in text output.
pub const GREEN: Style = AnsiColor::Green.on_default();
pub const RED: Style = AnsiColor::Red.on_default();
pub const YELLOW: Style = AnsiColor::Yellow.on_default();
pub const DIM: Style = Style::new().dimmed();

/// Returns `word` in `style`, left-aligned to `width` characters. The padding
/// is outside the style, so the alignment is the same with and without colour.
pub fn paint(word: &str, style: Style, width: usize) -> String {
    let pad = width.saturating_sub(word.chars().count());
    format!("{style}{word}{style:#}{}", " ".repeat(pad))
}

/// Writes the output of `command` and returns the exit code: 2 if there is
/// any error, otherwise 1 if `failed`, otherwise 0.
///
/// With `json`, stdout gets one envelope object with `results`, `summary` and
/// `errors`; otherwise stdout gets `text`. In both modes each error is also
/// written to stderr as one `tblctl: <message>` line.
pub fn finish(
    command: &str,
    json: bool,
    results: Value,
    summary: Value,
    errors: &[Error],
    text: &str,
    failed: bool,
) -> i32 {
    for err in errors {
        stderr_line(&format!("tblctl: {err}"));
    }
    let body = if json {
        let envelope = json!({
            "schema": format!("tblctl/{command}/v1"),
            "results": results,
            "summary": summary,
            "errors": errors.iter().map(Error::to_json).collect::<Vec<_>>(),
        });
        serde_json::to_string_pretty(&envelope).expect("a JSON value always serialises") + "\n"
    } else {
        text.to_string()
    };
    if let Err(err) = emit(&body) {
        stderr_line(&format!("tblctl: {err:#}"));
        return 2;
    }
    if !errors.is_empty() {
        2
    } else if failed {
        1
    } else {
        0
    }
}

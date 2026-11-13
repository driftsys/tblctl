//! Writes command output to stdout.

use anstyle::{AnsiColor, Style};
use std::io::Write;

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

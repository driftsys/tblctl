//! Writes command output to stdout.

use std::io::Write;

/// Writes `text` to stdout. A closed stdout (for example `| head`) is not an
/// error: the exit code still reports the result.
pub fn emit(text: &str) -> anyhow::Result<()> {
    match std::io::stdout().lock().write_all(text.as_bytes()) {
        Err(err) if err.kind() != std::io::ErrorKind::BrokenPipe => Err(err.into()),
        _ => Ok(()),
    }
}

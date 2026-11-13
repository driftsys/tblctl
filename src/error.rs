//! Operational errors. Each error has a stable snake_case code for the JSON
//! envelope (AD-0005), so a consumer never parses the message text.

use serde_json::{json, Map, Value};

#[derive(Debug)]
pub enum Error {
    /// A file could not be opened or parsed. `message` names the file.
    ReadFailed { file: String, message: String },
    /// The file extension is not one the command reads. `message` names the
    /// file.
    UnsupportedType { file: String, message: String },
    /// The file holds no table.
    NoTable { file: String },
    /// The file holds several tables and `--sheet` was not given.
    SeveralTables { file: String, tables: Vec<String> },
    /// No table has the name given to `--sheet`.
    NoSuchTable {
        file: String,
        name: String,
        tables: Vec<String>,
    },
    /// The `--out` extension does not match the format to write.
    WrongExtension {
        file: String,
        expected: &'static str,
        message: String,
    },
    /// `--out` is the input file.
    SameFile { file: String },
    /// The output file could not be written. `message` names the file.
    WriteFailed { file: String, message: String },
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::ReadFailed { .. } => "read_failed",
            Error::UnsupportedType { .. } => "unsupported_type",
            Error::NoTable { .. } => "no_table",
            Error::SeveralTables { .. } => "several_tables",
            Error::NoSuchTable { .. } => "no_such_table",
            Error::WrongExtension { .. } => "wrong_extension",
            Error::SameFile { .. } => "same_file",
            Error::WriteFailed { .. } => "write_failed",
        }
    }

    fn file(&self) -> &str {
        match self {
            Error::ReadFailed { file, .. }
            | Error::UnsupportedType { file, .. }
            | Error::NoTable { file }
            | Error::SeveralTables { file, .. }
            | Error::NoSuchTable { file, .. }
            | Error::WrongExtension { file, .. }
            | Error::SameFile { file }
            | Error::WriteFailed { file, .. } => file,
        }
    }

    pub fn hint(&self) -> Option<String> {
        match self {
            Error::SeveralTables { .. } | Error::NoSuchTable { .. } => {
                Some("choose one with --sheet".into())
            }
            Error::WrongExtension { expected, .. } => {
                Some(format!("use an --out path ending in .{expected}"))
            }
            _ => None,
        }
    }

    /// The error as one JSON object: `code`, `message`, `file`, `hint` when
    /// there is one, and the fields specific to the code.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("code".into(), json!(self.code()));
        m.insert("message".into(), json!(self.to_string()));
        m.insert("file".into(), json!(self.file()));
        if let Some(hint) = self.hint() {
            m.insert("hint".into(), json!(hint));
        }
        match self {
            Error::SeveralTables { tables, .. } | Error::NoSuchTable { tables, .. } => {
                m.insert("tables".into(), json!(tables));
            }
            Error::WrongExtension { expected, .. } => {
                m.insert("expected".into(), json!(expected));
            }
            _ => {}
        }
        Value::Object(m)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ReadFailed { message, .. }
            | Error::UnsupportedType { message, .. }
            | Error::WrongExtension { message, .. }
            | Error::WriteFailed { message, .. } => f.write_str(message),
            Error::NoTable { file } => write!(f, "{file}: no table found"),
            Error::SeveralTables { file, tables } => write!(
                f,
                "{file}: {} tables found; choose one with --sheet ({})",
                tables.len(),
                tables.join(", ")
            ),
            Error::NoSuchTable { file, name, tables } => write!(
                f,
                "{file}: no table named {name:?} (tables: {})",
                tables.join(", ")
            ),
            Error::SameFile { file } => write!(f, "{file}: --out is the input file"),
        }
    }
}

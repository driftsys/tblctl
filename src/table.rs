//! The table model shared by every reader and rule.

/// One table read from a source file. The first row is the header.
#[derive(Debug, Clone, Default)]
pub struct Grid {
    /// Source file, as given on the command line.
    pub file: String,
    /// Table name inside the file: a sheet name, `line N` for Markdown,
    /// `table N` for DOCX, or `data` for CSV.
    pub name: String,
    /// All rows, header first. Every row has the same number of cells.
    pub rows: Vec<Vec<String>>,
    /// Facts the rules need that the cell text does not show.
    pub facts: Facts,
}

#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// A merged range touches a row after the header.
    pub data_merges: bool,
    /// At least one cell holds a formula.
    pub formulas: bool,
    /// The table or sheet holds an image.
    pub images: bool,
    /// Number of visible sheets in the source workbook (1 for other formats).
    pub visible_sheets: usize,
    /// Cell length is measured on Markdown source with link URLs removed.
    pub markdown: bool,
}

impl Grid {
    /// Number of data rows (header excluded).
    pub fn data_rows(&self) -> usize {
        self.rows.len().saturating_sub(1)
    }

    pub fn columns(&self) -> usize {
        self.rows.first().map_or(0, Vec::len)
    }

    /// Pads every row to the widest row and drops trailing empty rows.
    pub fn normalise(&mut self) {
        while self
            .rows
            .last()
            .is_some_and(|r| r.iter().all(|c| c.trim().is_empty()))
        {
            self.rows.pop();
        }
        let width = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        for row in &mut self.rows {
            row.resize(width, String::new());
        }
    }
}

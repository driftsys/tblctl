# Design: `classify`, `import` and `export`

As built. Code: `src/read.rs`, `src/rules.rs`, `src/write.rs`,
`src/commands.rs`.

## Table model

Every reader returns `Grid` values (`src/table.rs`): rows of text with the
header first, padded to one width, and the facts the rules need (merged data
cells, formulas, images, number of visible sheets).

## Readers

| Input                      | Library                                    | One table per                             |
| -------------------------- | ------------------------------------------ | ----------------------------------------- |
| XLSX, XLSM, XLS, XLSB, ODS | `calamine`                                 | visible worksheet                         |
| DOCX                       | `zip` + `quick-xml` on `word/document.xml` | top-level `w:tbl`                         |
| CSV                        | `csv`                                      | file                                      |
| Markdown                   | `src/markdown.rs`                          | pipe table outside code and HTML comments |

- Hidden sheets, chart sheets, dialog sheets, macro sheets and VBA sheets are
  skipped.
- A merged range is flattened by repeating its top-left value. A merge that
  reaches below the header row is a merged data cell. A merge wholly outside the
  used range (formatting only) is ignored; a merge partly outside it is clipped
  to it, so a merge never widens the table. Merges are read for XLSX and XLS
  through `calamine`, and for ODS from `content.xml`
  (`table:number-rows-spanned`, `table:number-columns-spanned`). The XLSB reader
  does not expose merges, so XLSB merges are not detected.
- Images are detected for XLSX only. `calamine` does not expose images in XLS,
  XLSB or ODS files, so a table with images in those formats is not marked.
- Dates are written as `YYYY-MM-DD`, or `YYYY-MM-DD HH:MM:SS` when the time is
  not midnight, for XLSX, XLS and XLSB dates and ODS ISO dates alike. Whole
  numbers are written without a decimal point.
- In DOCX, `w:gridSpan` repeats a cell across columns and `w:vMerge` repeats the
  cell above; a vertical merge, or a horizontal one after the header row, is a
  merged data cell. Paragraphs in a cell are joined with line breaks; `w:br`
  becomes a line break and `w:tab` a tab. `w:drawing` or `w:pict` in a table
  marks images.
- In Markdown, `<br>` (any case, with or without `/`) becomes a line break and a
  backslash before ASCII punctuation is removed (`\*` reads as `*`).

## Rules

First match wins (`src/rules.rs`):

1. **T0 skip** — every cell empty; the same content as an earlier table in the
   same run (SHA-256 of the cells); or a header where one cell has the word
   `date` and a different cell has the word `author`, `description` or
   `modification` (whole words, any case).
2. **T1 Markdown** — one visible sheet, no merged data cells, no images, and
   within `--max-rows`, `--max-cols` and `--max-cell`.
3. **T2 CSV** — one visible sheet, no merged data cells, no images, no formulas.
4. **T3 link** — everything else.

For T0, `reasons` says why the table is skipped (`empty`, `duplicate of …`,
`revision history`). For T2 and T3, it lists why the table did not get an
earlier rule.

Not detected: whether cell colour carries meaning. Issue #1 lists it for T1 and
T2; no reader extracts colour yet.

## Commands

- `classify <file…> [--json]` prints one line per table
  (`T2  csv      a.xlsx#Sheet1  40×2  rows 40 > 30`) or the JSON array of
  AD-0002. Exit 2 if a file cannot be read, otherwise 0.
- `import <file> [--sheet NAME] --out PATH` classifies the selected table and
  writes a Markdown table (T1), CSV (T2) or a one-line link stub (T3). `--out`
  must end in `.md` (T1, T3) or `.csv` (T2). T0 writes nothing and exits 0. In a
  Markdown table, a line break becomes `<br>` and the characters
  ``\ | * _ ` [ ] < > & ~`` get a backslash, so the text renders as written. The
  link destination is relative to the directory of `--out`, with `%` and `#`
  percent-encoded, and wrapped in `<>` when it contains spaces, parentheses, `<`
  or `>` (which are percent-encoded). `[` and `]` in the file and sheet names of
  the label get a backslash. `--sheet` is required when the input holds more
  than one table.
- `export <md|csv> --out x.xlsx` writes one worksheet per table, named `Table1`,
  `Table2`, … when there are several. `--out` must end in `.xlsx`. A cell is
  written as a number only when the number reads back as the same text, so `007`
  and `1.50` stay text. The creation time is fixed, so the same input gives a
  byte-identical file.
- `import` and `export` refuse an `--out` that is the input file.

## Not built

- Colour detection (see Rules).
- OCR of table pictures (out of scope for v0.1).
- Merges in XLSB files, and images in XLS, XLSB and ODS files (see Readers).
- A link stub for a source on another Windows drive than `--out`: the relative
  path is not valid.

# Design: `classify`, `import` and `export`

As built. Code: `src/read.rs`, `src/rules.rs`, `src/write.rs`,
`src/commands.rs`.

## Table model

Every reader returns `Grid` values (`src/table.rs`): rows of text with the
header first, padded to one width, and the facts the rules need (merged data
cells, formulas, images, number of visible sheets).

## Readers

| Input                      | Library                                    | One table per           |
| -------------------------- | ------------------------------------------ | ----------------------- |
| XLSX, XLSM, XLS, XLSB, ODS | `calamine`                                 | visible worksheet       |
| DOCX                       | `zip` + `quick-xml` on `word/document.xml` | top-level `w:tbl`       |
| CSV                        | `csv`                                      | file                    |
| Markdown                   | `src/markdown.rs`                          | pipe table outside code |

- Hidden sheets, chart sheets and dialog sheets are skipped.
- A merged range is flattened by repeating its top-left value. A merge that
  reaches below the header row is a merged data cell. Merges are read for XLSX
  and XLS; the XLSB and ODS readers do not expose them.
- Dates are written as `YYYY-MM-DD`, or `YYYY-MM-DD HH:MM:SS` when the time is
  not midnight. Whole numbers are written without a decimal point.
- In DOCX, `w:gridSpan` repeats a cell across columns and `w:vMerge` repeats the
  cell above; a vertical merge, or a horizontal one after the header row, is a
  merged data cell. Paragraphs in a cell are joined with line breaks.
  `w:drawing` or `w:pict` in a table marks images.
- In Markdown, `<br>` (any case, with or without `/`) becomes a line break.

## Rules

First match wins (`src/rules.rs`):

1. **T0 skip** — every cell empty; the same content as an earlier table in the
   same run (SHA-256 of the cells); or a header with a column containing `date`
   and one containing `author`, `description`, `modification` or `change`.
2. **T1 Markdown** — one visible sheet, no merged data cells, no images, and
   within `--max-rows`, `--max-cols` and `--max-cell`.
3. **T2 CSV** — one visible sheet, no merged data cells, no images, no formulas.
4. **T3 link** — everything else.

`reasons` lists why a table did not get an earlier rule.

Not detected: whether cell colour carries meaning. Issue #1 lists it for T1 and
T2; no reader extracts colour yet.

## Commands

- `classify <file…> [--json]` prints one line per table
  (`T2  csv      a.xlsx#Sheet1  40×2  rows 40 > 30`) or the JSON array of
  AD-0002. Exit 2 if a file cannot be read, otherwise 0.
- `import <file> [--sheet NAME] --out PATH` classifies the selected table and
  writes a Markdown table (T1), CSV (T2) or a one-line link stub (T3). `--out`
  must end in `.md` (T1, T3) or `.csv` (T2). T0 writes nothing and exits 0. The
  link destination is relative to the directory of `--out`, wrapped in `<>` when
  it contains spaces or parentheses. `--sheet` is required when the input holds
  more than one table.
- `export <md|csv> --out x.xlsx` writes one worksheet per table. A cell is
  written as a number only when the number reads back as the same text, so `007`
  and `1.50` stay text. The creation time is fixed, so the same input gives a
  byte-identical file.
- `import` and `export` refuse an `--out` that is the input file.

## Not built

- Colour detection (see Rules).
- OCR of table pictures (out of scope for v0.1).

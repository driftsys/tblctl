# Design: `classify`, `import` and `export`

As built. Code: `src/read.rs`, `src/rules.rs`, `src/write.rs`,
`src/commands.rs`, `src/error.rs`, `src/output.rs`. Output format:
[AD-0005](../decisions/0005-json-envelope-and-human-output.md).

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
  does not expose merges, so XLSB merges are not detected. Because merged data
  cells cannot be ruled out, every XLSB table gets the reason
  `merged cells
  unknown (XLSB)` and is T3.
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
earlier rule. With `import --as`, `reasons` also ends with
`target forced by --as`.

Not detected: whether cell colour carries meaning. Issue #1 lists it for T1 and
T2; no reader extracts colour yet.

## Commands

- `classify <file…> [--json]` prints one line per table
  (`T2  csv      a.xlsx#Sheet1  40×2  rows 40 > 30`) and a last line
  `N tables: a T0, b T1, c T2, d T3` (`1 table: …` for one; left out when no
  file can be read). The rule is dim (T0), green (T1, T2) or yellow (T3) when
  stdout is a terminal or `CLICOLOR_FORCE` is set, and never when `NO_COLOR` is
  set. With `--json` it prints the envelope of AD-0005 (schema
  `tblctl/classify/v1`, the objects of AD-0002 in `results`, summary
  `{tables, t0, t1, t2, t3}`). Exit 2 if a file cannot be read, otherwise 0.
- `import <file> [--sheet NAME] [--as md|csv|link] --out PATH [--json]`
  classifies the selected table and writes a Markdown table (T1), CSV (T2) or a
  one-line link stub (T3). `--as` writes the given format instead; the result
  keeps the computed rule and adds the reason `target forced by --as`. `--out`
  must end in `.md` (Markdown, link) or `.csv` (CSV). Without `--as`, T0 writes
  nothing and exits 0; with `--as`, a T0 table is written, except an empty
  table, which is never written. In a Markdown table, a line break becomes
  `<br>` and the characters ``\ | * _ ` [ ] < > & ~`` get a backslash, so the
  text renders as written. The link destination is relative to the directory of
  `--out`, with `%` and `#` percent-encoded, and wrapped in `<>` when it
  contains spaces, parentheses, `<` or `>` (which are percent-encoded). `[` and
  `]` in the file and sheet names of the label get a backslash. `--sheet` is
  required when the input holds more than one table.
- `export <md|csv> --out x.xlsx [--json]` writes one worksheet per table, named
  `Sheet1` when there is one table and `Table1`, `Table2`, … when there are
  several. `--out` must end in `.xlsx`. A cell is written as a number only when
  the number reads back as the same text, so `007` and `1.50` stay text. The
  creation time is fixed, so the same input gives a byte-identical file.
- `import` and `export` refuse an `--out` that is the input file.
- `import` and `export` write their status line
  (`T1  markdown a.xlsx#S  -> out.md`, `2 table(s) -> out.xlsx`) to stderr;
  stdout stays empty in text mode. With `--json`, no status line is written and
  stdout gets the envelope of AD-0005: for `import`, one result
  `{file, table, rule, target, out, reasons}` (`out` is `null` when nothing is
  written) and summary `{written}`; for `export`, one result
  `{table, sheet, rows, columns}` per worksheet and summary `{out, sheets}`
  (`out` is `null` on error).

## Errors

Operational errors are values of `Error` in `src/error.rs`, one variant per
code: `read_failed`, `unsupported_type`, `no_table`, `several_tables`,
`no_such_table`, `wrong_extension`, `same_file`, `write_failed`. The text mode
message is the `Display` form, written to stderr as `tblctl: <message>`. With
`--json`, `src/output.rs` also writes each error to the `errors` field of the
envelope. `classify` and `check` report each unreadable file and continue;
`import` and `export` stop at the first error.

## Not built

- Colour detection (see Rules).
- OCR of table pictures (out of scope for v0.1).
- Merges in XLSB files (XLSB tables are T3), and images in XLS, XLSB and ODS
  files (see Readers).
- A link stub for a source on another Windows drive than `--out`: the relative
  path is not valid.

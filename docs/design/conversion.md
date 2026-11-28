# Design: `classify`, `import` and `export`

As built. Code: `src/read.rs`, `src/rules.rs`, `src/write.rs`,
`src/commands.rs`, `src/error.rs`, `src/output.rs`. Output format:
[AD-0005](../decisions/0005-json-envelope-and-human-output.md).

## Table model

Every reader returns `Grid` values (`src/table.rs`): rows of text with the
header first, padded to one width, and the facts the rules need (merged data
cells, formulas, images, meaningful colour, number of visible sheets).

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
- Colour is read for XLSX and XLSM only, by `src/colour.rs` with `zip` +
  `quick-xml` from `xl/styles.xml`, the workbook relationships (to find each
  sheet part) and the sheet parts. The colour of a data cell is (fill, font) as
  written, without theme resolution. A row or cell without an `r` attribute
  takes the next position, numbered as `calamine` numbers it. A `<c>` element
  without `s` uses style 0. A cell with no `<c>` element uses the row `s` with
  `customFormat`, else the column style, else style 0. A cell with the colour of
  style 0 is uncoloured. The header is the first row of the used range. The rule
  is [AD-0006](../decisions/0006-meaningful-colour-rule.md). For XLS, XLSB, ODS
  and DOCX colour is not detected.
- Images are detected for XLSX only. `calamine` does not expose images in XLS,
  XLSB or ODS files, so a table with images in those formats is not marked. In
  XLSX, only the pictures that `calamine` reports (`pictures_with_metadata`)
  count; charts and shapes are not detected as images.
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
2. **T1 Markdown** — one visible sheet, no merged data cells, no images, no
   meaningful colour, and within `--max-rows`, `--max-cols` and `--max-cell`.
3. **T2 CSV** — one visible sheet, no merged data cells, no images, no
   meaningful colour, no formulas.
4. **T3 link** — everything else.

For T0, `reasons` says why the table is skipped (`empty`, `duplicate of …`,
`revision history`). For T2 and T3, it lists why the table did not get an
earlier rule. With `import --as`, `reasons` also ends with
`target forced by --as`.

Meaningful colour (XLSX and XLSM) is the T3 reason `colour`:

- 1 data row: any coloured data cell.
- 2 or 3 data rows: different colours within one column.
- 4 or more data rows: any colour that is not a function of (column, row
  parity). A highlighted total row counts.
- Any conditional format overlapping a data cell.

Conditional formats are read in both forms: the `sqref` attribute, and the
`<xm:sqref>` element of the Excel 2010 (`x14`) form in `extLst`. A sheet target
that holds `..` is used as written, by `calamine` and by the colour reader, so a
visible sheet with such a target normally cannot be read and the file is
reported as unreadable.

Limits of the colour reader: `gradientFill` and number-format colour (`[Red]`)
are not detected; colours equal but encoded differently (`rgb` versus `theme`)
compare as different; colour set on part of a cell's text (rich text) is not
detected, only the cell style's font colour is; a hyperlink cell style (blue
font) can give a false T3.

## Commands

- `classify <file…> [--json]` prints one line per table
  (`T2  csv      a.xlsx#Sheet1  40×2  rows 40 > 30`) and a last line
  `N tables: a T0, b T1, c T2, d T3` (`1 table: …` for one; left out when no
  file can be read). The rule is dim (T0), green (T1, T2) or yellow (T3) when
  stdout is a terminal or `CLICOLOR_FORCE` is set, and never when `NO_COLOR` is
  set. With `--json` it prints the envelope of AD-0005 (schema
  `tblctl/classify/v1`, the objects of AD-0002 in `results`, summary
  `{tables, t0, t1, t2, t3}`). Exit 2 if a file cannot be read, otherwise 0.
- `import <file> [--sheet NAME] [--as md|csv|link] [--compact] --out PATH [--json]`
  classifies the selected table and writes a Markdown table (T1), CSV (T2) or a
  one-line link stub (T3). `--as` writes the given format instead; the result
  keeps the computed rule and adds the reason `target forced by --as`. `--out`
  must end in `.md` (Markdown, link) or `.csv` (CSV). Without `--as`, T0 writes
  nothing and exits 0; with `--as`, a T0 table is written. An empty table
  ignores `--as`: nothing is written, `target` stays `skip`, `reasons` is
  `["empty"]` and the `--out` extension is not checked. In a Markdown table, a
  line break becomes `<br>` and the characters ``\ | * _ ` [ ] < > & ~`` get a
  backslash, so the text renders as written. Columns are aligned by default:
  each cell is padded to the widest cell of its column, measured in terminal
  columns (East Asian wide characters and emoji count 2, at least 1 per column),
  and the delimiter row has that many dashes. This is the layout `prim` writes,
  so running the formatter afterwards changes nothing. In both modes, a run of
  two or more spaces or tabs in a cell becomes one space (Markdown renders a run
  as one space), before the line break becomes `<br>`. `--compact` writes one
  space on each side of each cell and `---` delimiters instead. The link
  destination is relative to the directory of `--out`, with `%` and `#`
  percent-encoded, and wrapped in `<>` when it contains spaces, parentheses, `<`
  or `>` (which are percent-encoded). `[` and `]` in the file and sheet names of
  the label get a backslash. `--sheet` is required when the input holds more
  than one table.
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

- Colour detection for XLS, XLSB, ODS and DOCX (see Readers).
- OCR of table pictures (out of scope for v0.1).
- Merges in XLSB files (XLSB tables are T3), and images in XLS, XLSB and ODS
  files (see Readers).
- A link stub for a source on another Windows drive than `--out`: the relative
  path is not valid.

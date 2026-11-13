# Design: `tblctl check`

As-built description of the T1 size check. Decisions that bind it are in
[AD-0001](../decisions/0001-check-output-and-exit-codes.md),
[AD-0003](../decisions/0003-command-flags.md),
[AD-0004](../decisions/0004-thresholds-as-flags.md) and
[AD-0005](../decisions/0005-json-envelope-and-human-output.md).

## Components

- `src/markdown.rs` finds GitHub-flavoured Markdown pipe tables in a document
  and returns them in document order.
- `src/check.rs` evaluates each table against the limits and renders the result.
- `src/output.rs` (`finish`) writes the output and computes the exit code: 2 if
  any file cannot be read, otherwise 1 if any table failed, otherwise 0.
- `src/cli.rs` defines the arguments; `src/main.rs` exits with the code that
  `finish` returns.

## Table detection

- A leading UTF-8 byte order mark is ignored.
- Tables inside fenced or indented code blocks and inside HTML comments
  (`<!-- ... -->`, on one or more lines) are skipped. A tab counts as 4 columns
  of indentation, so a tab-indented table is indented code.
- Tables inside block quotes (`>`, nested too) are found; the reported line is
  the line in the file.
- The header and delimiter rows must have the same number of cells.
- Body rows are cut or padded to the header's cell count; the table ends at a
  blank line, at a change of block-quote depth, or at the start of another
  block: a heading, a fence, an HTML block (a line starting with `<`), a list
  item (`-`, `*`, `+`, `N.` or `N)`) or a thematic break (`***`, `---`, `___`).
- `\|` is an escaped pipe inside a cell.

## Measures

- Rows: data rows only. The header and the delimiter row are not counted.
- Columns: the number of header cells.
- Cell length: characters of the cell as rendered, header included, measured on
  the trimmed raw Markdown source. A backslash before ASCII punctuation, such as
  `\*`, `\[` or `\|`, counts as 1 character, and an escaped `\[` does not open a
  link: `\[a](bcdef)` counts 10. A link or image `[text](url)` counts as `text`
  only, and each `<br>` counts as one character (a line break). Text inside a
  code span counts as written, backslashes included, so a link inside it is not
  reduced to its text.

A table fails when rows, columns or the longest cell exceed `--max-rows`,
`--max-cols` or `--max-cell`. Each exceeded limit adds one reason, such as
`rows 45 > 30`.

## Output

Text: one line per table, as in AD-0001, with the detail `rows×columns` and, on
failure, the reasons followed by `suggest CSV`. A last line gives the totals:
`N tables: P passed, F failed` (`0 tables: 0 passed, 0 failed` when there is no
table, `1 table: …` for one). The summary line is left out when no file can be
read. `PASS` is green and `FAIL` red when stdout is a terminal or
`CLICOLOR_FORCE` is set, and never when `NO_COLOR` is set (`src/output.rs`).

`--json` prints the envelope of AD-0005 with schema `tblctl/check/v1`. `results`
has one object per table, in input order then document order. Fields: `file`,
`line`, `status` (`pass` or `fail`), `rule` (`t1`), `rows`, `columns`,
`reasons`. `summary` is `{tables, pass, fail}`. A file that cannot be read adds
an error with code `read_failed` to `errors`.

## Exit code

2 if any file cannot be read (the error goes to stderr, and with `--json` also
to `errors`; the other files are still checked); otherwise 1 if any table
failed; otherwise 0. A closed stdout is not an error.

## Not built

- No check reports `REVIEW`; `--strict` is accepted and has no effect.
- A table inside a list item, indented 4 or more spaces, is treated as indented
  code and is not checked.

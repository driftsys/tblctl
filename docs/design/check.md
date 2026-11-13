# Design: `tblctl check`

As-built description of the T1 size check. Decisions that bind it are in
[AD-0001](../decisions/0001-check-output-and-exit-codes.md),
[AD-0003](../decisions/0003-command-flags.md) and
[AD-0004](../decisions/0004-thresholds-as-flags.md).

## Components

- `src/markdown.rs` finds GitHub-flavoured Markdown pipe tables in a document
  and returns them in document order.
- `src/check.rs` evaluates each table against the limits, renders the result and
  computes the exit code.
- `src/cli.rs` defines the arguments; `src/main.rs` maps an error to exit 2.

## Table detection

- Tables inside fenced or indented code blocks are skipped.
- The header and delimiter rows must have the same number of cells.
- Body rows are cut or padded to the header's cell count; the table ends at a
  blank line or at the start of another block.
- `\|` is an escaped pipe inside a cell.

## Measures

- Rows: data rows only. The header and the delimiter row are not counted.
- Columns: the number of header cells.
- Cell length: characters of the trimmed Markdown source, header included.

A table fails when rows, columns or the longest cell exceed `--max-rows`,
`--max-cols` or `--max-cell`. Each exceeded limit adds one reason, such as
`rows 45 > 30`.

## Output

Text: one line per table, as in AD-0001, with the detail `rows×columns` and, on
failure, the reasons followed by `suggest CSV`.

`--json` prints a pretty-printed array with one object per table, in input order
then document order. Fields: `file`, `line`, `status` (`pass` or `fail`), `rule`
(`t1`), `rows`, `columns`, `reasons`.

## Exit code

2 if any file cannot be read (the error goes to stderr and the other files are
still checked); otherwise 1 if any table failed; otherwise 0. A closed stdout is
not an error.

## Not built

No check reports `REVIEW`; `--strict` is accepted and has no effect.

# tblctl

A hermetic, cross-platform CLI for tables in Markdown docs (GitLab, GitHub,
mdbook): it classifies tables from XLSX/XLS/XLSB/ODS/DOCX/CSV/Markdown sources,
imports them as a Markdown table, a diffable CSV or a link stub, checks Markdown
tables against size limits, and exports back to XLSX.

> **Status: early development.** All four commands work. Cell colour is not yet
> detected. Scope and rules are tracked in
> **[#1](https://github.com/driftsys/tblctl/issues/1)**.

Siblings: [`diagctl`](https://github.com/driftsys/diagctl) (SVG diagrams) and
[`pixctl`](https://github.com/driftsys/pixctl) (raster images).

Exit codes: `0` clean · `1` ≥1 check failed · `2` operational error (an
unreadable file, a wrong `--out`, an argument error).

## Install

From the latest GitHub release (macOS, Linux; checksum-verified):

```sh
curl -fsSL https://raw.githubusercontent.com/driftsys/tblctl/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/driftsys/tblctl/main/install.ps1 | iex
```

`TBLCTL_INSTALL_DIR` (default `~/.local/bin`) and `TBLCTL_VERSION` (default
latest) override the defaults.

Manual download:

```sh
gh release download -R driftsys/tblctl -p 'tblctl-<target>*'
```

From source:

```sh
cargo install tblctl
```

## Usage

Inputs: XLSX, XLSM, XLS, XLSB, ODS, DOCX, CSV and Markdown.

```sh
tblctl classify att/*.xlsx att/*.docx     # which rule applies to each table
tblctl import att/limits.xlsx --sheet Limits --out docs/limits.md
tblctl check docs/*.md                    # lint Markdown tables
tblctl export docs/limits.md --out limits.xlsx
```

Rules, first match wins:

| Rule | Target   | When                                                                       |
| ---- | -------- | -------------------------------------------------------------------------- |
| T0   | skip     | empty, duplicate of an earlier table, or a revision history                |
| T1   | Markdown | within the size limits, one visible sheet, no merged data cells, no images |
| T2   | CSV      | larger, one visible sheet, no merged data cells, no images, no formulas    |
| T3   | link     | everything else: a one-line link to the source with sheet and size         |

`classify` prints one line per table and a summary line:

```text
T1  markdown docs/spec.md#line 1  1×2
T2  csv      att/limits.xlsx#Limits  40×2  rows 40 > 30
2 tables: 0 T0, 1 T1, 1 T2, 0 T3
```

`import` writes the format the rule selects; `--out` must end in `.md` (T1, T3)
or `.csv` (T2). `--sheet` selects a table when the input has several (a sheet
name, `line N` for Markdown, `table N` for DOCX). `import` and `export` write
their status line (`T1  markdown a.xlsx#S  -> out.md`) to stderr; their output
is the file, so stdout stays empty.

`check` prints one line per pipe table outside code blocks and HTML comments:

```text
PASS    [T1] docs/spec.md:12  3×4
FAIL    [T1] docs/spec.md:40  45×6 rows 45 > 30; suggest CSV
2 tables: 1 passed, 1 failed
```

`--strict` is accepted; it will turn `REVIEW` results into failures once a check
can report `REVIEW`.

`export` writes each table of a Markdown or CSV file to one sheet of an XLSX
file. The output is byte-identical for the same input.

Merged data cells are detected in XLSX, XLS, ODS and DOCX. They cannot be read
from XLSB, so every XLSB table is T3 (link) with the reason
`merged cells
unknown (XLSB)`. Images are detected in XLSX and DOCX, not in XLS,
XLSB or ODS.

On a terminal, `PASS` is green, `FAIL` red, and the `classify` rule is dim (T0),
green (T1, T2) or yellow (T3). Set `NO_COLOR` to a non-empty value to turn
colour off, or `CLICOLOR_FORCE` to turn it on when the output is not a terminal.
`NO_COLOR` wins when both are set.

The T1 limits are 30 data rows, 8 columns and 120 characters per cell. In
Markdown, a link counts as its text only. `--max-rows`, `--max-cols` and
`--max-cell` change the limits on `classify`, `import` and `check`.

## JSON output

With `--json`, every command prints one JSON object on stdout, also when an
error occurs ([AD-0005](docs/decisions/0005-json-envelope-and-human-output.md)):

```sh
tblctl classify --json spec.md missing.xlsx
```

```json
{
  "schema": "tblctl/classify/v1",
  "results": [
    {
      "file": "spec.md",
      "table": "line 1",
      "rule": "t1",
      "target": "markdown",
      "rows": 1,
      "columns": 2,
      "reasons": []
    }
  ],
  "summary": {
    "tables": 1,
    "t0": 0,
    "t1": 1,
    "t2": 0,
    "t3": 0
  },
  "errors": [
    {
      "code": "read_failed",
      "message": "missing.xlsx: Xlsx error: I/O error: No such file or directory (os error 2): ...",
      "file": "missing.xlsx"
    }
  ]
}
```

| Command    | `results`, one object per                                                           | `summary`                         |
| ---------- | ----------------------------------------------------------------------------------- | --------------------------------- |
| `classify` | table: `file`, `table`, `rule`, `target`, `rows`, `columns`, `reasons`              | `tables`, `t0`, `t1`, `t2`, `t3`  |
| `check`    | table: `file`, `line`, `status`, `rule`, `rows`, `columns`, `reasons`               | `tables`, `pass`, `fail`          |
| `import`   | imported table: `file`, `table`, `rule`, `target`, `out` (`null` for T0), `reasons` | `written` (0 or 1)                |
| `export`   | worksheet: `table`, `sheet`, `rows`, `columns`                                      | `out` (`null` on error), `sheets` |

Each error has `code`, `message`, `file`, a `hint` when there is one, and the
fields of its code. Codes: `read_failed`, `unsupported_type`, `no_table`,
`several_tables` (with `tables`), `no_such_table` (with `tables`),
`wrong_extension` (with `expected`), `same_file`, `write_failed`. Errors are
also written to stderr as `tblctl: <message>`. An argument error (an unknown
flag, a missing argument) is printed as text on stderr by the argument parser,
with exit 2 and no JSON.

## License

MIT

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.

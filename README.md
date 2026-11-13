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

Exit codes: `0` clean · `1` ≥1 check failed · `2` operational error.

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

`classify` prints one line per table, or with `--json` an array of objects
(`file`, `table`, `rule`, `target`, `rows`, `columns`, `reasons`).

`import` writes the format the rule selects; `--out` must end in `.md` (T1, T3)
or `.csv` (T2). `--sheet` selects a table when the input has several (a sheet
name, `line N` for Markdown, `table N` for DOCX).

`check` prints one line per pipe table outside code blocks:

```text
PASS    [T1] docs/spec.md:12  3×4
FAIL    [T1] docs/spec.md:40  45×6 rows 45 > 30; suggest CSV
```

`check --json` prints an array of objects (`file`, `line`, `status`, `rule`,
`rows`, `columns`, `reasons`). `--strict` is accepted; it will turn `REVIEW`
results into failures once a check can report `REVIEW`.

`export` writes each table of a Markdown or CSV file to one sheet of an XLSX
file. The output is byte-identical for the same input.

Merged data cells are detected in XLSX, XLS, ODS and DOCX, not in XLSB. Images
are detected in XLSX and DOCX, not in XLS, XLSB or ODS.

The T1 limits are 30 data rows, 8 columns and 120 characters per cell. In
Markdown, a link counts as its text only. `--max-rows`, `--max-cols` and
`--max-cell` change the limits on `classify`, `import` and `check`.

## License

MIT

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.

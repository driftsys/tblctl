# tblctl

A hermetic, cross-platform CLI for tables in Markdown docs (GitLab, GitHub,
mdbook): it classifies tables from XLSX/XLS/ODS/DOCX/CSV sources, imports them
as a Markdown table, a diffable CSV or a link stub, checks Markdown tables
against size limits, and exports back to XLSX.

> **Status: early development.** `check` works. `classify`, `import` and
> `export` are stubbed and exit `2` with "not yet implemented". Scope and rules
> are tracked in **[#1](https://github.com/driftsys/tblctl/issues/1)**.

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

```sh
tblctl check docs/*.md
```

`check` prints one line per pipe table outside code blocks:

```text
PASS    [T1] docs/spec.md:12  3×4
FAIL    [T1] docs/spec.md:40  45×6 rows 45 > 30; suggest CSV
```

The T1 limits are 30 data rows, 8 columns and 120 characters per cell (counted
in the Markdown source). `--max-rows`, `--max-cols` and `--max-cell` change
them. `--json` prints an array with one object per table (`file`, `line`,
`status`, `rule`, `rows`, `columns`, `reasons`). `--strict` is accepted; it will
turn `REVIEW` results into failures once a check can report `REVIEW`.

## License

MIT

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.

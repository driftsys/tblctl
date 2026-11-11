# tblctl

A hermetic, cross-platform CLI for tables in Markdown docs (GitLab, GitHub,
mdbook): it classifies tables from XLSX/XLS/ODS/DOCX/CSV sources, imports them
as a Markdown table, a diffable CSV or a link stub, checks Markdown tables
against size limits, and exports back to XLSX.

> **Status: early development.** Commands (`classify`, `import`, `check`,
> `export`) are stubbed and exit `2` with "not yet implemented". Scope and rules
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

## License

MIT

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.

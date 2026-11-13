# AD-0003: Flags and inputs

## Context

The commands need a common flag set before more commands are built. The rules T0
to T3 already choose the output format for an imported table.

## Options

- `--target md|csv|link` on `import` from the start. Rejected: the rules already
  choose the format by default.
- No override flag; rules always decide. Rejected: a table that the rules send
  to CSV could not be put into Markdown.
- `--as md|csv|link` on `import`, as an explicit override of the rule for one
  table. Chosen.

## Decision

- `--json` is available on every command (`classify`, `import`, `check`,
  `export`) and prints the envelope of
  [AD-0005](0005-json-envelope-and-human-output.md). Before AD-0005, only
  `classify` and `check` had it.
- `--strict` is available on `check` only. It turns `REVIEW` into exit 1.
- Commands that report results accept many input files.
- There is no `--target` flag. The rules T0 to T3 choose the output format.
- `import --as md|csv|link` overrides that choice for one table (needed to put a
  table that the rule sends to CSV, for example a 40-row table, into Markdown).
  Without `--as`, the rule decides. With `--as`, the result keeps the computed
  `rule`, `target` is the forced format, `reasons` ends with
  `target forced by --as`, and a T0 table is written too, except an empty table,
  which is never written.

`import` and `export` convert one file, so they take a single input.
`import --sheet` selects one table when the input has several.

## Consequences

- Without `--as`, the output format is predictable from the rules alone.
- `--as` makes the override explicit on the command line and visible in the JSON
  result, so a forced format is never mistaken for a rule decision.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0001, AD-0002, AD-0005

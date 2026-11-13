# AD-0003: Flags and inputs

## Context

The commands need a common flag set before more commands are built. The rules T0
to T3 already choose the output format for an imported table.

## Options

- `--target md|csv|link` on `import` from the start. Deferred: the rules already
  choose the format. An override such as `import --as md|csv|link` can be added
  if a need appears.
- No `--target`; rules decide. Chosen.

## Decision

- `--json` is available on every command (`classify`, `import`, `check`,
  `export`) and prints the envelope of
  [AD-0005](0005-json-envelope-and-human-output.md). Before AD-0005, only
  `classify` and `check` had it.
- `--strict` is available on `check` only. It turns `REVIEW` into exit 1.
- Commands that report results accept many input files.
- There is no `--target` flag. The rules T0 to T3 choose the output format.

`import` and `export` convert one file, so they take a single input.
`import --sheet` selects one table when the input has several.

## Consequences

- Output format is predictable from the rules alone.
- An override flag, if added later, is a new decision.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0001, AD-0002, AD-0005

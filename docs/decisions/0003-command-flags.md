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

- `--json` is available on each command that reports results (`classify`,
  `check`).
- `--strict` is available on `check` only. It turns `REVIEW` into exit 1.
- Commands that report results accept many input files.
- There is no `--target` flag. The rules T0 to T3 choose the output format.

Implementation status: `check` has `--json`, `--strict` and many inputs.
`classify` declares `--json` and many inputs in its argument definition but is a
stub. `import` and `export` take a single file.

## Consequences

- Output format is predictable from the rules alone.
- An override flag, if added later, is a new decision.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0001, AD-0002

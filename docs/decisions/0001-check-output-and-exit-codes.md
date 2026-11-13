# AD-0001: `check` output lines, states and exit codes

## Context

`tblctl check` reports one result per table. The sibling tools pixctl and
diagctl already define a line-per-result output and a three-value exit contract.
tblctl needs the same shape so that scripts and CI treat the three tools alike.

## Options

- PASS and FAIL only. Rejected: an unclear case (for example colour that may
  carry meaning, or merged cells that cannot be flattened) would be forced into
  one of the two states.
- PASS, FAIL and REVIEW, with `--strict` to turn REVIEW into a failure. Chosen.

## Decision

`check` prints one line per table: state, rule, `file:line`, detail.

```text
PASS    [T1] docs/spec.md:42  12×5
FAIL    [T1] docs/spec.md:88  45×6 rows 45 > 30; suggest CSV
```

- `PASS`: the table is within the T1 limits.
- `FAIL`: the table exceeds a T1 limit. The detail names each exceeded limit and
  suggests CSV. Exit 1.
- `REVIEW`: a human must decide. Exit 0; with `--strict`, exit 1.

Exit codes follow the repository contract: `0` clean, `1` at least one check
failed, `2` operational error. As built, exit 2 is returned when any input file
cannot be read; the remaining files are still checked and reported.

Implementation status: `PASS` and `FAIL` are built. No check produces `REVIEW`
yet, so `--strict` is accepted and has no effect. The REVIEW part of this
decision is agreed but not implemented.

## Consequences

- Scripts can rely on the same exit codes across tblctl, pixctl and diagctl.
- Adding a check that reports `REVIEW` needs no change to the output shape or
  the flags.
- The JSON form of this output is the envelope of
  [AD-0005](0005-json-envelope-and-human-output.md), described in
  [design/check.md](../design/check.md). AD-0005 also adds a summary line after
  the result lines and colours the state word on a terminal.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0003, AD-0004, AD-0005

# AD-0005: JSON envelope and human output

## Context

Agents and scripts read tblctl output. Before this decision, `classify --json`
and `check --json` printed a bare array of results (AD-0002, AD-0001), errors
went to stderr only, and `import` and `export` had no JSON output. An agent had
to read two streams and parse error text to learn what failed. The text output
also lacked a summary line, colour and examples in `--help`, which the
[clig.dev](https://clig.dev) guidelines recommend.

## Options

- A bare JSON array, as in AD-0002. Rejected: errors and totals are not in the
  document, so a consumer must also read stderr and count results itself.
- Results on stdout as JSON, errors only on stderr as text. Rejected: a consumer
  must parse error text to learn the cause, and the two streams can interleave.
- NDJSON, one object per line. Rejected: it streams well, but a consumer must
  tell result, summary and error lines apart, and the files tblctl reads are
  small enough to report in one document.
- One JSON object (an envelope) with results, summary and errors. Chosen: one
  parse gives everything, errors have stable codes, and the `schema` field
  carries a version so the shape can change without breaking consumers.

## Decision

With `--json`, every command (`classify`, `import`, `check`, `export`) writes
exactly one pretty-printed JSON object to stdout, also when an error occurs:

```json
{
  "schema": "tblctl/<command>/v1",
  "results": [],
  "summary": {},
  "errors": []
}
```

- `results`: `classify` gives one object per table (`file`, `table`, `rule`,
  `target`, `rows`, `columns`, `reasons`); `check` one object per table (`file`,
  `line`, `status`, `rule`, `rows`, `columns`, `reasons`); `import` one object
  (`file`, `table`, `rule`, `target`, `out`, `reasons`), where `out` is `null`
  when T0 writes nothing; `export` one object per worksheet written (`table`,
  `sheet`, `rows`, `columns`).
- `summary`: `classify` `{tables, t0, t1, t2, t3}`; `check`
  `{tables, pass, fail}`; `import` `{written}` (0 or 1); `export`
  `{out, sheets}`, where `out` is `null` when nothing was written.
- `errors`: one object per operational error with `code`, `message` and `file`,
  a `hint` when there is one, and the fields of its code. The codes are stable:
  `read_failed`, `unsupported_type`, `no_table`, `several_tables` (with
  `tables`), `no_such_table` (with `tables`), `wrong_extension` (with
  `expected`: `csv`, `md` or `xlsx`), `same_file` and `write_failed`.
- Each error is also written to stderr as one line `tblctl: <message>`, in JSON
  and text mode.
- Exit codes do not change: 2 if there is any error, otherwise 1 if any check
  failed, otherwise 0.
- Argument errors found by `clap` (an unknown flag, a missing argument) are
  printed by `clap` as text on stderr, with exit 2 and no envelope.

Human output:

- Every command's `--help` has an `Examples:` section, and the top-level help
  ends with a link to the README.
- `check` text output ends with `N tables: P passed, F failed`; `classify` text
  output ends with `N tables: a T0, b T1, c T2, d T3`. One table is written
  `1 table`.
- The state word of `check` (`PASS` green, `FAIL` red) and the rule of
  `classify` (T0 dim, T1 and T2 green, T3 yellow) are coloured only when stdout
  is a terminal. `NO_COLOR` with a non-empty value turns colour off;
  `CLICOLOR_FORCE` with a non-empty value turns it on when stdout is not a
  terminal. `NO_COLOR` wins over `CLICOLOR_FORCE`. Alignment is the same with
  and without colour.
- `import` and `export` write their status line to stderr. Their output is the
  file, so stdout stays empty in text mode.

This decision supersedes the array shape of AD-0002 and the JSON form of `check`
referenced by AD-0001. pixctl and diagctl are asked to adopt the same envelope.

## Consequences

- A consumer parses one document and reads `errors[].code` instead of message
  text.
- A shape change increments the version in `schema`.
- Scripts that read the bare arrays of v0.0.1 must read `results` instead.
- Scripts that read the `import` or `export` status line from stdout must read
  stderr, or use `--json`.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0001, AD-0002, AD-0003

# AD-0002: `classify --json` shape

Superseded by [AD-0005](0005-json-envelope-and-human-output.md) for the JSON
shape: the bare array and the stderr-only read errors of this record are
replaced by the JSON envelope. The result objects described under "Still in
force" are the `results` field of that envelope.

## Context

`classify` reports the rule (T0 to T3) and target format for each table in its
inputs. Its JSON output should be read by the same tooling that reads pixctl
`inspect --json`.

## Options

- One JSON object per input file, with the tables nested inside. Rejected: it
  differs from pixctl.
- A flat array with one object per table, as pixctl `inspect --json`. Chosen.

## Decision

Superseded by AD-0005:

- `classify --json` printed a pretty-printed JSON array with one object per
  table. AD-0005 replaces the array with an envelope whose `results` field holds
  these objects.
- An input that failed to read produced an error on stderr only. AD-0005 also
  reports it as an entry of the envelope's `errors` field.

Still in force:

- One result object per table, and no result object for an input that fails to
  read.
- Enum values are lowercase.
- Order is deterministic: input order, then document order.

Each object has the fields `file`, `table`, `rule`, `target`, `rows`, `columns`
and `reasons`. For example, `rule` is `"t2"`, `target` is `"csv"` and `reasons`
is `["rows 120 > 30"]`.

`target` is one of `skip` (T0), `markdown` (T1), `csv` (T2) and `link` (T3).
`table` is the sheet name for workbooks, `line N` for Markdown, `table N` for
DOCX and `data` for CSV.

## Consequences

- A change to the result object fields edits this record; a change to the
  envelope edits AD-0005.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0003, AD-0005

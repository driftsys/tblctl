# AD-0002: `classify --json` shape

## Context

`classify` reports the rule (T0 to T3) and target format for each table in its
inputs. Its JSON output should be read by the same tooling that reads pixctl
`inspect --json`.

## Options

- One JSON object per input file, with the tables nested inside. Rejected: it
  differs from pixctl.
- A flat array with one object per table, as pixctl `inspect --json`. Chosen.

## Decision

`classify --json` follows the pixctl `inspect --json` convention:

- A pretty-printed JSON array with one object per table.
- An input that fails to read produces an error on stderr and no object.
- Enum values are lowercase.
- Order is deterministic: input order, then document order.

Each object has the fields `file`, `table`, `rule`, `target`, `rows`, `columns`
and `reasons`. For example, `rule` is `"t2"`, `target` is `"csv"` and `reasons`
is `["rows 120 > 30"]`.

`target` is one of `skip` (T0), `markdown` (T1), `csv` (T2) and `link` (T3).
`table` is the sheet name for workbooks, `line N` for Markdown, `table N` for
DOCX and `data` for CSV.

## Consequences

- Consumers can use one parser for tblctl and pixctl output.
- Changing the shape of `classify --json` edits this record.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0003

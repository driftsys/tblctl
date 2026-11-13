# tblctl CLI conventions

Status: agreed 2026-11-11. Scope: issue #1. Aligns tblctl with pixctl and
diagctl before any command is implemented.

## 1. `check` output and exit codes

One line per table:

```text
PASS    [T1] docs/spec.md:42  12×5
FAIL    [T1] docs/spec.md:88  45×6 rows 45 > 30; suggest CSV
REVIEW  [T1] docs/spec.md:120 colour may carry meaning
```

Fields: state, rule, `file:line`, detail.

- `PASS` — the table is within the T1 limits.
- `FAIL` — the table exceeds a T1 limit. The detail names the limit and suggests
  CSV. Exit 1.
- `REVIEW` — a human must decide (for example, colour that may carry meaning, or
  merged cells that cannot be flattened). Exit 0; with `--strict`, exit 1.

Exit codes follow the repository contract: `0` clean, `1` at least one check
failed, `2` operational error.

## 2. `classify --json` shape

Same convention as pixctl `inspect --json`:

- A pretty-printed JSON array with one object per table.
- Inputs that fail to read produce an error on stderr and no object.
- Enum values are lowercase.
- Order is deterministic: input order, then document order.

```json
[
  {
    "file": "a.xlsx",
    "table": "Sheet1",
    "rule": "t2",
    "target": "csv",
    "rows": 120,
    "columns": 6,
    "reasons": ["rows 120 > 30"]
  }
]
```

## 3. Flags

- `--json` on each command that reports results (`classify`, `check`).
- `--strict` on `check` only; it turns `REVIEW` into exit 1.
- Commands that report results take many input files.
- No `--target`: the rules T0–T3 choose the output format. An override such as
  `import --as md|csv|link` can be added later if a need appears.

## 4. Thresholds

CLI flags only for v0.1, on `check` and `classify`:

- `--max-rows` (default 30)
- `--max-cols` (default 8)
- `--max-cell` (default 120 characters)

No configuration file. Projects that need other values pass the flags from a
`just` recipe or CI.

## Alternatives considered

- PASS/FAIL only for `check` — rejected: unclear cases would be forced into one
  of the two states.
- One JSON object per input file with nested tables — rejected: differs from
  pixctl.
- `--target md|csv|link` on `import` from the start — deferred: the rules
  already choose the format.
- `.tblctl.toml` with flag override — deferred: more to build and test than v0.1
  needs.

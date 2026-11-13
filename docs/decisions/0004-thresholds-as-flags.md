# AD-0004: T1 thresholds are command-line flags only

## Context

The T1 rule limits the size of a Markdown table. Projects need to change the
limits, and the way to configure them must stay small for v0.1.

## Options

- A `.tblctl.toml` file, with flags overriding it. Deferred: more to build and
  test than v0.1 needs.
- Flags only. Chosen.

## Decision

`check`, `classify` and `import` take the T1 thresholds as flags only:

- `--max-rows`, default 30 data rows.
- `--max-cols`, default 8 columns.
- `--max-cell`, default 120 characters. In Markdown cells, a link or image
  `[text](url)` counts as `text` only, because spec tables often hold links
  whose URLs the reader does not see.

There is no configuration file. A project that needs other values passes the
flags from a `just` recipe or from CI.

## Consequences

- No configuration discovery or precedence rules to define or test.
- Each project repeats its custom values in its own recipes.
- Adding a configuration file later is a new decision.

Satisfies: [issue #1](https://github.com/driftsys/tblctl/issues/1) Related:
AD-0001

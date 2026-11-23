# Meaningful Cell Colour Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** XLSX tables whose data cells carry meaningful colour are classified T3
with the reason `colour` (#8).

**Architecture:** A new module `src/colour.rs` holds a pure decorative-colour
rule and an XLSX style reader (`quick-xml` over the zip parts, as `read.rs`
already does for ODS and DOCX). `read.rs` sets a new `Facts.colour`, and
`rules.rs` turns it into a T3 structure reason.

**Tech Stack:** Rust 2021, `zip` 9, `quick-xml` 0.42, calamine 0.38 (cells),
`rust_xlsxwriter` 0.102 (test fixtures).

**Spec:** `docs/wip/2026-11-23-colour-design.md`

## Global Constraints

- Header = first row of the sheet's used range (calamine `range.start()`); data
  cells = all later rows of the used range, empty cells included.
- Cell colour = (fill colour, font colour), compared as written. No theme
  resolution.
- Fill colour: `fgColor` of a `patternFill` whose `patternType` is not `none`.
  Absent or `none` = no fill.
- Font colour: the font's `color` element. Absent or `auto="1"` = default.
- Decorative iff every data cell's colour depends only on (column, row parity).
- Any `conditionalFormatting` whose `sqref` overlaps a data cell = meaningful.
- Only `Sheets::Xlsx` (XLSX and XLSM) is checked. Other formats:
  `colour =
  false`.
- Reason text: exactly `colour`.
- Commit scopes allowed: cli, check, classify, import, export, docs, ci, hooks,
  deps, repo, install, release. Use `classify`.
- `just build` must pass (fmt, clippy `-D warnings`, tests).

## Review Focus

1. Style index resolution order for an empty cell: cell `s`, else row `s` when
   the row has `customFormat="1"`, else the `<col style>` covering the column,
   else 0. A highlighted empty data cell must count. Test in Task 2.
2. `sqref` with several space-separated ranges and single cells (`"A1:B2 D5"`).
   Test in Task 2.
3. A used range that does not start at A1 (header row is not row 1). Test in
   Task 3.
4. Theme colours that differ only by `tint` are different colours. Test in
   Task 2.
5. Relationship targets written as absolute (`/xl/worksheets/sheet1.xml`) or
   relative (`worksheets/sheet1.xml`). Test in Task 2.

---

### Task 1: Decorative-colour rule

**Files:**

- Create: `src/colour.rs`
- Modify: `src/main.rs` (add `mod colour;`)

**Interfaces:**

- Produces: `pub type Colour = (Option<String>, Option<String>);` (fill, font)
  and `pub fn varies(cells: &[(u32, u32, Colour)]) -> bool` taking (row, col,
  colour) for data cells only. Returns `true` when two cells with the same
  column and the same row parity have different colours.

- [ ] **Step 1: Write the failing unit tests** in `src/colour.rs`
      `#[cfg(test)] mod tests`, with a helper building a grid from a closure
      `f(row, col) -> Colour` over rows 1..=6, cols 0..=2:
  - `uniform_colour_is_decorative`: every cell `(Some("FFFF0000"), None)` →
    `!varies`.
  - `column_colour_is_decorative`: colour by column only → `!varies`.
  - `row_bands_are_decorative`: fill on even rows, none on odd → `!varies`.
  - `bands_plus_column_colour_is_decorative`: (band by parity, font by column) →
    `!varies`.
  - `one_red_cell_is_meaningful`: all default except (3, 1) red → `varies`.
  - `coloured_total_row_is_meaningful`: last row (6) filled, others not →
    `varies`.
- [ ] **Step 2: Run** `cargo test colour::` — expected: compile failure
      (`varies` not defined).
- [ ] **Step 3: Implement** `varies` with a `HashMap<(u32, u32), &Colour>` keyed
      by (col, row % 2).
- [ ] **Step 4: Run** `cargo test colour::` — expected: 6 passed.
- [ ] **Step 5: Commit** `feat(classify): add the decorative colour rule` (body
      `Refs #8`).

### Task 2: XLSX style and conditional-format reader

**Files:**

- Modify: `src/colour.rs`

**Interfaces:**

- Consumes: `Colour`, `varies` (Task 1).
- Produces:
  `pub fn xlsx_meaningful(path: &Path) -> anyhow::Result<HashMap<String, SheetColour>>`
  keyed by sheet name, and `pub struct SheetColour { /* private */ }` with
  `pub fn meaningful(&self, start: (u32, u32), end: (u32, u32)) -> bool`.
  `start`/`end` are calamine's absolute zero-based (row, col) of the used range;
  row `start.0` is the header. `meaningful` builds the data cells of rows
  `start.0 + 1 ..= end.0`, cols `start.1 ..= end.1`, resolves each cell's colour
  (Review Focus 1), and returns `true` when `varies` is true or a
  conditional-format range overlaps those cells.

Reading steps: `xl/styles.xml` → `fonts`, `fills`, `cellXfs` (each `xf` has
`fontId`, `fillId`); `xl/workbook.xml` `sheet name r:id` +
`xl/_rels/workbook.xml.rels` `Relationship Id Target` → sheet part path (Review
Focus 5); each sheet part → `col min max style`, `row r s
customFormat`,
`c r s`, `conditionalFormatting sqref`. Parse A1 references to zero-based (row,
col).

- [ ] **Step 1: Write the failing unit tests**, building minimal XLSX zips in
      the test with `zip::ZipWriter` from XML strings (a helper
      `fn zip_xlsx(parts: &[(&str, &str)]) -> tempfile::NamedTempFile`, using
      the `tempfile` dev-dependency):
  - `fill_on_one_data_cell_is_meaningful`.
  - `pattern_type_none_is_no_fill`: a fill with `patternType="none"` and an
    `fgColor` on one cell → not meaningful.
  - `font_colour_counts`: one cell with a red font → meaningful.
  - `missing_s_uses_style_zero`.
  - `empty_cell_takes_row_then_column_style` (Review Focus 1): an empty data
    cell absent from the sheet XML, inside a `customFormat` row with a fill →
    meaningful; same with a `<col style>` covering one column only → not
    meaningful (whole column).
  - `tint_makes_a_different_colour` (Review Focus 4).
  - `conditional_format_on_data_is_meaningful` with `sqref="A1:A1 B3"` (Review
    Focus 2); `conditional_format_on_header_only_is_not`.
  - `absolute_and_relative_targets_resolve` (Review Focus 5).
- [ ] **Step 2: Run** `cargo test colour::` — expected: compile failure.
- [ ] **Step 3: Implement** `xlsx_meaningful` and `SheetColour::meaningful` in
      `src/colour.rs` with `quick_xml::Reader` events, following the style of
      `read.rs` `ods::merges`.
- [ ] **Step 4: Run** `cargo test colour::` — expected: all pass.
- [ ] **Step 5: Commit** `feat(classify): read cell colour from XLSX styles`
      (body `Refs #8`).

### Task 3: Wire the fact into classification, test end to end, document

**Files:**

- Modify: `src/table.rs` (`Facts`), `src/read.rs:172-255` (`workbook_tables`),
  `src/rules.rs:134-146` (structure reasons and the rule unit test at `:318`),
  `tests/convert.rs`, `README.md:101-105`

**Interfaces:**

- Consumes: `colour::xlsx_meaningful`, `SheetColour::meaningful` (Task 2).
- Produces: `Facts.colour: bool` with doc comment "Data cells carry meaningful
  colour (XLSX only)."

- [ ] **Step 1: Write the failing tests.**
  - `src/rules.rs`: add `|f: &mut Facts| f.colour = true` to
    `merged_data_cells_images_or_several_sheets_are_t3`, expecting T3 with
    reason `colour`.
  - `tests/convert.rs` `colour_is_t3_unless_decorative`, using the existing
    `xlsx` helper and `rust_xlsxwriter::Format::set_background_color`, then
    `classify_json`:
    - `varied.xlsx`: one data cell red → `"rule": "t3"`, reasons contain
      `"colour"`.
    - `header.xlsx`: only the header row filled → `"t1"`.
    - `bands.xlsx`: even data rows filled → `"t1"`.
    - `cond.xlsx`: `rust_xlsxwriter::ConditionalFormatCell` on the data cells →
      `"t3"` with `"colour"`.
    - `offset.xlsx` (Review Focus 3): table starting at C3, header filled, one
      data cell filled → `"t3"`; header-only fill → `"t1"`.
- [ ] **Step 2: Run** `cargo test colour` — expected: compile failure on
      `f.colour`, then the integration test fails.
- [ ] **Step 3: Implement.** Add the field, and set `colour: false` in the other
      `Facts` literals (`src/read.rs:123`, `:146`, `:420`). In
      `workbook_tables`, for `Sheets::Xlsx(_)` call
      `colour::xlsx_meaningful(path)` once before the sheet loop (error context
      `file`), and set
      `colour: map.get(name).is_some_and(|c| c.meaningful(start, end))` with
      `start`/`end` from the range (`false` for an empty range). In `rules.rs`,
      push `"colour"` after `"images"`.
- [ ] **Step 4: Update `README.md`** next to the image sentence: "Meaningful
      cell colour (fills, font colours and conditional formats that are not
      header-only, whole-column or alternating row bands) is detected in XLSX
      and XLSM, and makes a table T3 with the reason `colour`. It is not
      detected in XLS, XLSB, ODS or DOCX."
- [ ] **Step 5: Run** `just build` — expected: success.
- [ ] **Step 6: Commit**
      `feat(classify): classify tables with meaningful
      colour as T3` (body
      `Closes #8`).

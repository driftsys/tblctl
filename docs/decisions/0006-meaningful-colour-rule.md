# AD-0006: Meaningful cell colour makes a table T3

## Context

Some XLSX attachments carry meaning in cell colour, for example a red, amber or
green status cell. Converting such a table to Markdown or CSV loses the meaning
without a warning. Issue #1 requires "colour not meaningful" for T1 and T2.
tblctl must decide which colour is meaningful. A false T3 only keeps a table as
XLSX; a missed case loses information. The rule therefore prefers a false T3 to
a missed case.

## Options

- A. Any variation: two or more different colours on data cells give T3. Simple,
  but striped rows also become T3. Rejected: banding is common and carries no
  meaning.
- B. Variation that is not decoration: ignore the header, whole-column colour
  and alternating row bands; count any other variation and any conditional
  format on data cells. Chosen.
- C. A REVIEW state instead of T3. Rejected for now: it depends on issue #10,
  whose REVIEW trigger is not decided.
- Within B, a banding rule based on row parity at every table size. Rejected
  after review: it missed status tables of 1 to 3 data rows.

## Decision

For XLSX and XLSM, a table is T3 with the reason `colour` when its data cells
carry meaningful colour. The header (the first row of the used range) is
ignored.

- The colour of a data cell is the pair (fill, font) as written in
  `xl/styles.xml`. Themes are not resolved to RGB. The style is the cell's `s`,
  else the row style (with `customFormat`), else the column style, else style 0.
  A cell whose colour equals the colour of style 0 counts as uncoloured, because
  writers give the default font an explicit theme colour.
- 1 data row: any coloured data cell is meaningful.
- 2 or 3 data rows: different colours within one column are meaningful.
- 4 or more data rows: colour is decorative if and only if it depends only on
  the column and the parity of the row. Any other variation is meaningful, so a
  highlighted total row is meaningful.
- Any conditional format whose range overlaps a data cell is meaningful.
- Excel table styles do not use cell styles and are ignored.
- XLS, XLSB, ODS and DOCX are not checked for colour.

## Consequences

- Colours that are equal but encoded differently (for example `rgb` and `theme`)
  compare as different, which can give a false T3.
- Known limits, accepted: cells or rows without an `r` attribute are dropped or
  misnumbered; conditional formats in `x14` `extLst` are not read; `../`
  relationship targets are not normalised; `gradientFill` and number-format
  colour (`[Red]`) are not detected; a hyperlink cell style (blue font) can give
  a false T3.
- Moving to a REVIEW state later (option C) is a new decision tied to issue #10.

Satisfies: [issue #8](https://github.com/driftsys/tblctl/issues/8),
[issue #1](https://github.com/driftsys/tblctl/issues/1) Related: AD-0002

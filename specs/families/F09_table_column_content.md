# F09 — Table column content

## Members

- **Table.AddColumn** — add a column whose cells are the lambda's per-row return.
- **Table.AddIndexColumn** — add a sequential index column.
- **Table.DuplicateColumn** — copy a column under a new name.
- **Table.TransformColumns** — apply a per-column lambda to one or more columns.
- **Table.TransformColumnTypes** — re-type one or more columns.
- **Table.ReplaceValue** — replace cells matching a value with a replacement (per-column, per-cell, or per-row).
- **Table.SplitColumn** — split a column by delimiter or width into multiple columns.
- **Table.CombineColumns** — combine columns into one.
- **Table.FillDown** — propagate non-null values downward in named columns.
- **Table.FillUp** — propagate non-null values upward in named columns.

## Shared argument shape

Step reference plus per-column descriptors. The descriptor shape varies considerably; per-function leaves describe the exact hint.

## Shared type signature

*(Table, descriptor) → Table*. Most members preserve schema row count; AddColumn and SplitColumn widen the schema.

## Shared schema-transform rule

Per-member. Each member's schema rule is documented in its leaf because there is genuine variation.

## Shared runtime semantics

1. Evaluate the input table.
2. Apply the per-member transformation per row, producing a fresh table (R-INV-09).
3. For per-row lambdas, push the underscore stack per R-EACH-02.

## Shared SQL lowering

- Schema-widening members (AddColumn, SplitColumn, CombineColumns) lower to SELECT projections that compute the new column expressions alongside the input columns.
- Per-cell transformations (TransformColumns, TransformColumnTypes, ReplaceValue) lower to SELECT projections that wrap the affected columns in CASE expressions or CAST.
- Window-function members (FillDown, FillUp, AddIndexColumn) lower to SELECT projections using window functions.

## Shared null/empty/error rules

Per-member. Each leaf documents its specific null and error handling.

## Per-member appendix

This appendix is intentionally minimal because each member has enough specifics to warrant a leaf file. See `functions/table/<Function>.md` for the per-member contract.

## Conformance

Family-level fixtures: `conformance/families/F09_table_column_content/`.
Per-member fixtures: `conformance/functions/<Function>/`.


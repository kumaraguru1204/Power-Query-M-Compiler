# Table.RemoveRowsWithErrors

**Family.** F07 — table row filter. See `families/F07_table_row_filter.md`.

**Per-member operation.** Returns a fresh table containing only the rows for which every cell coerces successfully to its column's inferred type. Rows with at least one coercion failure are dropped.

**Edge cases.**
- All cells coerce successfully → input unchanged.
- All rows have at least one coercion failure → empty result.
- Coercion-failure tracking is partial today (the executor coerces eagerly per cell on demand). A pre-pass that pre-tags failing rows is on the roadmap.

**Conformance.** `conformance/functions/Table.RemoveRowsWithErrors/`.


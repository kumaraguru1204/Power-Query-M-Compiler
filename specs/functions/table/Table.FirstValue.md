# Table.FirstValue

**Family.** F10 — table aggregation. See `families/F10_table_aggregation.md`.

**Per-member operation.** Returns the first column's first row's cell value, coerced per the column's inferred type. Optional second argument: a default value if the table is empty.

**Edge cases.**
- Empty table → default if supplied; else null.

**Conformance.** `conformance/functions/Table.FirstValue/`.


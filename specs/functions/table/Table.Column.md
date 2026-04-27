# Table.Column

**Family.** F10 — table aggregation. See `families/F10_table_aggregation.md`.

**Per-member operation.** Returns the named column's cell values as a list, in row order. The list's element type is the column's inferred type.

**Edge cases.**
- Unknown column → E302.
- Empty table → empty list.

**Conformance.** `conformance/functions/Table.Column/`.


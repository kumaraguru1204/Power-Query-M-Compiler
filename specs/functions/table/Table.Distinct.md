# Table.Distinct

**Family.** F07 — table row filter. See `families/F07_table_row_filter.md`.

**Per-member operation.** Returns rows whose tuple of values (across the equality-key columns) is unique. Without the optional column-name list, all columns participate. Source order is preserved; for duplicate tuples, the first-encountered row is kept.

**Edge cases.**
- No optional columns → equality key is the full row.
- A column name not in the schema → E302.
- Empty input → empty output.

**Conformance.** `conformance/functions/Table.Distinct/`.


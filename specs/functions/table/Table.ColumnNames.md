# Table.ColumnNames

**Family.** F10 — table aggregation. See `families/F10_table_aggregation.md`.

**Per-member operation.** Returns the column names of the input table as a List-of-Text in column order.

**Edge cases.**
- Empty table (zero rows but defined schema) → list of column names (non-empty).
- A table with zero columns is impossible by construction (R-TBL-01 requires at least one column from the workbook payload's headers).

**Conformance.** `conformance/functions/Table.ColumnNames/`.


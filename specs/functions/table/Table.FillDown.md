# Table.FillDown

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** In each named column, replaces null cells with the most recent non-null value above. Two arguments: table, column-name list.

**Schema rule.** Schema preserved.

**Edge cases.**
- A column whose first cells are null and there is no non-null above stays null.
- Empty input → empty output.
- Column unknown: E302.

**Conformance.** `conformance/functions/Table.FillDown/`.


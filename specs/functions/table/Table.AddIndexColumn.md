# Table.AddIndexColumn

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Appends a sequential integer column. Optional second argument is the column name (default *Index*). Optional third argument is the starting integer (default 0). Optional fourth argument is the increment (default 1).

**Schema rule.** Output schema = input schema + one new Integer column at the end.

**Edge cases.**
- Empty input → empty output with the new column declared but no rows.
- Negative increment is permitted.

**Conformance.** `conformance/functions/Table.AddIndexColumn/`.


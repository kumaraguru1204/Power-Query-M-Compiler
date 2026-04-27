# Table.DuplicateColumn

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Appends a copy of an existing column under a new name. Three required args: table, source column name, new column name. Optional fourth: type for the duplicate.

**Schema rule.** Output schema = input schema + one new column with the same type (or declared) at the end.

**Edge cases.**
- Source column unknown: E302.
- New name clashes with existing column: E402.

**Conformance.** `conformance/functions/Table.DuplicateColumn/`.


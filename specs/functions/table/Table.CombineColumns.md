# Table.CombineColumns

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Combines several columns into one column. Four arguments: table, source column names, combiner lambda (typically *Combiner.CombineTextByDelimiter*), new column name.

**Schema rule.** Output schema = input schema with the source columns removed and the new column inserted at the position of the first source column.

**Edge cases.**
- Combiner dispatch is partial; only delimiter-combining is well-supported.
- Null cells are typically rendered as empty text by the default combiner; per-combiner policy on the roadmap.

**Conformance.** `conformance/functions/Table.CombineColumns/`.


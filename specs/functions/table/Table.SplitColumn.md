# Table.SplitColumn

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Splits one column into multiple columns, by delimiter or by fixed width. Three or more arguments: table, source column name, splitter lambda (typically *Splitter.SplitTextByDelimiter*), optional new column names, optional new column types.

**Schema rule.** Output schema = input schema with the source column removed and the new columns inserted at its position.

**Edge cases.**
- Splitter dispatch is partial; today only delimiter-based splitting is well-supported.
- A row with fewer parts than expected pads with nulls; with more parts truncates. Per-replacer policy is on the roadmap.

**Conformance.** `conformance/functions/Table.SplitColumn/`.


# Table.TransformColumns

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Applies a per-column lambda to one or more columns. The second argument is a list of pairs *{column-name, lambda}* (or a list of triples if a result type is declared). Each lambda takes the cell value and returns the new cell value.

**Schema rule.** Output schema = input schema with each named column re-typed to the lambda's return type (or the declared type if supplied).

**Edge cases.**
- Named column unknown: per missing-field policy (default Error → E302).
- Mixed inferred return types across rows: type-checker computes the common upper bound; runtime widens per R-NUM-04 if numeric.

**Conformance.** `conformance/functions/Table.TransformColumns/`.


# Table.AddColumn

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Appends one column whose name is the second-argument string and whose cells are the per-row return value of the third-argument lambda (a row-context lambda). Optional fourth argument is a column type to declare for the new column.

**Schema rule.** Output schema = input schema + one new column at the end with the declared type (or inferred from the lambda's return type if absent).

**Edge cases.**
- Column-name clash with an existing column: E402 (FunctionSignatureMismatch — schema clash subcategory). Some implementations of M auto-suffix; this implementation errors. Tracked.
- Lambda returns null on every row: column is created with the declared type, all cells null.

**Conformance.** `conformance/functions/Table.AddColumn/`.


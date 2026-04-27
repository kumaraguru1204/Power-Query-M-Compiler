# Table.TransformColumnTypes

**Family.** F09 — table column content. See `families/F09_table_column_content.md`.

**Per-member operation.** Re-types one or more columns. The second argument is a list of pairs *{column-name, type}*. Cells are re-coerced from raw text to the declared type. Optional third argument is a culture string or an options record.

**Schema rule.** Output schema = input schema with the named columns re-typed.

**Edge cases.**
- Named column unknown: E302.
- A cell that fails to coerce to the new type: E507 at execution time. (M's official semantics produce a per-cell error value rather than aborting; this implementation aborts. Tracked.)
- Culture/options record currently parsed but not applied. Tracked.

**Conformance.** `conformance/functions/Table.TransformColumnTypes/`.


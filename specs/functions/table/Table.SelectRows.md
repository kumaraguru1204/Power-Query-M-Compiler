# Table.SelectRows

**Family.** F07 — table row filter. See `families/F07_table_row_filter.md`.

**Per-member operation.** Returns a fresh table containing only the rows for which the row-context predicate lambda returns true. Schema preserved (R-TBL-04).

**Edge cases.**
- Empty input table → empty output table preserving schema (R-TBL-05).
- Predicate returning null on a row: row excluded (R-NULL-06).
- Predicate not Boolean at type-check: should be E405; today the check is partial. The runtime evaluates whatever the body returns and treats it as truthy/falsy per R-NULL-06; the SQL emitter may produce invalid SQL. Tracked.

**Conformance.** `conformance/functions/Table.SelectRows/`.


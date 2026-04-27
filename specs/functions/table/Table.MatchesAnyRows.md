# Table.MatchesAnyRows

**Family.** F07 — table row filter (Boolean variant). See `families/F07_table_row_filter.md`.

**Per-member operation.** Returns true iff the row-context predicate returns true for at least one row. Empty table returns false. Short-circuits at the first true.

**Edge cases.** Same predicate-type-enforcement gap as `Table.SelectRows`.

**Conformance.** `conformance/functions/Table.MatchesAnyRows/`.


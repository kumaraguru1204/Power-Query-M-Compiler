# Table.MatchesAllRows

**Family.** F07 — table row filter (Boolean variant). See `families/F07_table_row_filter.md`.

**Per-member operation.** Returns true iff the row-context predicate returns true for every row. Empty table returns true (vacuous truth). Short-circuits at the first false (or null, treated as false).

**Edge cases.** Same predicate-type-enforcement gap as `Table.SelectRows`.

**Conformance.** `conformance/functions/Table.MatchesAllRows/`.


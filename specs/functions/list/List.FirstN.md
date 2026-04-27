# List.FirstN

**Family.** F06 — table and list row trim. See `families/F06_table_row_trim.md`.

**Per-member operation.** Returns the first N elements of the input list.

**Edge cases.**
- N greater than length → input list unchanged.
- N is null → treated as zero (returns empty list).
- The optional second-argument *condition* form (where N is replaced by a predicate that consumes a prefix until first false) is not yet supported; tracked.

**Conformance.** `conformance/functions/List.FirstN/`.


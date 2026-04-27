# List.Select

**Family.** F05 — list higher-order. See `families/F05_list_higher_order.md`.

**Per-member operation.** Returns the elements of the input list for which the predicate lambda returns true. Source order preserved.

**Edge cases.**
- Empty input → empty list.
- Predicate returning null on an element: that element is excluded (R-NULL-06).
- Predicate returning a non-Boolean value: should be E405 at type-check time. Today this passes silently and produces invalid SQL (W002 plus a runtime error if the SQL is executed). Tracked in `PROJECT_REPORT.md`.

**Conformance.** `conformance/functions/List.Select/`.


# List.MatchesAny

**Family.** F05 — list higher-order. See `families/F05_list_higher_order.md`.

**Per-member operation.** Returns true iff the predicate lambda returns true for at least one element. Short-circuits at the first true.

**Edge cases.**
- Empty list → false.
- Predicate returning a non-Boolean: same gap as List.Select.

**Conformance.** `conformance/functions/List.MatchesAny/`.


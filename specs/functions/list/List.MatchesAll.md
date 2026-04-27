# List.MatchesAll

**Family.** F05 — list higher-order. See `families/F05_list_higher_order.md`.

**Per-member operation.** Returns true iff the predicate lambda returns true for every element. Short-circuits at the first false (or null, treated as false per R-NULL-06).

**Edge cases.**
- Empty list → true (vacuous truth).
- Predicate returning a non-Boolean: same gap as List.Select; tracked.

**Conformance.** `conformance/functions/List.MatchesAll/`.


# List.RemoveNulls

**Family.** F04 — list set operation. See `families/F04_list_set_operation.md`.

**Per-member operation.** Returns the input list with null elements dropped. Source order preserved. Equivalent to `List.Select(L, each _ <> null)` but more concise.

**Edge cases.**
- Empty list → empty list.
- All-nulls list → empty list.
- No nulls → input unchanged.

**Conformance.** `conformance/functions/List.RemoveNulls/`.


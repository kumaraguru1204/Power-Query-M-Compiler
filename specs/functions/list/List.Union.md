# List.Union

**Family.** F04 — list set operation. See `families/F04_list_set_operation.md`.

**Per-member operation.** Takes a list of lists. Returns the deduplicated union. Output order: first occurrence of each distinct value across all input lists in source order.

**Edge cases.**
- Outer list empty → empty result.
- All inner lists empty → empty result.

**Conformance.** `conformance/functions/List.Union/`.


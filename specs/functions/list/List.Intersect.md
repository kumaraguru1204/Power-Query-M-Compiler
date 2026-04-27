# List.Intersect

**Family.** F04 — list set operation. See `families/F04_list_set_operation.md`.

**Per-member operation.** Takes a list of lists. Returns the elements present in every input list. Output order follows the first input list.

**Edge cases.**
- Outer list empty → empty result.
- Any inner list empty → empty result.
- A single inner list → that list deduplicated.

**Conformance.** `conformance/functions/List.Intersect/`.


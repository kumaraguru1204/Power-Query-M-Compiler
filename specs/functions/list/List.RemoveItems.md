# List.RemoveItems

**Family.** F04 — list set operation. See `families/F04_list_set_operation.md`.

**Per-member operation.** Returns the elements of the first list whose value equals (R-VAL-10) no element of the second list. Source order and duplicates are preserved (an element of the first list is removed only when it has any equal in the second; duplicates of a non-removed value all stay).

**Edge cases.**
- Empty first list → empty result.
- Empty second list → first list unchanged.

**Conformance.** `conformance/functions/List.RemoveItems/`.


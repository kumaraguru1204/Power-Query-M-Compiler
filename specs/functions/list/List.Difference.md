# List.Difference

**Family.** F04 — list set operation. See `families/F04_list_set_operation.md`.

**Per-member operation.** Returns the elements of the first list that are not equal (R-VAL-10) to any element of the second list. Source order is preserved.

**Edge cases.**
- Empty first list → empty result regardless of second list.
- Empty second list → first list unchanged.
- Duplicates in the first list are preserved (this is the difference from `List.RemoveItems`, which deduplicates by side-effect of EXCEPT semantics in some lowerings).

**Conformance.** `conformance/functions/List.Difference/`.


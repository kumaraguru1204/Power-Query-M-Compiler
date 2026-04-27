# List.Distinct

**Family.** F04 — list set operation. See `families/F04_list_set_operation.md`.

**Per-member operation.** Returns the input list with duplicate elements removed; the first occurrence of each value is kept and the rest dropped. Source order of the kept elements is preserved.

**Edge cases.**
- Empty list → empty list.
- A single null is kept; multiple nulls collapse to one (per R-VAL-10 null-equals-null).
- Optional second argument (an equality predicate lambda) is recognised by the parser but currently ignored; tracked in PROJECT_REPORT.

**Conformance.** `conformance/functions/List.Distinct/`.


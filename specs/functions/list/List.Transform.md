# List.Transform

**Family.** F05 — list higher-order. See `families/F05_list_higher_order.md`.

**Per-member operation.** Returns a new list whose elements are the lambda's return values, applied to each element of the input list in source order. The output element type is the lambda's return type.

**Edge cases.**
- Empty input → empty list (the output element type defaults to None at type-check time, since no body ever runs).
- A lambda that returns null on every element produces a list of nulls; List.Transform does not drop nulls.

**Conformance.** `conformance/functions/List.Transform/`.


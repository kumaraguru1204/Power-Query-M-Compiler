# List.Range

**Family.** F06 — table and list row trim. See `families/F06_table_row_trim.md`.

**Per-member operation.** Returns the elements at zero-based positions [offset, offset + count). The third argument *count* is optional; when absent, returns from offset to the end.

**Edge cases.**
- offset greater than or equal to length → empty list.
- offset + count greater than length → returns elements from offset to end.
- offset less than zero → E505.

**Conformance.** `conformance/functions/List.Range/`.


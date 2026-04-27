# List.RemoveMatchingItems

**Family.** F04 — list set operation. See `families/F04_list_set_operation.md`.

**Per-member operation.** Like `List.RemoveItems` but with an equality lambda supplied as the third argument. Returns the elements of the first list for which the lambda returns false against every element of the second list.

**Edge cases.**
- Lambda returning null: treated as false (R-NULL-06).
- The lambda's argument shape is two-arg; today the catalogue treats it as a row-context lambda over the underscore as the first-list element. Confirm against official M; tracked.

**Conformance.** `conformance/functions/List.RemoveMatchingItems/`.


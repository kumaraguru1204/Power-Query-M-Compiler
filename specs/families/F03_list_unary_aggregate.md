# F03 — List unary aggregate

## Members

- **List.First** — first element, or null on empty.
- **List.Last** — last element, or null on empty.
- **List.Single** — the sole element; error if not exactly one.
- **List.SingleOrDefault** — the sole element, or a supplied default; error if more than one.
- **List.Min** — minimum, ignoring nulls.
- **List.Max** — maximum, ignoring nulls.
- **List.Median** — median (50th percentile), ignoring nulls.
- **List.Mode** — most frequent element, ignoring nulls.
- **List.Modes** — all most-frequent elements (a list).
- **List.IsDistinct** — true iff every element appears at most once.
- **List.PositionOf** — zero-based position of the first occurrence; -1 if absent.
- **List.PositionOfAny** — zero-based position of the first element matching any of a set.
- **List.Contains** — true iff the list contains a given element.
- **List.ContainsAll** — true iff the list contains every element of a given list.
- **List.ContainsAny** — true iff the list contains at least one element of a given list.

## Shared argument shape

Members take one required argument (the list); some take a second positional argument (the needle, the default, or the lookup list) per the per-member appendix.

## Shared type signature

*(List-of-T [, T or List-of-T or scalar default]) → R*, where T is the list's element type and R is per the per-member appendix.

## Shared schema-transform rule

Not applicable (expression-level).

## Shared runtime semantics

1. Evaluate the list argument to a runtime list value.
2. Apply the per-member reducer over the elements in source order, using the family's null-handling rule (R-LIST-07, R-NULL-07).
3. Return the result.

## Shared SQL lowering

Most members lower to a SQL aggregate (MIN, MAX, etc.) applied over a values-CTE built from the list. Per-member appendix gives the exact SQL.

## Shared null/empty/error rules

- Empty list: per the per-member appendix.
- Null elements: per R-NULL-07 (most aggregates ignore them).

## Per-member appendix

| Function             | Extra arg              | Reducer                                        | Empty-list behaviour                  | SQL                              |
| -------------------- | ---------------------- | ---------------------------------------------- | ------------------------------------- | -------------------------------- |
| List.First           | none                   | First element.                                 | null                                  | LIMIT 1                          |
| List.Last            | none                   | Last element.                                  | null                                  | ORDER BY position DESC LIMIT 1   |
| List.Single          | none                   | The unique element.                            | E505 (or domain error)                | SELECT only when COUNT=1         |
| List.SingleOrDefault | default value          | The unique element, or default.                | default                               | as above with COALESCE           |
| List.Min             | none                   | Numerical or lexicographical minimum.          | null                                  | MIN                              |
| List.Max             | none                   | Numerical or lexicographical maximum.          | null                                  | MAX                              |
| List.Median          | none                   | Median; for even count, mean of the two middle.| null                                  | PERCENTILE_CONT(0.5)             |
| List.Mode            | none                   | Most frequent; ties: first encountered.        | null                                  | not-yet-lowered (W002)           |
| List.Modes           | none                   | All most-frequent elements as a list.          | empty list                            | not-yet-lowered (W002)           |
| List.IsDistinct      | none                   | All elements unique.                           | true                                  | COUNT vs COUNT(DISTINCT)         |
| List.PositionOf      | needle                 | Index of first match; -1 if absent.            | -1                                    | not-yet-lowered (W002)           |
| List.PositionOfAny   | needle list            | Index of first element matching any.           | -1                                    | not-yet-lowered (W002)           |
| List.Contains        | needle                 | True iff present.                              | false                                 | EXISTS                           |
| List.ContainsAll     | needle list            | True iff every needle present.                 | true                                  | NOT EXISTS over the difference   |
| List.ContainsAny     | needle list            | True iff at least one needle present.          | false                                 | EXISTS over intersect            |

## Conformance

Family-level fixtures: `conformance/families/F03_list_unary_aggregate/`.
Per-member fixtures: `conformance/functions/<Function>/` for each.

## Open questions

- Several members lack SQL lowering today (W002). Tracked.
- Mode tie-breaking convention should be confirmed against official M.


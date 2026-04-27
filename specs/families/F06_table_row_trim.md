# F06 — Table and List row trim

## Members

- **Table.FirstN** — first N rows of a table.
- **Table.LastN** — last N rows of a table.
- **Table.Range** — rows from offset to offset+count of a table.
- **Table.ReverseRows** — rows in reverse order.
- **List.FirstN** — first N elements of a list.
- **List.LastN** — last N elements of a list.
- **List.Range** — elements from offset to offset+count of a list.
- **List.RemoveFirstN** — list without its first N elements.
- **List.RemoveLastN** — list without its last N elements.
- **List.Skip** — list without its first N elements (alias of RemoveFirstN).

## Shared argument shape

Two or three positional arguments: a step reference (Table members) or list/step-or-value (List members), then one or two integers describing the slice.

## Shared type signature

For Table members: *(Table, Integer [, Integer]) → Table* (schema preserved).
For List members: *(List-of-T, Integer [, Integer]) → List-of-T*.

## Shared schema-transform rule

The output schema is identical to the input schema (Table members) or has the same element type (List members). Row count differs.

## Shared runtime semantics

1. Evaluate the input.
2. Apply the per-member slicing rule from the appendix using zero-based indices (R-LIST-04).
3. Return a fresh table or list with the selected rows/elements (R-INV-09).

## Shared SQL lowering

Members map to SQL LIMIT / OFFSET / window-function combinations per the appendix. Some lowerings depend on dialect; the default targets PostgreSQL syntax.

## Shared null/empty/error rules

- N less than zero: R-ERR-EXEC-INDEX (E505).
- N greater than the input length: silently clamped (returns whatever rows/elements exist).
- N is null: treated as zero.

## Per-member appendix

| Function              | Slicing rule                                                | SQL                                          |
| --------------------- | ----------------------------------------------------------- | -------------------------------------------- |
| Table.FirstN          | Rows [0, N).                                                | LIMIT N                                      |
| Table.LastN           | Rows [count - N, count).                                    | ORDER BY index DESC LIMIT N + reverse        |
| Table.Range           | Rows [offset, offset + count).                              | OFFSET offset LIMIT count                    |
| Table.ReverseRows     | Rows in reverse.                                            | ORDER BY rownum DESC                         |
| List.FirstN           | Elements [0, N).                                            | LIMIT N over values-CTE                      |
| List.LastN            | Elements [count - N, count).                                | LIMIT N + reverse                            |
| List.Range            | Elements [offset, offset + count).                          | OFFSET offset LIMIT count                    |
| List.RemoveFirstN     | Elements [N, count).                                        | OFFSET N                                     |
| List.RemoveLastN      | Elements [0, count - N).                                    | LIMIT (count - N)                            |
| List.Skip             | Same as RemoveFirstN.                                       | OFFSET N                                     |

## Conformance

Family-level fixtures: `conformance/families/F06_table_row_trim/`.
Per-member fixtures: `conformance/functions/<Function>/` where leaves exist.

## Open questions

- Table.LastN requires a stable row index; today it relies on input order.


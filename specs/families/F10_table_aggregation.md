# F10 — Table aggregation

## Members

- **Table.RowCount** — number of rows.
- **Table.Column** — extract one column as a list.
- **Table.ColumnNames** — list of all column names.
- **Table.FirstValue** — value of the first column of the first row.

## Shared argument shape

Step reference plus, for some members, a column-name string.

## Shared type signature

*(Table [, Text]) → R*, where R varies per member: Integer (RowCount), List-of-T (Column), List-of-Text (ColumnNames), T (FirstValue).

## Shared schema-transform rule

The output is a single value or a list, not a table. The catalogue's schema-transform hook returns an empty schema; the executor wraps the result as documented in R-EXEC-03.

## Shared runtime semantics

1. Evaluate the input table.
2. Apply the per-member computation.
3. Return the result; the executor's value-binding wrapping turns it into a one-row, one-column table or a one-column table per R-EXEC-03 if the call appears as a step.

## Shared SQL lowering

- RowCount → COUNT(*).
- Column → SELECT col FROM previous_cte.
- ColumnNames → SELECT column_name FROM information_schema (or hard-coded literal list per the type-checker schema).
- FirstValue → LIMIT 1 + first column projection.

## Shared null/empty/error rules

- Empty table: RowCount returns 0; Column returns empty list; ColumnNames returns the schema's column names regardless; FirstValue returns null.
- Unknown column name (Column): E302.

## Per-member appendix

| Function           | Extra arg     | Returns                                              |
| ------------------ | ------------- | ---------------------------------------------------- |
| Table.RowCount     | none          | Integer row count.                                   |
| Table.Column       | column name   | List of cell values for the named column.            |
| Table.ColumnNames  | none          | List of column names.                                |
| Table.FirstValue   | none          | First-column-first-row cell value, or null on empty. |

## Conformance

Family-level fixtures: `conformance/families/F10_table_aggregation/`.
Per-member fixtures: `conformance/functions/<Function>/`.


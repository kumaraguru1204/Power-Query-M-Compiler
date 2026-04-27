# F08 — Table column shape

## Members

- **Table.SelectColumns** — keep only the named columns.
- **Table.RemoveColumns** — drop the named columns.
- **Table.RenameColumns** — rename per a name-pair list.
- **Table.ReorderColumns** — reorder per a name list.
- **Table.HasColumns** — Boolean: does the table have all named columns?
- **Table.TransformColumnNames** — apply a renaming function to every column name.

## Shared argument shape

Step reference plus a column-shape descriptor. The descriptor varies per member (a column-name list, a rename pair list, a renaming lambda) per the per-member appendix.

## Shared type signature

*(Table, descriptor [, missing-field policy]) → Table* — except HasColumns which returns Boolean.

## Shared schema-transform rule

The output schema is the input schema with columns added, removed, renamed, or reordered per the per-member rule. The output schema is computed at type-check time and trusted by every later stage (R-INV-10).

## Shared runtime semantics

1. Evaluate the input table.
2. Build a fresh table whose columns are the projection of the input columns according to the per-member rule.
3. Cells inside selected columns are not re-coerced; the original raw text values pass through (R-INV-09 cloning).

## Shared SQL lowering

Members lower to a SELECT projection over the previous CTE; column ordering and aliases come from the type-checker-computed schema (R-SQL-11).

## Shared null/empty/error rules

- A named column that does not exist: per the missing-field policy (the optional last argument). Default is Error (E302). Other policies: Ignore (skip), UseNull (synthesise a null column).
- Empty input table: output has the transformed schema, zero rows.

## Per-member appendix

| Function                       | Descriptor              | Schema rule                                                                 |
| ------------------------------ | ----------------------- | --------------------------------------------------------------------------- |
| Table.SelectColumns            | column-name list        | Output columns = those named, in the order given.                            |
| Table.RemoveColumns            | column-name list or string | Output columns = input columns minus the named ones.                       |
| Table.RenameColumns            | rename pair list        | Output columns = input columns with names mapped per pairs.                  |
| Table.ReorderColumns           | column-name list        | Output columns = those named first (in given order), then the rest.          |
| Table.HasColumns               | column-name list        | (Boolean output; no schema change.)                                          |
| Table.TransformColumnNames     | renaming lambda         | Output columns = input columns with each name passed through the lambda.    |

## Conformance

Family-level fixtures: `conformance/families/F08_table_column_shape/`.

## Open questions

- Missing-field policy parsing is consistent across members but the implementation is per-function today; consolidation owed.


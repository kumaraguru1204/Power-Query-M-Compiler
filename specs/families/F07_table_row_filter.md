# F07 — Table row filter

## Members

- **Table.SelectRows** — rows where the predicate lambda returns true.
- **Table.RemoveRowsWithErrors** — rows where no cell coercion fails.
- **Table.SelectRowsWithErrors** — rows where at least one cell coercion fails.
- **Table.Distinct** — rows whose tuple of values is unique (optionally restricted to a column subset).
- **Table.MatchesAllRows** — true iff every row satisfies the predicate.
- **Table.MatchesAnyRows** — true iff at least one row satisfies the predicate.

## Shared argument shape

Most members take a step reference plus a row-context lambda. Distinct takes a step reference and an optional column-name list.

## Shared type signature

Predicate-style: *(Table, RowRecord → Boolean) → Table* (or → Boolean for the *Matches* predicates).
Distinct: *(Table [, List-of-Text]) → Table*.

## Shared schema-transform rule

Schema preserved.

## Shared runtime semantics

1. Evaluate the input table.
2. Iterate rows in source order.
3. For each row, push the underscore stack with the row's record (R-EACH-02).
4. Evaluate the predicate (or the equality of the column subset for Distinct).
5. Combine per the per-member rule.

## Shared SQL lowering

Predicate members lower to a WHERE clause containing the lambda body lowered per `cross_cutting/14_sql_lowering_principles.md`. Distinct lowers to SELECT DISTINCT (or DISTINCT ON for the optional column subset). The Matches predicates lower to EXISTS / NOT EXISTS over the WHERE-filtered subquery.

## Shared null/empty/error rules

- Predicate returning null: row excluded (R-NULL-06).
- Predicate not Boolean at type-check: E405. (Today often passes silently and produces invalid SQL; tracked.)
- Empty input table: returns empty table preserving schema (R-TBL-05).
- Distinct on a column that does not exist: E302 (UnknownColumn).

## Per-member appendix

| Function                    | Predicate or extra              | Per-row decision                            | SQL                                |
| --------------------------- | ------------------------------- | ------------------------------------------- | ---------------------------------- |
| Table.SelectRows            | row-context predicate           | Keep iff predicate true.                    | WHERE                              |
| Table.RemoveRowsWithErrors  | none                            | Keep iff no cell-coercion failure.          | not-yet-lowered (W002)             |
| Table.SelectRowsWithErrors  | none                            | Keep iff at least one cell-coercion failure.| not-yet-lowered (W002)             |
| Table.Distinct              | optional column-name list       | Keep first row of each tuple-equivalence.   | SELECT DISTINCT [ON (cols)]        |
| Table.MatchesAllRows        | row-context predicate           | All rows satisfy.                           | NOT EXISTS over negation           |
| Table.MatchesAnyRows        | row-context predicate           | At least one row satisfies.                 | EXISTS                             |

## Conformance

Family-level fixtures: `conformance/families/F07_table_row_filter/`.
Per-member fixtures: `conformance/functions/<Function>/`.

## Open questions

- Cell-coercion-error rows: today the executor coerces eagerly per row and the tracking infrastructure is partial; design owed.
- Predicate type enforcement gap (E405).


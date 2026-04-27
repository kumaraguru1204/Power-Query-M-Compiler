# Cross-cutting: SQL Lowering Principles

## Scope
Applies to the SQL-emission stage. Cited by every family file's "Shared SQL lowering" section.

## Rules

**R-SQL-01 — Each step becomes a named subquery.** The SQL emitter walks the program and produces a chain of common-table-expressions (named subqueries). Each step's CTE is named after the step. The final query selects from the output step's CTE.

**R-SQL-02 — The workbook initialiser becomes a values-CTE.** The first step produces a CTE that is a literal-values subquery built from the input table's data: every cell is rendered as a literal in its column's type, and every column is named by its header.

**R-SQL-03 — Function calls dispatch by name.** For every catalogued function, the SQL emitter looks up a per-function lowering procedure. If found, it produces a CTE referencing the previous step's CTE; if not found, it emits a comment placeholder and warning W002.

**R-SQL-04 — Lambda bodies become SQL expressions.** A lambda body is lowered by walking the same operators and column-references as the executor uses; the result is a SQL expression substituted at the appropriate position (a WHERE clause, a SELECT projection, an ORDER BY clause, a GROUP BY column, etc.).

**R-SQL-05 — Implicit-underscore handling.** When a lambda body refers to the implicit underscore in a row context, the SQL emitter substitutes the appropriate column-qualified reference (the surrounding CTE name plus the column name). When a lambda body refers to the underscore in a list-element context where the values-CTE has aliased the column to a synthetic name (typically *Value*), the underscore lowers to that synthetic-column reference.

**R-SQL-06 — Type-driven literal rendering.** Integer literals render as bare digits. Float literals render with the decimal point. Text literals render with single-quote escaping (every embedded single-quote is doubled). Boolean literals render as the SQL standard *TRUE* and *FALSE*. Null literals render as *NULL*.

**R-SQL-07 — Quoting of column and CTE names.** Column names and CTE names are double-quoted. Any embedded double-quote inside an M-quoted-identifier is doubled.

**R-SQL-08 — Predicate Boolean-isation.** When a SQL WHERE clause is built from a lambda body whose top-level operator is not naturally a SQL Boolean expression (for example, the body is a column reference with no comparison), the emitter wraps it in *= TRUE* if the body's static type is Boolean. If the body's static type is not Boolean, the emitter must refuse to emit and instead produce W002 plus a placeholder; this prevents invalid SQL such as a bare column in a WHERE clause.

**R-SQL-09 — Dialect.** The default dialect is ANSI SQL with PostgreSQL-compatible extensions (the values-CTE shape, double-quoted identifiers). Per-function lowerings note where they require dialect-specific syntax.

**R-SQL-10 — Unsupported-fallback policy.** A function whose per-function lowering is missing or whose required SQL feature is not available in the chosen dialect produces:
- a CTE definition that selects everything from the previous step's CTE unchanged (so subsequent steps still have a referenceable input),
- a SQL comment immediately above the CTE explaining the fallback,
- a warning W002 in the diagnostics list.

This makes the SQL output runnable while flagging the gap.

**R-SQL-11 — Schema-driven projections.** A step that changes the schema (a column-shape function) lowers its projection list using the type-checker-computed output schema, not by re-deriving from the call arguments. This keeps the SQL projection in lock-step with the type system (R-INV-10).

**R-SQL-12 — Best-effort never aborts the pipeline.** The SQL emitter produces a string for every well-formed input program. It never returns an error category; problems are warnings only. The runtime executor remains the source of truth for results.

## Examples

A program *let Source = Excel.Workbook(File.Contents("x"), null, true), F = Table.SelectRows(Source, each [Age] > 30) in F* lowers (sketched) to: a Source CTE built from VALUES, an F CTE selecting from Source where the *"Age"* column is greater than 30, and a final select from F.

A program whose final step is a function lacking a SQL lowering produces a Source CTE, intermediate CTEs as available, then for the unsupported step a passthrough CTE with a comment and a W002 warning.

## Test coverage

Pointers will live under `conformance/cross_cutting/sql/`.

## Open questions

- R-SQL-08: the wrap-in-*=-TRUE* rule is implemented partially; tracked.
- R-SQL-09: a configurable target dialect would be useful; deferred.


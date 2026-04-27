# Pipeline stage 8 — SQL emission

**Input.** The validated, type-annotated program tree and the input table.

**What it does.** Walks the same program and emits an equivalent SQL query string. Implements the principles in `cross_cutting/14_sql_lowering_principles.md`. Each step becomes a named common-table-expression (CTE):

1. **Workbook initialiser** becomes a values-CTE built from the input table's literal values, with column aliases set to the header names and types driven by the inferred column types (R-SQL-02, R-SQL-06).
2. **Sheet navigation** becomes a passthrough CTE (selecting everything from the input).
3. **Generic function call** dispatches by the function's full name to a per-function lowering procedure (R-SQL-03). The procedure produces a CTE that references the previous step's CTE. If no lowering is registered, emit a passthrough CTE plus a comment plus a W002 warning (R-SQL-10).
4. **Value binding** becomes a CTE selecting a single literal value or a literal list as appropriate.

The final query is a *SELECT \* FROM <output-step-CTE>*.

Lambda bodies are lowered into SQL expressions by walking the same operators and column references the executor uses. Implicit-underscore handling per R-SQL-05.

**Output.** A single string containing the SQL query.

**Failure modes.** None that abort the pipeline. Functions without lowerings produce W002 warnings; the pipeline always returns a string (R-SQL-12).

**Storage shape.** Just a string.


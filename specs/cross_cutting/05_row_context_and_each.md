# Cross-cutting: Row Context and the *each* Shorthand

## Scope
Applies to the type-checker (when checking lambda arguments to row-context functions) and the executor (when evaluating lambda bodies). Cited by every family that takes a lambda.

## Rules

**R-EACH-01 — The each-shorthand desugars at parse time.** *each EXPR* parses to an explicit one-parameter lambda whose parameter is the underscore character and whose body is EXPR. After parsing, the rest of the system sees only explicit lambdas; the each-shorthand has no separate node kind.

**R-EACH-02 — Row context for table-row predicates.** When a step-level function takes a lambda whose role is "row predicate" or "row mapper" (functions in F07 row-filter, parts of F09 row-mapping, F10 aggregations), the lambda's single parameter is bound to a record value whose fields are the input table's columns at the current row. Field names match column names; field types match column types.

**R-EACH-03 — Element context for list-element lambdas.** When a step-level or expression-level function takes a lambda whose role is "element predicate" or "element mapper" (members of F05 list-higher-order, several F03 aggregates, several F04 set-operations), the lambda's single parameter is bound to one element of the input list at a time. The element's type is the inner type of the list (R-TYPE-09).

**R-EACH-04 — The underscore stack.** The executor maintains a stack of underscore bindings. Entering a lambda body whose parameter is the underscore pushes a new binding; leaving the body pops it. Within the body, a bare reference to the underscore resolves to the most-recently pushed binding. This makes nested each-shorthands behave the obvious way: the outer underscore is shadowed by the inner one inside the inner body.

**R-EACH-05 — Bare identifiers in row context.** Inside a row-context lambda body, a bare identifier first checks against the parameter (the row record); if it matches a field of the row record, it is treated as that field's value. If it does not match a row column, it is then checked against the surrounding scope (step names, lexically enclosing parameters). If it matches nothing, it is R-ERR-NAME-COL.

**R-EACH-06 — Bracketed column access in row context.** A name in square brackets inside a row-context lambda is a column access: the field of that name is fetched from the current row record. An unknown column is R-ERR-NAME-COL. Bracketed access is the only unambiguous way to refer to columns whose names collide with surrounding identifiers.

**R-EACH-07 — Underscore field access in row context.** *_[ColumnName]* is exactly equivalent to *[ColumnName]* in a row context. The explicit form is preferred when the row context is one nested level deep and the implicit form would be ambiguous.

**R-EACH-08 — Lambdas that are not row-context.** Some functions take lambdas without binding row context (for example, the reducer of List.Accumulate). For those functions, R-EACH-02 and R-EACH-03 do not apply; the lambda's parameters are bound positionally to the function-supplied arguments and nothing else. The catalogue documents this per function.

**R-EACH-09 — Type-checking of lambda bodies.** When the type-checker descends into a lambda body that has been declared row-context for a particular table, it pushes the table's column-name-to-type map into its scope. Bare identifiers and bracket accesses against that scope yield typed expressions. The body's inferred type bubbles up as the lambda's return type (R-TYPE-08).

## Examples

The expression *each [Salary] > 50000* inside a row-context lambda for a table with column *Salary* of type Integer evaluates the access *[Salary]* against the current row record, then compares to the integer 50000, producing a Boolean (R-EACH-06, R-PARSE-06).

The expression *each _[User][Name] = "B"* inside a list-element-context lambda over a list of records evaluates the underscore (the current element record), navigates into its *User* field (a sub-record), then into that sub-record's *Name* field, then compares to the text *"B"* (R-EACH-05, R-PARSE-09).

A nested each-shorthand body referring to the outer row's columns must use the outer row's record explicitly: passing the outer row into the inner closure is required because the inner each shadows the underscore (R-EACH-04).

## Test coverage

Pointers will live under `conformance/cross_cutting/row_context/`.

## Open questions

- R-EACH-04: documentation of the shadowing behaviour for new readers needs more examples.
- R-EACH-08: a small number of functions still need their lambda role classified; tracked in PROJECT_REPORT.


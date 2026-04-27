# Cross-cutting: Runtime Value Model

## Scope
Applies to the executor only. Other stages reason in static types (R-TYPE-01); the executor reasons in runtime values.

## Rules

**R-VAL-01 — The value vocabulary.** A runtime value is one of: an integer, a floating-point number, a boolean, a piece of text, a null, a list of values, or a record (an ordered mapping from field name to value).

**R-VAL-02 — Coercion entry point.** A raw text cell is coerced to a runtime value on demand using the column's inferred type as the coercion target (R-TYPE-06).

**R-VAL-03 — Numeric kinds at runtime.** The system distinguishes only two numeric runtime kinds: integer and float. Currency, Date, DateTime, and other typed columns either coerce to integer/float internally or pass through as text pending fuller temporal support.

**R-VAL-04 — Null is a value.** Null is a first-class runtime value; it is not an absence. Operations on null follow the rules in `06_null_propagation.md`.

**R-VAL-05 — Lists are ordered and homogeneous-by-convention.** A list value preserves insertion order. A list literal mixing element kinds is permitted at runtime but signals upstream type widening (R-TYPE-09).

**R-VAL-06 — Records preserve field-insertion order.** A record value's iteration order is the order in which fields were declared in the source. Field equality is case-sensitive.

**R-VAL-07 — Tables are not runtime values.** Tables flow between steps in the executor's environment but are not part of the value vocabulary; they are a separate kind handled by step-kind dispatch (R-EXEC-01 in `pipeline/07_execution.md`). A list-of-records may *resemble* a table but has different semantics — it is an opaque value to most operators, while a table is queried column-wise.

**R-VAL-08 — The implicit underscore.** Inside an each-shorthand body (or any explicit lambda whose parameter name is the underscore), the underscore identifier resolves to the lambda's first parameter. The executor maintains a stack so nested underscore-binding contexts (a row context inside which a list-element context is opened) do not collide. The most recently pushed binding wins (R-EACH-04 in `05_row_context_and_each.md`).

**R-VAL-09 — Privacy.** No other sub-project references the runtime value vocabulary (Source-of-Truth invariant R-INV-08). Type-checker, formatter, and SQL emitter reason in static types and AST shapes only.

**R-VAL-10 — Equality.** Two integer values are equal iff their numeric values match. Two floats are equal under standard floating-point equality. Integer-versus-float compares numerically after widening the integer (R-NUM-04). Two text values are equal iff their character sequences are identical (case-sensitive). Two booleans are equal iff identical. Null equals null. Two lists are equal iff their element sequences are pairwise equal. Two records are equal iff their field-name sets are equal and their field values are pairwise equal.

## Examples

A cell whose raw text is *"42"* in an Integer column coerces at runtime to the integer 42 (R-VAL-02). The same cell in a Text column coerces to the text *"42"*.

A list value *{1, 2, "3"}* exists at runtime; static type-checking widens its element type to Any (R-TYPE-09).

A row context containing the underscore binding to a record, evaluating a list-comprehension that opens an inner each-shorthand, sees the inner each's underscore shadow the outer one (R-VAL-08).

## Test coverage

Pointers will live under `conformance/cross_cutting/value_model/`.

## Open questions

- R-VAL-03: deferred temporal value kinds are tracked in PROJECT_REPORT.
- R-VAL-07: distinction between list-of-records and tables in lambdas is sometimes surprising; documentation tracking.


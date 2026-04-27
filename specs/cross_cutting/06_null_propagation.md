# Cross-cutting: Null Propagation

## Scope
Applies to the executor and to every family that processes possibly-null values.

## Rules

**R-NULL-01 — Null is a first-class value.** Null is not the absence of a value; it is the runtime value that represents "no data" (R-VAL-04).

**R-NULL-02 — Arithmetic on null short-circuits to null.** Any arithmetic operator (plus, minus, multiply, divide, unary minus) with at least one null operand produces null. Division by zero is checked **before** null short-circuit only when neither operand is null; otherwise null wins.

**R-NULL-03 — Concatenation on null short-circuits to null.** The text-concatenation ampersand with at least one null operand produces null.

**R-NULL-04 — Comparison on null is asymmetric.** Equals and not-equals against null produce a Boolean (null = null is true; null = anything-non-null is false). Other comparisons (less-than, less-than-or-equal, greater-than, greater-than-or-equal) where at least one operand is null produce null.

**R-NULL-05 — Logical *and* and *or* short-circuit with null awareness.** *false and null* is false. *true or null* is true. *true and null* is null. *false or null* is null. *not null* is null.

**R-NULL-06 — Predicate truthiness.** Where a Boolean is required (the result of a row-filter predicate, an *if* condition), null counts as false. The row is excluded; the *if* takes the *else* branch. This is the only place null is silently coerced to a Boolean.

**R-NULL-07 — Null in lists.** A null is a permitted list element. Aggregates have per-aggregate rules: List.Sum and List.Average ignore nulls; List.Count counts them; List.Min and List.Max ignore nulls; List.Distinct treats null as a distinct value equal only to other nulls.

**R-NULL-08 — Null in records.** A record field whose value is null is still present in the record; it is not the same as a missing field. Field access on a record returns the field's value, which may be null.

**R-NULL-09 — Null cells.** A column whose every cell coerced to null leaves the column type as the inferred type and the cells as nulls. A column that is text *"null"* (case-insensitive) is treated as a missing-cell marker by the column-type-inference routine and contributes no information about the column's type.

**R-NULL-10 — Null in coercion.** Coercing the literal text *"null"* to any non-Text type produces the runtime null value. Coercing it to Text produces the text *"null"*.

## Examples

The expression *1 + null* evaluates to null (R-NULL-02). The expression *"a" & null* evaluates to null (R-NULL-03). The expression *null = null* evaluates to true (R-NULL-04). The expression *null < 5* evaluates to null (R-NULL-04).

A row filter whose predicate evaluates to null on a particular row removes that row (R-NULL-06).

List.Sum over the list *{1, null, 3}* returns the integer 4 (R-NULL-07).

## Test coverage

Pointers will live under `conformance/cross_cutting/null/`.

## Open questions

- R-NULL-09: the missing-cell marker should arguably be configurable.


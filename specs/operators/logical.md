# Operator: Logical

## Operators
The keyword *and*, the keyword *or*. (Unary *not* is documented in `unary.md`.)

## Arity
Binary infix.

## Precedence and associativity
*or* is the lowest-precedence operator in the whole language. *and* sits one level above. Both are left-associative (R-PARSE-05).

## Operand types
Both operands must be Boolean. A non-Boolean operand is R-ERR-TYPE-OPERAND.

## Result type
Boolean — except where R-NULL-05 produces null.

## Runtime — null awareness
- *false and X* is false even if X is null.
- *true or X* is true even if X is null.
- *true and null* is null.
- *false or null* is null.
- *null and false* is false; *null and true* is null.
- *null or true* is true; *null or false* is null.
- *null and null* is null; *null or null* is null.

## Short-circuit
The right operand is evaluated only when its value can affect the result. *false and X* does not evaluate X; *true or X* does not evaluate X. This means a side-effecting expression on the right (rare in M but possible through user-defined functions) may not run.

## Type errors
A non-Boolean operand is R-ERR-TYPE-OPERAND (E401).

## SQL lowering
Maps to SQL *AND* and *OR*. SQL's three-valued logic agrees with R-NULL-05.

## Conformance
Pointers to fixtures will live under `conformance/operators/logical/`.


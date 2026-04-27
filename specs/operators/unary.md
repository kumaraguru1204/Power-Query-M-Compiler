# Operator: Unary

## Operators
Unary minus (the minus sign before a primary). The keyword *not*.

## Arity
Unary prefix.

## Precedence and associativity
Both bind tighter than any binary operator (R-PARSE-07).

## Operand types
- Unary minus: numeric (Integer, Float, Currency).
- Unary *not*: Boolean.

## Result type
- Unary minus: same numeric type as the operand.
- Unary *not*: Boolean — except where R-NULL-05 produces null.

## Runtime
- Unary minus negates per the operand's runtime kind. Integer negation of the minimum 64-bit value is R-ERR-EXEC-OVERFLOW.
- Unary *not* flips Boolean values; *not null* is null.

## Null
- Unary minus on null is null (R-NULL-02).
- Unary *not* on null is null (R-NULL-05).

## Literal-folding
A unary minus applied to a numeric literal is folded by the parser into a single negative numeric literal token (R-PARSE-07). This means *- 1* and *-1* parse to the same node.

## Type errors
A non-numeric operand to unary minus is R-ERR-TYPE-OPERAND. A non-Boolean operand to *not* is R-ERR-TYPE-OPERAND.

## SQL lowering
Unary minus maps to SQL unary minus. *not* maps to SQL *NOT*.

## Conformance
Pointers to fixtures will live under `conformance/operators/unary/`.


# Operator: Arithmetic

## Operators
Plus, minus, asterisk, forward slash.

## Arity
Binary infix.

## Precedence and associativity
Plus and minus share the additive precedence level. Asterisk and forward slash share the multiplicative precedence level (one level higher than additive). All four are left-associative (R-PARSE-05).

## Operand types
Both operands must be numeric (Integer, Float, or Currency) per R-NUM-02. The static result type is Integer if both operands are Integer; Float otherwise (R-NUM-03).

## Runtime
Numeric widening per R-NUM-04. Division by zero is R-ERR-EXEC-DIV0 only when neither operand is null. Integer overflow on plus, minus, or multiply is R-ERR-EXEC-OVERFLOW (R-NUM-06). Float operations follow IEEE 754 (R-NUM-06).

## Null
Any null operand short-circuits to null (R-NULL-02). The division-by-zero check is suppressed when either operand is null.

## Type errors
A non-numeric operand is R-ERR-TYPE-OPERAND. Boolean is not numeric (R-NUM-11).

## SQL lowering
Plus, minus, asterisk become the standard SQL operators of the same names. Forward slash becomes SQL division; numeric promotion rules differ slightly across dialects but the default lowering targets ANSI SQL.

## Conformance
Pointers to fixtures will live under `conformance/operators/arithmetic/`.


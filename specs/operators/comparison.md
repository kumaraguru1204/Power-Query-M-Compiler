# Operator: Comparison

## Operators
Equals, not-equals, less-than, less-than-or-equal, greater-than, greater-than-or-equal.

## Arity
Binary infix.

## Precedence and associativity
All six share one precedence level, below additive (R-PARSE-05, R-PARSE-06). Left-associative; chained comparisons are syntactically permitted but rarely useful.

## Operand types
Operand types must be **comparable** per R-TYPE-04: equal types, both numeric, both textual, both temporal in the same family, or one is null.

## Result type
Always Boolean — except for less-than family operators where at least one operand is null, which produce null per R-NULL-04. (Equals and not-equals against null still produce a Boolean.)

## Runtime
Numeric comparison after widening (R-NUM-07). Text comparison by code-point order (R-TEXT-04). Temporal comparison by chronological instant (R-DT-05).

## Null
Equals and not-equals: defined per R-NULL-04. Other four: null operand produces null.

## Type errors
Non-comparable types is R-ERR-TYPE-NONCOMPARABLE (E406).

## SQL lowering
Maps directly to the SQL operators of the same names: *=*, *<>*, *<*, *<=*, *>*, *>=*. SQL's three-valued logic for comparisons against NULL agrees with R-NULL-04.

## Conformance
Pointers to fixtures will live under `conformance/operators/comparison/`.


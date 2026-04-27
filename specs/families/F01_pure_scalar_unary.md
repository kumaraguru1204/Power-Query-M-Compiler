# F01 — Pure scalar unary

## Members

- **Number.Round** — rounds a number to a given number of decimal digits.

## Shared argument shape

One required positional argument: a scalar value (the operand). Optional positional arguments per the per-member appendix.

## Shared type signature

The signature is *(T) → T* where T is the scalar type accepted by the per-member operation. For Number.Round, T is Float (Integer is widened to Float per R-NUM-04).

## Shared schema-transform rule

Not applicable (members of this family are expression-level functions; they do not appear as step-level catalogued calls in this implementation).

## Shared runtime semantics

1. Evaluate the operand to a runtime value.
2. If the operand is null, return null (R-NULL-02 analogue for unary scalar functions).
3. Otherwise apply the per-member operation and return the result.

## Shared SQL lowering

Lowers as the corresponding SQL scalar function applied to the operand. The per-member SQL function name is in the appendix.

## Shared null/empty/error rules

- Null operand: produces null.
- Wrong-type operand: R-ERR-EXEC-TYPE.
- Domain errors (overflow, division-by-zero in inner arithmetic): per the per-member appendix.

## Per-member appendix

| Function       | Operation                                              | Optional args                                  | SQL function | Domain errors |
| -------------- | ------------------------------------------------------ | ---------------------------------------------- | ------------ | ------------- |
| Number.Round   | Round to N decimal digits using banker's rounding.     | Second arg N (Integer); default 0.             | ROUND(x, N)  | None.         |

## Conformance

Family-level fixtures: `conformance/families/F01_pure_scalar_unary/`.
Per-member fixtures: `conformance/functions/Number.Round/`.

## Open questions

- Banker's rounding vs. half-away-from-zero: the choice should match official Power Query M; verification owed.


# F05 — List higher-order

## Members

- **List.Select** — keeps elements where the predicate returns true.
- **List.Transform** — replaces each element with the lambda's return value.
- **List.MatchesAll** — true iff every element satisfies the predicate.
- **List.MatchesAny** — true iff at least one element satisfies the predicate.

## Shared argument shape

Two required arguments: a list (parsed via the StepRefOrValue hint so it may be either a step reference or an inline expression) and a lambda (parsed via the EachExpr hint so an each-shorthand or explicit lambda is accepted).

## Shared type signature

- Predicate-style members (Select, MatchesAll, MatchesAny): *(List-of-T, T → Boolean) → R*, where R is List-of-T (Select) or Boolean (the rest).
- Mapper-style member (Transform): *(List-of-T, T → U) → List-of-U*.

The lambda's parameter type is T (the element type of the input list); the predicate must return Boolean (R-LAMBDA-RET); the mapper may return any type.

## Shared schema-transform rule

Not applicable (expression-level).

## Shared runtime semantics

1. Evaluate the list argument to a runtime list.
2. For each element in source order, push the underscore stack with this element (R-EACH-04), evaluate the lambda body, pop.
3. Combine the per-element results per the per-member rule.

## Shared SQL lowering

Per-member lowering depends on the lambda's complexity. For each-shorthand bodies whose only column reference is the underscore on a values-CTE column aliased *Value*, the lowering is straightforward (Select becomes WHERE; Transform becomes a projection). For more complex bodies, lowering may emit W002.

## Shared null/empty/error rules

- Empty input list: returns the per-member identity (Select returns empty list; MatchesAll returns true; MatchesAny returns false; Transform returns empty list).
- Null elements: passed to the lambda as null. The lambda body is responsible for handling (R-NULL-06 covers the predicate case).
- Predicate returning a non-Boolean value: E405 (LambdaReturnTypeMismatch) at type-check time. Emitted as a warning in this implementation today; tracked in PROJECT_REPORT to upgrade to a hard error.

## Per-member appendix

| Function          | Lambda role | Combiner                                | Empty list returns | SQL                              |
| ----------------- | ----------- | --------------------------------------- | ------------------ | -------------------------------- |
| List.Select       | predicate   | Keep elements where predicate is true.  | empty list         | WHERE                            |
| List.Transform    | mapper      | Output = mapped values.                 | empty list         | projection over values-CTE       |
| List.MatchesAll   | predicate   | All true.                               | true               | NOT EXISTS over negation         |
| List.MatchesAny   | predicate   | Any true.                               | false              | EXISTS                           |

## Conformance

Family-level fixtures: `conformance/families/F05_list_higher_order/`.
Per-member fixtures: `conformance/functions/<Function>/`.

## Open questions

- Lambda-return type-check enforcement is a known gap; tracked in PROJECT_REPORT (`E405` hardening).
- SQL lowering for non-trivial lambda bodies needs design work.


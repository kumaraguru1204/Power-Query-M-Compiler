# F04 — List set operation

## Members

- **List.Difference** — elements in A not in B.
- **List.Intersect** — elements in every supplied list.
- **List.Union** — elements in any supplied list (deduplicated).
- **List.Distinct** — deduplicated single list (one-arg variant).
- **List.RemoveItems** — elements in A whose value equals no element of B.
- **List.RemoveMatchingItems** — same as RemoveItems but with custom equality.
- **List.RemoveNulls** — drops null elements.
- **List.Reverse** — reverses element order.

## Shared argument shape

Either one list (Distinct, RemoveNulls, Reverse) or two lists (Difference, RemoveItems, RemoveMatchingItems) or a list of lists (Intersect, Union). The per-member appendix documents the exact shape.

## Shared type signature

The result type is List-of-T where T unifies across the input list element types.

## Shared schema-transform rule

Not applicable (expression-level).

## Shared runtime semantics

1. Evaluate the input list(s) to runtime list values.
2. Walk per the per-member algorithm from the appendix, using R-VAL-10 equality unless the per-member rule overrides (e.g. RemoveMatchingItems uses an equality predicate supplied by the caller).
3. Element order in the output: per the per-member rule.

## Shared SQL lowering

Most members lower as set-operation SQL (UNION, INTERSECT, EXCEPT) over values-CTEs. Order-preserving members are best-effort in SQL because SQL set operations do not guarantee order.

## Shared null/empty/error rules

- Null in inputs: per-member; some include null as a value (RemoveNulls explicitly drops it).
- Empty input(s): per-member.

## Per-member appendix

| Function                   | Args              | Algorithm                                                              | SQL                            |
| -------------------------- | ----------------- | ---------------------------------------------------------------------- | ------------------------------ |
| List.Difference            | A, B              | Elements of A not equal (R-VAL-10) to any element of B.                | EXCEPT                         |
| List.Intersect             | list of lists     | Elements present in every input list.                                  | INTERSECT chain                |
| List.Union                 | list of lists     | Distinct union of all inputs.                                          | UNION                          |
| List.Distinct              | A                 | Distinct elements of A in source order.                                | SELECT DISTINCT                |
| List.RemoveItems           | A, B              | Elements of A with no equal element in B.                              | EXCEPT (without dedup of A)    |
| List.RemoveMatchingItems   | A, B, predicate   | Elements of A with no element of B satisfying predicate.               | not-yet-lowered (W002)         |
| List.RemoveNulls           | A                 | Elements of A whose runtime value is not null.                         | WHERE x IS NOT NULL            |
| List.Reverse               | A                 | Elements of A in reverse order.                                        | ORDER BY position DESC         |

## Conformance

Family-level fixtures: `conformance/families/F04_list_set_operation/`.
Per-member fixtures: `conformance/functions/<Function>/`.

## Open questions

- Order preservation in set lowering needs explicit documentation per dialect.
- RemoveMatchingItems requires custom-predicate lowering, deferred.


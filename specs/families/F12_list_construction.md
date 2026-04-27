# F12 — List construction

## Members

- **List.Numbers** — arithmetic progression of numbers.
- **List.Repeat** — a list with one value repeated N times.
- **List.Random** — a list of N pseudo-random floats in [0, 1).
- **List.Generate** — a list produced by an initial value, a continuation predicate, and a step function.
- **List.Combine** — flatten a list of lists.

## Shared argument shape

Members take 1–4 positional arguments per the per-member appendix. List.Generate takes lambdas; the others take scalars or lists.

## Shared type signature

*(args…) → List-of-T*, where T is determined per member (Float, Integer, the input element type, etc.).

## Shared schema-transform rule

Not applicable (expression-level, returns a list).

## Shared runtime semantics

1. Evaluate the arguments.
2. Apply the per-member generator from the appendix.
3. Return the materialised list (R-LIST-09).

## Shared SQL lowering

- List.Numbers → generate_series (PostgreSQL) or VALUES.
- List.Repeat → CROSS JOIN to a generate_series.
- List.Random → not-yet-lowered (random functions vary by dialect).
- List.Generate → not-yet-lowered (general recursion is hard in SQL).
- List.Combine → UNION ALL of the inner lists' values-CTEs.

## Shared null/empty/error rules

- N less than zero: R-ERR-EXEC-INDEX (E505).
- N equals zero: returns empty list.

## Per-member appendix

| Function       | Args                                            | Algorithm                                                            |
| -------------- | ----------------------------------------------- | -------------------------------------------------------------------- |
| List.Numbers   | start (Float), count (Integer), step (Float)    | Numbers start, start+step, …, count entries.                          |
| List.Repeat    | value, N (Integer)                              | The value repeated N times.                                           |
| List.Random    | N (Integer)                                     | N pseudo-random Floats in [0, 1).                                     |
| List.Generate  | initial, continue (lambda), next (lambda)       | Iterates next from initial while continue holds.                      |
| List.Combine   | list of lists                                   | Concatenation in order.                                               |

## Conformance

Family-level fixtures: `conformance/families/F12_list_construction/`.

## Open questions

- List.Generate is partially supported; tracked.
- Random reproducibility (seeding) needs design.


# Tables.FromValue

## 1. Identity

- **Full name.** Tables.FromValue.
- **Namespace.** Tables.
- **Family.** None.
- **Status.** Implemented for the basic shapes.

## 2. Argument shape

One argument: a value (StepRefOrValue). The shape varies:

- A scalar → one-row, one-column table with column name *Value*.
- A list → one-column table with column name *Value* and one row per element.
- A record → one-row table with one column per field.

## 3. Type signature

*(Any) → Table*. Signatures vary per call shape.

## 4. Schema-transform rule

Per the call shape, as in §2.

## 5. Runtime semantics

Identical to the value-binding step kind's wrapping behaviour (R-EXEC-03), but explicit at the function-call level so the user can use it inside larger expressions.

## 6. SQL lowering

Lowers to a small VALUES-CTE matching the inferred shape.

## 7. Reference behaviour

Aligns with official M.

## 8. Conformance fixtures

`conformance/functions/Tables.FromValue/`.

## 9. Open questions and known gaps

- None significant.


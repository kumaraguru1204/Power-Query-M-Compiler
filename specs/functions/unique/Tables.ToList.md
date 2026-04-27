# Tables.ToList

## 1. Identity

- **Full name.** Tables.ToList.
- **Namespace.** Tables.
- **Family.** None.
- **Status.** Partial.

## 2. Argument shape

Two required arguments: a step reference (the input table) and a combiner lambda. The combiner is applied per row to the row's record and produces a single value.

## 3. Type signature

*(Table, RowRecord → T) → List-of-T*.

## 4. Schema-transform rule

The output is a list, not a table. Wrapped per R-EXEC-03 if the call appears at step level.

## 5. Runtime semantics

For each row of the input, push the underscore stack with the row record (R-EACH-02), evaluate the combiner, pop. Collect the results into a list in row order.

## 6. SQL lowering

Lowers when the combiner is a recognised projection (a single column, a small concatenation). Otherwise W002.

## 7. Reference behaviour

Aligns with official M.

## 8. Conformance fixtures

`conformance/functions/Tables.ToList/`.

## 9. Open questions and known gaps

- General combiner lowering is a recurring challenge; tracked.


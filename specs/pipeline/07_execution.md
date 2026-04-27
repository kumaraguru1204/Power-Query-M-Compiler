# Pipeline stage 7 — Execution

**Input.** The validated, type-annotated program tree and the input table.

**What it does.** Maintains an **environment** (a mutable map from step name to the table value bound to that step) and an **underscore stack** (R-EACH-04). Walks the bindings in source order. For each binding, dispatches by step kind:

**R-EXEC-01 — Workbook initialiser.** Materialise the input table directly: clone it, re-tag with the call's path string, bind to the step's name. The actual file is never read (R-EXEC-N1).

**R-EXEC-02 — Sheet navigation.** Look up the named input step in the environment, clone its table, bind to the step's name. Sheet selection by name is currently a passthrough (the input table is what was loaded from the payload); R-EXEC-N2.

**R-EXEC-03 — Value binding.** Evaluate the bound expression once. If the result is a list of values, build a one-column table whose column type is inferred from the values. Otherwise, build a one-row, one-column table whose cell holds the value. Bind to the step's name.

**R-EXEC-04 — Generic function call.** Dispatch by the fully-qualified function name to a per-function evaluator. Each per-function evaluator reads its arguments out of the typed argument-union variants and produces an output table. Bind to the step's name.

## Expression evaluation rules

Inside per-function evaluators, expressions are evaluated against the appropriate row context using the runtime value vocabulary (`cross_cutting/04_value_model.md`):

- **Literals** evaluate to the obvious runtime value.
- **Bracket column access** in row context fetches the column's raw text value at the current row index and coerces it per the column's inferred type (R-VAL-02, R-TYPE-06).
- **Bare identifier** resolves per R-EACH-05: implicit underscore, step name, or column in the current row.
- **Field access** evaluates the operand to a record then looks up the field (R-REC-02).
- **Binary and unary operators** behave per the relevant operator file in `operators/`.
- **Function-call expressions** dispatch to expression-level function implementations.
- **Lambdas** evaluate to closure values that, when applied, evaluate their body with parameters bound in the closure's environment plus a fresh underscore-stack push if applicable.
- **List literals** evaluate to runtime list values; **record literals** to runtime record values.

## Final-result selection

After all bindings have run:

- If the program has a non-trivial selection expression (anything other than a bare step identifier), evaluate that expression against the environment. Convert: a list-typed result becomes a one-column table; any other result becomes a one-row one-column table.
- Otherwise, the table bound to the program's output step name is the result.

**Output.** A single result table value.

**Failure modes.** Execute error category. Codes E501 (DivisionByZero), E502 (OperandTypeMismatchAtRuntime), E503 (UnknownStepAtRuntime — defensive), E504 (NumericOverflow), E505 (IndexOutOfRange), E506 (UnknownField), E507 (CoercionFailure). Execute errors are single descriptive strings, not diagnostic records (R-ERR-01).

**Storage shape.** A mutable environment map alive only for the duration of execution. A thread-local stack for the underscore binding (R-EACH-04). Immutable table values flowing through; cloning is free, no in-place mutation (R-INV-09).

## Notes

- R-EXEC-N1: the workbook initialiser is currently a passthrough; full Excel parsing is a future extension.
- R-EXEC-N2: sheet navigation is a passthrough; multi-sheet workbooks require future work.


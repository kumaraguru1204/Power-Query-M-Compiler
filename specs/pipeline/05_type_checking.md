# Pipeline stage 5 — Type checking

**Input.** The resolved program tree and the input table.

**What it does.** Walks every binding in source order. Maintains a step-schema map keyed by step name and valued by ordered list of *(column name, column type)* pairs. For each binding:

1. **Bottom-up expression-type inference.** Walk every expression node recursively. For each kind, apply the rule from R-TYPE-01 through R-TYPE-12 plus the rules in `05_row_context_and_each.md`:
   - Literals: their obvious type.
   - Bare identifier in row context: column type or step type.
   - Bracket column access: column type.
   - Field access: looked up in the operand's record type.
   - Binary operation: per the operator family files (`operators/*.md`).
   - Unary operation: per `operators/unary.md`.
   - Function call (expression position): catalogue lookup, signature unification.
   - Lambda: Function type with the body's inferred type as inner type.
   - List literal: List-of-T where T is the joined element type.
   - Record literal: Record type built from the field types.
   Each computed type is written into the expression node's inferred-type slot in place.

2. **Step-level function-call validation.** For every generic function-call step, look up the function in the catalogue, find the signature whose arity matches, unify the supplied argument types against the signature's parameter types (R-TYPE-07). Lambda arguments are checked in the appropriate row context (R-EACH-09).

3. **Schema computation.** Invoke the catalogue entry's schema-transform hook with the input schema and the call arguments; if absent, propagate the input schema unchanged. Write the result into the step record's output-schema slot and into the step-schema map.

**Output.** The same program tree, mutated in place: every expression node carries its inferred type, every step record carries its output schema.

**Failure modes.** Diagnostics category. Codes E401 (OperandTypeMismatch), E402 (FunctionSignatureMismatch), E403 (UnificationFailure), E404 (UnknownColumnInExpression), E405 (LambdaReturnTypeMismatch), E406 (NonComparableTypes). All collected; pipeline halts after the pass.

**Storage shape.** Same tree as parsing produced; the inferred-type and output-schema slots are now populated. No new top-level structure.

## Catalogue contract

**R-CAT-01 — Argument hints drive parsing.** Already documented in `pipeline/03_parsing.md`.

**R-CAT-02 — Type signatures drive checking.** A signature is *(parameter types, return type)*. Type variables in the signature unify with concrete argument types per R-TYPE-07.

**R-CAT-03 — Optional and nullable parameters.** A signature may mark trailing parameters optional. An optional parameter that is absent at the call site contributes no constraint; an optional parameter that is supplied must satisfy its declared type. A nullable parameter accepts Null in addition to its declared type.

**R-CAT-04 — Schema-transform hook.** Receives the input schema and the call arguments, returns the output schema. May raise R-ERR-SCHEMA-CLASH (mapped to E402) if argument-derived columns collide with existing columns.


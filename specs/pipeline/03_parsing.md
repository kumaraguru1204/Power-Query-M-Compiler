# Pipeline stage 3 — Parsing

**Input.** The token list produced by lexing.

**What it does.** Implements the grammar in `cross_cutting/02_syntactic_grammar.md`. Two layers:

1. **Step-level parser.** Reads the *let* keyword, then a comma-separated list of named bindings, then the *in* keyword, then the selection clause. For each binding, dispatches the right-hand side per R-PARSE-03 into one of four step kinds (workbook initialiser, sheet navigation, generic function call, value binding). For generic function calls, looks up the function in the catalogue and uses its argument hints to drive per-argument parsing (R-PARSE-04).
2. **Expression-level parser.** Implements precedence-climbing with the operator hierarchy in R-PARSE-05. Recognises every primary expression form in R-PARSE-08 and the chaining rule for field access in R-PARSE-09.

The parser also handles:
- Lambda parsing — explicit (R-PARSE-10) and each-shorthand (R-PARSE-11), with the each-shorthand desugaring at parse time (R-EACH-01).
- List and record literals (R-PARSE-12, R-PARSE-13).
- Selection clause shape (R-PARSE-14).

**Output.** A program tree. Root: list of bindings, output name, output position marker, optional fully-parsed selection expression. Each binding: step name, step name's position marker, step record (kind, position marker, empty output-schema slot to be filled by the type-checker). Each step kind: per R-PARSE-03 plus the typed argument-union variants for generic function calls.

Every expression node carries a position marker and an empty inferred-type slot for the type-checker to populate (R-INV-10).

**Failure modes.** Parse error category. Sub-cases mapped to E201 (UnexpectedToken), E202 (MissingToken), E203 (UnknownFunction), E204 (MalformedArgument), E205 (DuplicateStepName), E206 (EmptyLetBlock).

The parser attempts limited recovery (skipping to the next comma or close-paren) so a single program can produce multiple parse diagnostics in one pass.

**Storage shape.** A tree of records. The expression nodes carry their inferred-type slot empty; later stages may write but no later stage may modify the parse-stage shape.


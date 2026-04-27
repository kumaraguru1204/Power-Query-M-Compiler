# Pipeline stage 6 — Formatting

**Input.** The validated, type-annotated program tree.

**What it does.** Recursively walks the tree and emits canonical M source text. The formatter is a pure function of the tree (R-INV-11); it does not consult the input table, the executor, or the SQL emitter.

1. **Program shape.** Emits *let*, a newline, the bindings written one per line at one indent level separated by commas, a newline, *in*, a newline, the selection expression at one indent level.
2. **Bindings.** Emits the step name, an equals sign with one space on each side, then the right-hand side. The right-hand side renders per its step kind:
   - Workbook initialiser: canonical *Excel.Workbook(File.Contents("…"), …)*.
   - Sheet navigation: canonical *step{[Item="…", Kind="…"]}[Data]*.
   - Generic function call: namespace-dot-name, open paren, comma-separated arguments, close paren. Each argument renders per its argument-union variant.
   - Value binding: the bound expression rendered per the expression rules below.
3. **Expressions.** Render with the minimum number of parentheses that preserves the parse tree. Operator precedence (R-PARSE-05) drives the parenthesisation decision: a sub-expression whose operator binds *less tightly* than its parent needs parentheses; otherwise no parens.
4. **Lambdas.** A one-parameter lambda whose parameter is the underscore renders as the each-shorthand: *each BODY*. Every other lambda renders explicitly: *(params) => body*.
5. **Lists and records.** Lists render as *{…}*; records as *[…]*. One element per line when the literal exceeds a width threshold; inline otherwise.

**Output.** A single string containing the cleanly re-formatted source.

**Failure modes.** None. The tree is known well-formed; formatting cannot fail.

**Storage shape.** Just a string.


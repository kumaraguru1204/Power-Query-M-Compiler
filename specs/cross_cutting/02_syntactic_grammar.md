# Cross-cutting: Syntactic Grammar

## Scope
Applies to the parsing stage. Cited by every family file (for argument shapes) and by the formatter (for round-trip rendering).

## Rules

**R-PARSE-01 — Program shape.** A program is the keyword *let*, then one or more named bindings separated by commas, then the keyword *in*, then a final selection expression. At least one binding is required. The selection expression is an arbitrary expression but is most commonly a bare identifier naming one of the bindings.

**R-PARSE-02 — Named binding.** A named binding is an identifier (the step name), then an equals sign, then a right-hand side. Step names are unique within a program; a duplicate step name is R-ERR-PARSE-DUP.

**R-PARSE-03 — Right-hand-side dispatch.** The parser inspects the right-hand side and chooses one of four step kinds:

- **Workbook initialiser** when it matches the recognised pattern *Excel.Workbook(File.Contents("…"), …)* with the further optional arguments described in F11.
- **Sheet navigation** when it matches *step-ref{[Item="…", Kind="…"]}[Data]*.
- **Generic function call** when it is a fully-qualified call (namespace dot name then parenthesised arguments) whose function exists in the catalogue.
- **Value binding** for everything else (literals, lists, records, lambdas, arithmetic expressions, non-catalogued function calls).

**R-PARSE-04 — Argument-hint dispatch.** For a generic function call, the parser reads the function's argument-hint list from the catalogue and parses each positional argument according to its hint variant. The hint variants are enumerated in `SOURCE_OF_TRUTH.md` §6.3 and §11 item 6.

**R-PARSE-05 — Operator precedence.** From lowest to highest binding strength: logical *or*; logical *and*; comparisons; additive (plus, minus, the concatenation ampersand); multiplicative (star, slash). All operators are left-associative.

**R-PARSE-06 — Comparison operators.** Equals, not-equals, less-than, less-than-or-equal, greater-than, greater-than-or-equal. They share one precedence level. Chained comparisons (such as *a < b < c*) parse left-associatively as *((a < b) < c)*; this is rarely useful and usually a type error caught later.

**R-PARSE-07 — Unary operators.** A leading minus or the keyword *not* binds tighter than any binary operator. A leading minus before a numeric literal is folded into a single negative literal; before any other primary it is a unary operation.

**R-PARSE-08 — Primary expressions.** A primary is one of: a numeric literal; a string literal; the *true* or *false* keyword; the *null* keyword; an identifier; a bracketed column access (a name in square brackets, used in row context); a function call (an identifier or namespace-dotted name followed by parenthesised arguments); an explicit lambda; an *each*-shorthand lambda; a list literal; a record literal; a parenthesised expression for grouping.

**R-PARSE-09 — Field access.** A primary followed by square brackets containing a field name is a field access. Field accesses chain: *r[a][b]* parses as field-access(field-access(r, a), b).

**R-PARSE-10 — Explicit lambda.** A parenthesised parameter list, then the fat-arrow token, then a body expression. Parameter names are bare identifiers separated by commas; an empty parameter list is permitted.

**R-PARSE-11 — Each-shorthand lambda.** The keyword *each* followed by a body. This desugars during parsing to an explicit one-parameter lambda whose parameter name is the underscore character.

**R-PARSE-12 — List literal.** Open-brace, comma-separated expressions, close-brace. The empty list is permitted.

**R-PARSE-13 — Record literal.** Open-bracket, comma-separated *name = expression* pairs, close-bracket. Field names are bare identifiers or quoted identifiers (R-LEX-08). Within one record literal a field name appears at most once.

**R-PARSE-14 — Selection clause.** The expression after *in*. If it is a bare identifier matching a step name, it is recorded as the program's output name. Otherwise it is parsed as a full expression and stored alongside the bindings; the executor evaluates it after running the bindings.

**R-PARSE-15 — Failure handling.** Any unexpected token, missing token, malformed argument, unknown function name in a generic-function-call position, or incoherent operator sequence is reported as a parse diagnostic (R-ERR-PARSE-NN). The pipeline halts after parse-stage diagnostics are gathered.

## Examples

The program *let x = 1 + 2, y = x * 3 in y* parses to two bindings (x = additive expression, y = multiplicative expression referencing x) and an output name y.

The program *let A = Table.SelectRows(Source, each [Age] > 30) in A* parses to two bindings (Source must be defined earlier in real code; here it would error at name-resolution time) and an output name A. The second binding's right-hand side is a generic function call whose second argument is parsed as an each-expression hint into a lambda body that compares the row's *Age* column to 30.

## Test coverage

Pointers will live under `conformance/cross_cutting/syntactic/`.

## Open questions

- R-PARSE-06: chained comparisons should arguably be a parse-time warning.
- R-PARSE-14: selection expressions other than bare identifiers are partially supported; tracked in PROJECT_REPORT.


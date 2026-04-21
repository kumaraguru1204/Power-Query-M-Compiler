# M Engine — Source of Truth

> **Status.** Authoritative specification of the project as it exists today.
>
> **Audience.** Anyone reading, extending, or maintaining the system, in any programming language.
>
> **Rule.** Every change to the codebase must agree with this document. If a change would contradict it, either the change is wrong, or this document must be updated in the same commit. The code follows this document; never the other way around.
>
> **Form.** This document is written in plain English. It contains no programming-language syntax, no type signatures, no field names with punctuation, and no code samples. Every concept is described in words. The intent is that if the entire source tree were deleted tomorrow, a competent engineer using only this document could re-implement an equivalent system in any programming language they choose.

---

## 1. What the System Is

The system is a small compiler and runtime for a subset of Microsoft Power Query's tabular transformation language (commonly known as the *M language*). Given two pieces of input — a small block of tabular data and a piece of M source code that transforms it — the system performs six end-to-end services:

1. It reads the source code and turns it into a structured representation it can analyse.
2. It validates that every name, every column reference, every type, and every function call mentioned in the source actually makes sense in context.
3. It evaluates the source code against the input data and produces a result table.
4. It translates the same source code into an equivalent SQL query.
5. It emits a cleanly re-formatted version of the original source.
6. It reports any problems it found back to the caller as rich, location-aware diagnostic records that can be rendered against the original source text with underlines and suggestions.

The system is exposed through three interchangeable entry points: a console binary used for local debugging, a command-line interface for scripted use, and an HTTP server that hosts a browser-based playground. All three entry points funnel into one shared compilation function so behaviour is identical regardless of how the system is invoked.

---

## 2. The Two Inputs and the Seven Outputs

### 2.1 Inputs

Every successful invocation of the system requires exactly two inputs:

1. **The workbook payload.** A textual document in the standard data-interchange format (JSON) carrying three fields:
   - a *source* string, naming the file the data came from (label only, never opened),
   - a *sheet* string, naming the worksheet within that file (label only),
   - a *rows* array. The rows array is a list of lists of plain text strings. The **first** inner list is treated as the column headers; every subsequent inner list is one data row, with positional alignment to the headers. Cell values are always plain text — type interpretation happens later.
2. **The formula text.** A string containing one M-language expression. The expression is required to be a let-block: a sequence of named bindings followed by a final selection clause that names which binding (or which expression) is the result.

When the formula is omitted, the system synthesises a default formula whose only effect is to expose the input data under a step named *Source*.

### 2.2 Outputs

A successful invocation returns seven artefacts bundled together:

1. **The input table** — the payload as it was parsed and typed (see §6.1).
2. **The result table** — the table produced by evaluating the formula against the input table.
3. **The cleaned formula text** — a canonical re-print of the formula with consistent indentation and spacing.
4. **The generated SQL** — a query string equivalent to the formula.
5. **The parsed program** — the structured representation of the formula, used by debugging tools.
6. **The token list** — a human-readable rendering of the lexer's output, used by debugging tools.
7. **A success indication** — a simple flag, plus an optional list of warnings.

A failed invocation returns one of five typed error categories (see §8), each carrying enough information for the caller to render a precise, location-anchored error message.

---

## 3. The High-Level Data Journey

The journey through the system is a strictly linear pipeline of nine stages. Each stage consumes the output of the previous stage and produces a new artefact. No stage is allowed to skip backwards or short-circuit forwards.

The stages, in order, are:

1. **Workbook ingestion** — turns the textual payload into a typed table of cells.
2. **Lexing** — turns the formula text into a flat sequence of tokens.
3. **Parsing** — turns the token sequence into a structured tree of bindings and expressions.
4. **Name resolution** — checks that every reference in the tree points to something that exists, and remembers what each named step produces.
5. **Type checking** — checks that every operator, expression, and function call is well-typed, and writes the inferred type onto every node of the tree.
6. **Formatting** — re-prints the now fully-validated tree as clean source text.
7. **Execution** — evaluates the tree against the input table and produces a result table.
8. **SQL emission** — translates the same tree into an equivalent SQL query string.
9. **Packaging** — gathers all artefacts into one bundle and returns it to the caller.

Stages 1 through 5 are *gating*: failure at any of them halts the pipeline and returns an error. Stages 6 through 8 are *productive*: by the time they run, the program is known to be well-formed, so they only fail under exceptional runtime conditions (for example, division by zero during execution).

---

## 4. The Repository Shape (in plain English)

The project is organised as a workspace of small, focused sub-projects (each one a folder under a *crates* directory), plus a top-level project that contains the entry-point binaries and a thin shared library.

The top-level project contains:

- A **console debug entry point** that holds a hard-coded payload and formula and prints every artefact to standard output. Used only by developers to iterate on a single formula in isolation.
- A **command-line entry point** that reads its inputs from arguments or files.
- A **web-server entry point** that hosts an HTTP service on a fixed local address, exposes three routes (a health check, a compile route, and a debug-mode compile route), and serves a folder of static files (the playground page, its scripts and assets).
- A **shared library** that defines the request and response shapes used by the command-line and web entry points, plus a single compile function that all entry points call.
- A **public assets folder** holding the playground page and its visual assets.
- This document, an interface guide for end users, and a living development log.

Beneath the workspace, thirteen sub-projects each carry one responsibility:

| Position | Sub-project        | Responsibility                                                                                                    |
| -------- | ------------------ | ----------------------------------------------------------------------------------------------------------------- |
| 1        | diagnostics        | The error model: source-position markers, diagnostic records, and the human-readable renderer.                    |
| 2        | types              | The data-type vocabulary, type inference for raw text columns, and rules for coercing raw text into typed values. |
| 3        | grammar            | Static knowledge of the M language: keywords, operators, the catalogue of supported functions, and the type algebra used by the type-checker. |
| 4        | lexer              | Turns raw source text into a flat list of tokens carrying position markers.                                       |
| 5        | abstract syntax    | The shape of a parsed program: expressions, named step bindings, programs, and the typed argument union used by step-level function calls. |
| 6        | pipeline           | The runtime data model: typed tables and their columns, plus the loader that turns the workbook payload into a typed table. |
| 7        | parser             | Turns a token list into the abstract syntax tree, consulting the grammar's function catalogue to know how to read each function's arguments. |
| 8        | resolver           | Walks the tree and verifies that every step name and every column name exists in scope at the point it is used.   |
| 9        | type-checker       | Walks the tree and verifies that every operator, expression, and function call is well-typed; annotates every node with its inferred type. |
| 10       | executor           | Evaluates the validated tree against the input table and produces the result table.                               |
| 11       | sql emitter        | Translates the validated tree into an equivalent SQL query string.                                                |
| 12       | formatter          | Re-prints either a parsed program back to clean source or a typed table to a human-friendly preview.              |
| 13       | engine             | The orchestrator: glues all of the above into one pipeline and exposes the single public entry point.             |

Sub-projects sit in a strict dependency stack: each one may depend only on those listed above it. The diagnostics sub-project depends on nothing else in the workspace; the engine sub-project depends on every other one.

---

## 5. The Cross-Cutting Building Blocks

Five concepts appear across multiple stages. Defining them once here keeps every stage description short.

### 5.1 The position marker

Every meaningful element produced anywhere in the pipeline carries a **position marker**. A position marker records five numbers about a region of the original source text:

- the absolute character offset where the region starts,
- the absolute character offset where the region ends,
- the line number where the region starts (counting from one),
- the column number on that line where the region starts (counting from one),
- the length of the region in characters (derivable from start and end, but cached for convenience).

There is also a designated **dummy** position marker used for elements that have no corresponding source location (synthesised values, for example).

Position markers are the foundation of every error message: they let the renderer pinpoint exactly which characters of the input the user must look at.

### 5.2 The diagnostic record

A **diagnostic** is a structured error/warning/hint. It carries:

- a short stable identifier (a code such as the letter E followed by three digits), used to recognise the same kind of problem across runs;
- a primary message in plain English;
- a severity (error, warning, or hint);
- zero or more **labels**, each of which pairs a position marker with a small message ("expected here", "started here", and so on);
- an optional suggestion string ("did you mean such-and-such?").

Diagnostics are how every stage reports problems. The system never throws plain strings as errors out of a pipeline stage. Plain-string error messages are confined to the boundaries (the renderer that prints diagnostics to a console, or the web layer that flattens them to JSON).

The diagnostics sub-project also owns a **renderer**: given the original source text and a list of diagnostics, it produces a multi-line human-readable string that includes the offending source line, an underline pointing at the labelled region, and the suggestion text.

### 5.3 The type vocabulary

The type-vocabulary sub-project owns the universe of types the system understands. It includes scalar types (Integer, Float, Boolean, Text, Date, DateTime, DateTimeZone, Duration, Time, Currency, Binary, Null), composite types (a homogeneous List of some inner type, a Function whose return type is some inner type), and the rules used to translate to and from M-language type names (such as the textual form *Int64.Type* corresponding to Integer).

It also owns two helpers:

- a **column-type-inference** routine that, given the raw text values of a column, returns the most specific type all of them satisfy: if every value parses as an integer, the column is Integer; otherwise if every value parses as a floating-point number, the column is Float; otherwise if every value is the literal text *true* or *false*, the column is Boolean; otherwise the column is Text. An empty column defaults to Text.
- a **coercion** routine that converts a single raw text value into the typed runtime value implied by the column's inferred type.

### 5.4 The function catalogue

The grammar sub-project holds a **function catalogue** as static data, organised into namespaces (Excel, Table, Tables, List, Text, Number, Logical). Each catalogue entry describes one M function and records, in a uniform shape:

- the function's bare name (for example, *SelectRows*) and its full namespaced name (for example, *Table.SelectRows*);
- an ordered list of **argument hints** — one hint per positional parameter, telling the parser how to read that argument from the token stream (for example, "this is a step reference", "this is a column-name list", "this is an expression that becomes a lambda body", "this is an optional nullable Boolean");
- one or more **type signatures** — each describing one acceptable arity and the parameter and return types using the type algebra (which supports type variables, function types, list types, optional parameters, and nullable parameters);
- an optional **schema-transform hook** — a procedure that, given the input table's schema and the call-site arguments, computes the output table's schema (used for functions whose output columns depend on what the caller passed in);
- a documentation string.

This catalogue is the *single source of truth* for which M functions exist and how they behave at the syntactic and type level. The parser reads the argument hints; the type-checker reads the signatures; the executor and SQL emitter look up the entry by name and dispatch to their per-function implementations.

### 5.5 The runtime value vocabulary

The executor owns its own **value vocabulary**, distinct from the type vocabulary above. Where the type vocabulary describes *static* types known at compile time, the value vocabulary describes the *runtime* shapes that values actually take during evaluation: an integer, a floating-point number, a boolean, a piece of text, a null, a list of values, or a record (an ordered mapping from field name to value).

Cell values from the input table are coerced from their stored raw text form into runtime values on demand, using the column's inferred type to pick the right coercion. The runtime value vocabulary is private to the executor; no other sub-project reasons in terms of runtime values.

---

## 6. Stage-by-Stage Data Transformation

This is the heart of the document. Each subsection follows the same five-part shape:

- **Input** — what the stage receives.
- **What it does** — the transformation, in algorithmic prose.
- **Output** — what the stage produces.
- **Failure modes** — how it can go wrong and what diagnostic it reports.
- **Storage shape** — how the produced data is held in memory.

### 6.1 Workbook ingestion

**Input.** The workbook payload as a single string in the standard data-interchange format.

**What it does.**

1. Parses the payload string into a three-field record (source name, sheet name, list of rows of strings). Any failure here aborts the pipeline.
2. Splits the rows: the first row becomes the **column-header list**; the remainder become the **data rows**.
3. Creates one column per header, each initially holding an empty list of raw text values and a placeholder type of Text.
4. Walks the data rows, appending each cell value to the matching column by position. If a data row has more cells than there are headers, the extra cells are silently dropped (matching the behaviour of spreadsheet importers).
5. For each column, runs the type-inference routine (§5.3) on its raw values and replaces the placeholder type with the inferred type.

**Output.** A single typed table value carrying the source name, sheet name, and an ordered list of typed columns. Each column carries its name, its inferred type, and its raw text values (still untyped on a per-cell basis).

**Failure modes.** A single failure mode: malformed payload (this becomes the *Json* error category, §8).

**Storage shape.** A record with three fields: source name, sheet name, and a list of column records. A column record has three fields: name, inferred type, and a list of raw text values. The cells are *not* converted to typed values yet; they remain as text, and consumers coerce on demand.

### 6.2 Lexing

**Input.** The formula text as a single string.

**What it does.** Walks the formula one character at a time, maintaining a position counter (offset, line, and column). At each step:

1. Skips whitespace.
2. Looks at the next character to decide what kind of token starts here:
   - A double-quote opens a **string literal**. Characters are accumulated until a matching double-quote closes the literal. An unterminated string at end-of-input is an error.
   - A digit opens a **numeric literal**. Digits and at most one decimal point are accumulated. Zero decimal points means an integer literal; one decimal point means a floating-point literal; more is an invalid-number error.
   - A letter or underscore opens an **identifier**. Letters, digits, and underscores are accumulated until a non-identifier character. The accumulated text is then looked up against the keyword table from the grammar sub-project: if it matches *let*, *in*, *each*, *and*, *or*, *not*, *true*, *false*, or *null*, the corresponding keyword or literal token is produced; otherwise it becomes a generic identifier token.
   - A hash followed immediately by a double-quote opens a **quoted identifier** (used by M to allow spaces in names): characters between the quotes become an identifier token.
   - One of the dedicated punctuation characters (period, comma, parentheses, braces, brackets, plus, minus, star, slash, ampersand) becomes the corresponding single-character token.
   - The character *equals* may be either a comparison-equals token or, if followed by *greater-than*, a fat-arrow token used for explicit lambdas.
   - The character *greater-than* may be either greater-than or, if followed by *equals*, greater-than-or-equal.
   - The character *less-than* may be less-than, less-than-or-equal, or, if followed by *greater-than*, the not-equal token.
   - Anything else is an unexpected-character error.
3. Records the accumulated token together with a position marker covering the region it occupied.

After the input is exhausted, an end-of-input token is appended.

**Output.** A flat list of tokens, each carrying a kind (the categories above) and a position marker.

**Failure modes.** Three kinds: unterminated string literal, unexpected character, invalid numeric literal. Each is converted to a diagnostic with the offending region highlighted, and the pipeline halts (the *Lex* error category, §8).

**Storage shape.** A list of token records. A token record is a kind plus a position marker. The token kind is one of a fixed enumeration that covers literals (string, integer, float, boolean, null), identifiers, keywords, comparison operators, arithmetic operators, the concatenation operator, the dot, the comma, the fat-arrow, the four kinds of brackets, and the end-of-input marker.

### 6.3 Parsing

**Input.** The token list produced by lexing.

**What it does.** Builds a structured tree representing the program. The grammar of the language being parsed is:

- A **program** is the keyword *let*, followed by one or more named bindings separated by commas, followed by the keyword *in*, followed by a final selection expression. The selection expression is most commonly a bare identifier naming one of the bindings; when it is anything else (a function call, an arithmetic expression, an if-then-else), the parser still builds the full expression and stores it alongside the program.
- A **named binding** is an identifier (the step name), followed by an equals sign, followed by either:
  - a **special-shape** right-hand side (the workbook initialiser, recognised as the function *Excel.Workbook* applied to *File.Contents* of a string and optional flags; or the workbook navigation pattern, recognised as a step name followed by braces containing a record then square brackets containing the field name *Data*), each of which produces a dedicated kind of step; or
  - a **fully-qualified function call** (a namespace identifier, a dot, a function name, then arguments in parentheses), which is looked up in the function catalogue and parsed argument-by-argument according to its argument hints, producing a generic function-call step; or
  - **anything else** (a literal, a list, a record, a lambda, an arithmetic expression, a non-catalogued function call), which produces a value-binding step that holds the raw expression.
- An **expression** follows standard infix conventions. A precedence-climbing routine handles the operator hierarchy (from lowest to highest binding strength): logical *or*; logical *and*; comparisons (equals, not-equals, greater-than, less-than, greater-than-or-equal, less-than-or-equal); additive (plus, minus, the concatenation ampersand); multiplicative (star, slash). All operators are left-associative. Below the operators sit the primary expressions: numeric, string, boolean, and null literals; identifiers; bracketed column-access (a name in square brackets, used in row context); explicit field access on a named variable (a primary followed by square brackets containing a field name, possibly chained); function calls (an identifier or namespace-dotted name followed by parenthesised arguments); explicit lambdas (parenthesised parameter list, fat-arrow, body); the each-shorthand lambda (the keyword *each* followed by a body, which desugars internally to a one-parameter explicit lambda whose parameter is the underscore character); list literals (braces around comma-separated expressions); record literals (square brackets around comma-separated *name = expression* pairs); and parenthesised expressions for grouping.
- Argument-shape parsing for catalogued function calls is driven by the **argument hints** stored on the catalogue entry. The hint tells the parser whether to expect a step reference (a bare identifier), a string literal, an integer, a column-name list (braced strings), a rename list (braced pairs of strings), a type list (braced *string-type* pairs), a sort list (braced pairs of column name and order keyword), an aggregation list (braced descriptors of named aggregates), a transform list (braced pairs of column name and lambda), a join-kind keyword, an optional nullable boolean, an optional integer, an optional missing-field policy, an optional culture-or-options record, a bare type list (braced unadorned types), or a generic expression. Each such hint produces a dedicated variant of the argument union.

**Output.** A program tree. The tree's root holds a list of named bindings, the bare name of the output binding, the position marker of that name, and an optional fully-parsed selection expression (present only when the selection clause is more than a bare identifier). Each named binding holds the step name, the position marker of the name, and a **step record** (kind, position marker, and an empty slot for the output schema that the type-checker will fill in later). Each step kind is one of four cases: the workbook initialiser (carrying its file path and optional flags); the workbook sheet navigation (carrying the input step name, the *Item* string, the *Kind* string, and the field name); a value binding (carrying one expression node); or a generic function call (carrying the fully-qualified function name and a list of typed argument-union values).

**Failure modes.** Any unexpected token, missing token, malformed argument, unknown function, or incoherent operator sequence is reported as a parse diagnostic and halts the pipeline (the *Parse* error category, §8).

**Storage shape.** A tree of records. The expression nodes additionally carry an empty slot for the inferred type that the type-checker will fill in later — a position the type-checker may write to but no other later stage may modify.

### 6.4 Name resolution

**Input.** The program tree produced by parsing, plus the input table.

**What it does.** Walks every binding in source order, maintaining two pieces of state:

- a **scope** — the set of step names defined so far;
- a **step-schema map** — for each step name defined so far, the list of column names that step's output table is currently known to contain.

For each binding, it:

1. Records the step's name into the scope.
2. Walks the step's contents looking for references that need validation: each step reference is checked against the scope, each column reference (whether explicit bracket access or a bare identifier used in row context where the row's columns are visible) is checked against the appropriate schema (the previous step's output schema, looked up in the step-schema map). Unknown names produce diagnostics with a "did you mean?" suggestion based on the closest existing name as measured by edit distance (with a small distance threshold so wildly different names produce no suggestion).
3. Computes the step's output schema (for the workbook initialiser, this is the input table's columns; for sheet navigation, the same; for catalogued function calls, the function's schema-transform hook from the catalogue is invoked if present, otherwise the schema is propagated unchanged; for value bindings, the schema is empty since the binding does not produce a table). The computed schema is recorded into the step-schema map under the step's name.

**Output.** The same program tree, unchanged in shape, but with confirmation that all references resolve. Returned alongside is a fully populated step-schema map describing, for every step, the column names that step is known to produce. (The map itself is not propagated explicitly to later stages because the type-checker re-derives it; resolution exists primarily to *fail fast* on unknown names with a precise suggestion.)

**Failure modes.** Any unknown step name or unknown column name produces a diagnostic. All diagnostics collected during the walk are returned together (resolution does not stop on the first error, so the user sees all name problems at once); if any are present, the pipeline halts (the *Diagnostics* error category, §8).

**Storage shape.** A scope set holding only step names. A step-schema map keyed by step name, valued by ordered list of column names.

### 6.5 Type checking

**Input.** The (resolved) program tree and the input table.

**What it does.** Walks every binding in source order, maintaining a step-schema map keyed by step name and valued by ordered list of *(column name, column type)* pairs. For each binding, it:

1. **Infers expression types bottom-up.** Each expression node is examined recursively, and a single column-type is computed for it according to a fixed rule set:
   - Literals have their obvious type (an integer literal is Integer, a float literal is Float, a boolean literal is Boolean, a string literal is Text, the null literal is Null).
   - A bracket column access has the type of the named column in the surrounding row context; an unknown column is a type error.
   - A bare identifier in row context resolves either to a column (and gets that column's type), to a step name (and gets a Function-of-Table type — rare in expression position), or to the implicit underscore parameter inside an each-shorthand (which carries the row-record type of the surrounding context).
   - A field access on an identifier picks the named field's type out of the identifier's record type.
   - A binary operation has its result type determined by the operator family: arithmetic operators require both operands to be numeric and produce numeric (Integer if both are Integer, Float otherwise); the concatenation operator requires both operands to be Text and produces Text; comparison operators require both operands to be comparable to each other and produce Boolean; logical operators require Boolean operands and produce Boolean. Operand-type mismatches are diagnostics.
   - A unary *not* requires Boolean; a unary minus requires numeric.
   - A function call (in expression position) is looked up in the function catalogue; the matching arity signature is selected; argument types are unified against the signature's parameter types using the type algebra; the signature's return type (after substitution of any type variables that unification bound) is the result.
   - A lambda has a Function type whose inner type is the body's inferred type.
   - A list literal has type List-of-T, where T is the type that all element types share (an empty list has element type Null).
   - A record literal has a Record type built from its fields.
   - As each expression node is computed, its type-slot is written in place. Subsequent stages read this slot.
2. **Validates step-level function calls.** The function catalogue is consulted for the function name; the matching arity signature is found; each call argument's contribution is type-checked according to that signature. Lambda arguments are checked in the **row context** of the appropriate input table: for example, the lambda passed to a row-filtering function is checked with column names from the input table's schema in scope.
3. **Computes the step's output schema** by invoking the catalogue entry's schema-transform hook (if present) with the input schema and the call arguments, or by propagating the input schema unchanged (if absent). The result is written into the step record's output-schema slot and into the step-schema map.

**Output.** The same program tree, mutated in place: every expression node now carries its inferred type, and every step record now carries its output schema.

**Failure modes.** Operator-type mismatches, function-signature mismatches, unknown columns in expression position, and any other type incoherence become diagnostics. All diagnostics are collected; if any are present, the pipeline halts (the *Diagnostics* error category, §8).

**Storage shape.** Same tree as parsing produced, but with the inferred-type and output-schema slots populated. No new top-level structure is created.

### 6.6 Formatting

**Input.** The (validated, type-annotated) program tree.

**What it does.** Produces a canonical textual representation by recursively walking the tree:

1. The program text is *let*, a newline, the bindings written one per line with consistent indentation and separated by commas, a newline, *in*, a newline, an indented selection expression.
2. Each binding is written as the step name, an equals sign with a single space on either side, then the right-hand side. The right-hand side is rendered according to the step kind: the workbook initialiser is rendered with its canonical argument list; sheet navigation is rendered with its canonical brackets; function calls are rendered as namespace-dot-name followed by parenthesised arguments where each argument is rendered by its argument-union variant (string literals are quoted, column lists are braced and quoted, lambda bodies are written as *each* followed by the body, and so on).
3. Expressions are rendered following the operator-precedence rules with the minimum number of parentheses that preserves the parse tree. Lambdas with the implicit underscore parameter are rendered using the *each* shorthand; explicit lambdas are rendered with their parenthesised parameter list and fat-arrow.

**Output.** A single string containing the cleanly re-formatted source.

**Failure modes.** None: the tree is already known to be valid.

**Storage shape.** Just a string.

### 6.7 Execution

**Input.** The (validated, type-annotated) program tree and the input table.

**What it does.** Maintains an **environment**: a mapping from step name to the table value bound to that step. It then walks the bindings in source order. For each binding:

1. **The workbook-initialiser kind** materialises the input table directly: the result is the input table value, cloned and re-tagged with the path string from the call (the actual file is never read).
2. **The sheet-navigation kind** looks up the named input step in the environment and returns its table cloned (sheet selection by name is not currently consulted, since the input table is what was loaded from the payload).
3. **The value-binding kind** evaluates the bound expression once. If the result is a list of values it becomes a one-column table whose column type is inferred from the values; otherwise it becomes a one-row, one-column table with the inferred type of the single value.
4. **The generic function-call kind** dispatches by the fully-qualified function name to a per-function evaluator. Each per-function evaluator reads its arguments out of the typed argument-union variants (extracting step references, column lists, lambdas, and so on by their kind) and produces an output table.

Inside per-function evaluators, expressions are evaluated against the appropriate row context using the runtime value vocabulary (§5.5). The evaluation rules are:

- Literals evaluate to the obvious runtime value.
- Bracket column access in row context fetches the column's raw text value at the current row index and coerces it according to the column's inferred type.
- A bare identifier may resolve to: the implicit underscore (in row context, this is the current row record; in list-element context, this is the current element value); a step name (the bound table); or a column in the current row (treated like bracket access).
- A field access on a value first evaluates the inner value to a record, then looks up the named field.
- Binary operators do the arithmetic, comparison, concatenation, or logical work expected by their kind; numeric operands are widened to Float if either side is Float; division by zero yields a runtime error.
- Unary operators do the obvious negation.
- Function-call expressions dispatch to expression-level function implementations (text length, number-from, list-sum, and so on).
- Lambdas evaluate to closure values that, when applied, evaluate their body with their parameters bound; the each-shorthand desugars to a one-parameter lambda whose parameter is the implicit underscore.
- List literals evaluate to runtime list values; record literals evaluate to runtime record values.

The implicit underscore deserves special note: because it can mean either "the current row" (in table contexts) or "the current element" (in list contexts), the executor maintains a small stack of underscore bindings so nested contexts work correctly. When code references the underscore, the most recently pushed binding is used.

After all bindings have run, the executor selects the final result. If the program has a non-trivial selection expression (anything other than a bare step identifier), that expression is evaluated against the environment and its result is converted to a table (a list-typed result becomes a one-column table; any other result becomes a one-row one-column table). Otherwise, the named output step's bound table is the result.

**Output.** A single result table value.

**Failure modes.** Three kinds of runtime errors are recognised: unknown step (defensive — should never occur after a successful resolve), division by zero, and type mismatch (a value used in a context that demands a different runtime kind). Each becomes the *Execute* error category (§8). The executor does not produce diagnostic records; it produces single-message errors.

**Storage shape.** A mutable environment map alive only for the duration of execution; a thread-local stack for the underscore binding; immutable table values flowing through. Tables are cloned freely; nothing is mutated in place.

### 6.8 SQL emission

**Input.** The (validated, type-annotated) program tree and the input table.

**What it does.** Produces a single SQL query string by translating the program. Each step becomes a common-table-expression (a named subquery) named after the step. The workbook initialiser becomes a literal-values subquery built from the input table's data. Each subsequent step is translated by dispatching on its kind to a per-function emitter, which produces a subquery referencing the previous step's name. The final query selects from the output step's subquery.

Per-function emitters know how to lower the common patterns: a row-selection function becomes a WHERE clause; an add-column function becomes an additional projected expression; a sort function becomes an ORDER BY clause; a column-rename function becomes aliased projections; a column-removal function becomes a narrower projection list; type transforms become CAST expressions; and so on. Lambda bodies are lowered into SQL expressions by walking the same operator and column-reference rules used during execution.

**Output.** A single string containing the SQL query.

**Failure modes.** SQL emission is best-effort: when a function has no SQL lowering, the output may include an *unsupported* placeholder. Emission never aborts the pipeline.

**Storage shape.** Just a string.

### 6.9 Packaging

The seven artefacts (input table, result table, cleaned formula, SQL string, parsed program, token list, success indication) are gathered into one bundle and returned to the caller.

---

## 7. Entry Points and How They Share the Pipeline

The system supports three entry-point binaries, all of which behave identically because they all funnel through the same shared compile function.

### 7.1 The console debug entry point

A small program that holds a hard-coded workbook payload string and a hard-coded formula string in its source code. When run, it prints the payload, runs the engine, then prints the token list, the parsed program (in a debug-formatted dump), the cleaned formula, the result table (in a human-readable preview), and the generated SQL. On error it prints the rendered diagnostic instead. Used only by developers iterating on a single formula.

### 7.2 The command-line entry point

A binary that reads its inputs from arguments or files, calls the shared compile function, and prints the response. Behaviour is identical to the web entry point because both use the same shared function.

### 7.3 The web-server entry point

A binary that hosts an HTTP server on a fixed local address (port 8080). It exposes three routes:

- a **health route** that returns a simple status record indicating the service is running, together with a version label;
- a **compile route** that accepts a request containing a formula and an optional payload and returns a response with success indication, result preview text, list of detailed errors, list of warnings, formatted code, generated SQL, and (only when debug mode is requested) the token list and parsed-program dump;
- a **debug-compile route** that does the same as the compile route but always enables debug mode.

It also serves a folder of static files (the playground page and its assets) at the root path, so a browser visiting the address loads the playground HTML, which uses the compile route to evaluate user input.

### 7.4 The shared compile function

All three entry points call a single function in the top-level shared library. That function:

1. Substitutes a small default payload if the caller supplied none (so an empty request still produces a meaningful response).
2. Calls the engine's main entry point with the payload and formula.
3. On success, builds a response carrying the rendered result table, the cleaned formula, and the generated SQL. If debug mode is enabled, also includes the token list and a debug dump of the parsed program.
4. On failure, walks the diagnostics and produces a list of detailed error records, each carrying the message, the line number, the column number, the length of the highlighted region, and the source line text — exactly what a UI needs to highlight the offending span.

---

## 8. Error Model and Propagation

The system has exactly five error categories, one per pipeline stage that can fail:

| Category    | Originating stage                              | Carries                                              |
| ----------- | ---------------------------------------------- | ---------------------------------------------------- |
| Json        | Workbook ingestion                             | A single descriptive string                          |
| Lex         | Lexing                                         | A list of diagnostic records                         |
| Parse       | Parsing                                        | A list of diagnostic records                         |
| Diagnostics | Name resolution or type checking               | A list of diagnostic records                         |
| Execute     | Execution                                      | A single descriptive string                          |

For any category that carries diagnostics, the engine's renderer pretty-prints each one against the original source text, including the source line, an underline pointing at the labelled region, and the suggestion text. Diagnostics never leave the engine as plain strings; conversion to user-visible text happens only at the boundary (the renderer for console output, or the web layer's flattening for JSON output).

---

## 9. Invariants the System Must Always Honour

These rules are the contract. A change that breaks any of them is a regression, regardless of whether tests pass.

1. **One pipeline.** Every entry point reaches the same compile function, which reaches the same nine-stage pipeline. There is no second pathway that bypasses any stage.
2. **Order is fixed.** Workbook ingestion → lex → parse → resolve → type-check → format → execute → SQL emit → package. No stage is skipped, no stage runs out of order. A later stage is not allowed to "fix up" what an earlier stage got wrong.
3. **Position markers are mandatory.** Every token, every node of the parsed tree, every diagnostic label carries a position marker (real or dummy). No silent discarding.
4. **Diagnostics are typed records.** Any error originating inside the pipeline is a diagnostic record, never a plain string. Plain-string conversion happens only at the renderer or web-layer boundary.
5. **Function knowledge is data.** Adding or changing an M function means editing the function catalogue plus the per-function entries in the executor and the SQL emitter, and nothing else. Bespoke parser branches per function are a design smell — the catalogue's argument-hint vocabulary should grow to absorb any new shape.
6. **The grammar sub-project has no upward dependencies.** It is the dictionary; the rest of the system reads from it. Nothing in the grammar sub-project may know about the executor, the SQL emitter, or the engine.
7. **The diagnostics sub-project has no horizontal dependencies.** It depends on nothing else in the workspace. It is the alphabet for all errors.
8. **The runtime value vocabulary is private.** Only the executor reasons in runtime values. No other sub-project sees them.
9. **Tables are immutable in transit.** Each step produces a fresh table. No step mutates its input table in place.
10. **Type annotations, once written, are trusted.** The type-checker is the only writer of inferred-type and output-schema slots. Every later stage reads them and trusts them; no later stage re-derives types.
11. **The formatter is a pure function of the parsed tree.** It does not consult the input table, the executor, or the SQL emitter.
12. **The default payload is for fallback only.** Production callers always supply a real payload. The default exists so that an empty request still returns a meaningful response for testing.
13. **Behaviour is identical across entry points.** A bug visible only in the web entry point but not in the console entry point lies in the entry-point glue, not in the engine.

---

## 10. How to Use This Document

- **Before any change**, locate the section that covers the area being touched. If the change conflicts with what is written, decide whether the document is wrong (then update it as part of the change) or the design is wrong (then revise the design).
- **When adding a new M function**, follow §5.4 and §9 rule 5: one new entry in the function catalogue, plus per-function entries in the executor and the SQL emitter.
- **When adding a new pipeline stage**, place it in the strict order of §3 and re-check §9 rules 1 through 4.
- **When adding a new entry-point binary**, route it through the shared compile function (§7.4) and confirm it satisfies §9 rule 13.
- **When writing per-function specifications** (planned for a future *specs* directory, one file per M function), describe inputs, outputs, type signatures, edge cases, and SQL lowering for that one function. This document is the bird's-eye view; the per-function specifications are the close-ups.

---

## 11. Reconstruction Checklist

If the entire source tree were deleted, a competent engineer using only this document should be able to rebuild an equivalent system by completing, in order, every item below. Each item names a deliverable, not an implementation choice — the engineer chooses the language, the data structures, and the tools.

1. **Position-marker model.** A record holding start offset, end offset, line, column, and length, plus a designated dummy value.
2. **Diagnostic model.** A record holding code, message, severity (error, warning, hint), labels (each a position marker plus message), and an optional suggestion. Plus a renderer that, given source text and a list of diagnostics, prints them with source-context underlines.
3. **Type vocabulary.** The enumeration of types from §5.3, the back-and-forth mapping with M-language type names, the comparable and numeric predicates, the column-type-inference routine, and the raw-text-to-runtime-value coercion routine.
4. **Function catalogue.** The static catalogue described in §5.4, organised into the seven namespaces (Excel, Table, Tables, List, Text, Number, Logical), with each entry carrying name, argument-hint list, type signatures, optional schema-transform hook, and documentation.
5. **Token model and lexer.** The token-kind enumeration from §6.2 and the lexer state machine described there.
6. **Abstract syntax model.** The expression kinds from §6.3 (literals, identifier, bracket column access, field access, binary operation, unary operation, function call, lambda, list literal, record literal), the four step kinds (workbook initialiser, sheet navigation, value binding, generic function call), the typed argument-union variants from §6.3 (step reference, step-reference list, expression, string, column list, rename list, type list, sort list, aggregate list, transform list, integer, optional integer, nullable boolean, optional missing-field policy, optional culture-or-options record, bare type list, join kind), and the program record (binding list, output name, output position marker, optional fully-parsed selection expression).
7. **Workbook ingestion.** A loader that performs the five steps in §6.1 and produces a typed table value (source name, sheet name, list of typed columns; each column carries name, inferred type, raw text values).
8. **Parser.** A parser that produces a program tree per §6.3, consulting the function catalogue's argument hints to drive per-argument parsing for catalogued function calls.
9. **Name resolver.** A walker that produces the scope and step-schema map per §6.4, emitting diagnostics with edit-distance suggestions.
10. **Type-checker.** A walker that, per §6.5, infers expression types bottom-up, validates function-call arguments using the catalogue's type signatures and the unification rules of the type algebra, computes step output schemas, and writes inferred types and schemas back into the tree in place.
11. **Formatter.** A pure printer that re-emits the validated tree as canonical source per §6.6.
12. **Executor.** A walker that maintains the environment, evaluates each binding's expressions using the runtime value vocabulary, dispatches catalogued function calls to per-function evaluators, manages the implicit-underscore stack for nested contexts, and produces the final result table per §6.7.
13. **SQL emitter.** A walker that produces named-subquery SQL per §6.8, with per-function lowerings and a graceful unsupported-placeholder fallback.
14. **Engine orchestrator.** A single function that accepts a workbook payload and a formula string and runs all nine stages in order, returning either the seven-artefact bundle or one of the five error categories.
15. **Shared compile function.** The boundary function described in §7.4: substitutes a default payload if absent, calls the engine, builds the response or detailed-error list, and returns it.
16. **Entry-point binaries.** A console debug entry point with hard-coded inputs (§7.1); a command-line entry point that reads from arguments or files (§7.2); a web-server entry point exposing the three routes and serving the static playground (§7.3). All three call the shared compile function.
17. **Invariant enforcement.** A test suite that exercises each of the thirteen invariants in §9, so future regressions trip a test rather than escape into production.

A reconstruction that satisfies all seventeen items of this checklist, while honouring every invariant in §9, is by definition equivalent to the system this document describes.

---

*End of Source of Truth. Keep this file truthful.*


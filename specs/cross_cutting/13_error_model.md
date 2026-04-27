# Cross-cutting: Error Model

## Scope
Applies to every stage. This file enumerates every error code the system raises.

## Rules

**R-ERR-01 — Five categories.** Errors fall into one of five typed categories per `SOURCE_OF_TRUTH.md` §8: Json (workbook ingestion), Lex, Parse, Diagnostics (resolver and type-checker), Execute. The first four propagate as either single descriptive strings (Json, Execute) or as lists of structured diagnostic records (Lex, Parse, Diagnostics).

**R-ERR-02 — Diagnostic record shape.** Every diagnostic carries a stable code, a primary message, a severity (error / warning / hint), zero or more labels (each a position marker plus message), and an optional suggestion string (R-DIAG in `SOURCE_OF_TRUTH.md` §5.2).

**R-ERR-03 — Code namespace.** Codes are uppercase letters followed by three digits. The prefix denotes the originating area:
- *E1xx* — lexical;
- *E2xx* — parsing;
- *E3xx* — name resolution;
- *E4xx* — type checking;
- *E5xx* — execution;
- *Wxxx* — warnings.

## Catalogue

### Lexical (E1xx)

| Code  | Name                          | Trigger                                                        |
| ----- | ----------------------------- | -------------------------------------------------------------- |
| E101  | UnterminatedString            | A string literal reaches end-of-input with no closing quote.   |
| E102  | UnexpectedCharacter           | A character that begins no token kind.                         |
| E103  | InvalidNumber                 | A numeric literal with two or more decimal points or out of range. |
| E104  | UnsupportedComment            | A comment-like sequence (planned).                             |

### Parsing (E2xx)

| Code  | Name                          | Trigger                                                        |
| ----- | ----------------------------- | -------------------------------------------------------------- |
| E201  | UnexpectedToken               | The current token cannot start the current production.         |
| E202  | MissingToken                  | A required token is absent.                                    |
| E203  | UnknownFunction               | A namespace-dotted call whose function is not in the catalogue.|
| E204  | MalformedArgument             | An argument cannot be parsed under its declared hint.          |
| E205  | DuplicateStepName             | Two bindings in the same let-block share a name.               |
| E206  | EmptyLetBlock                 | A let-block with no bindings.                                  |

### Name resolution (E3xx)

| Code  | Name                          | Trigger                                                        |
| ----- | ----------------------------- | -------------------------------------------------------------- |
| E301  | UnknownStep                   | A reference to a step name not defined earlier in the program. |
| E302  | UnknownColumn                 | A reference to a column name not in the appropriate schema.    |

### Type checking (E4xx)

| Code  | Name                          | Trigger                                                        |
| ----- | ----------------------------- | -------------------------------------------------------------- |
| E401  | OperandTypeMismatch           | A binary or unary operator with operands of incompatible types.|
| E402  | FunctionSignatureMismatch     | A call where no signature accepts the supplied arity and types.|
| E403  | UnificationFailure            | A type variable bound to two incompatible types.               |
| E404  | UnknownColumnInExpression     | A column reference inside an expression whose row context lacks it. |
| E405  | LambdaReturnTypeMismatch      | A lambda whose body returns the wrong type for its position (predicate must be Boolean, etc.). |
| E406  | NonComparableTypes            | A comparison operator on types that are not pairwise comparable per R-TYPE-04. |

### Execution (E5xx)

| Code  | Name                          | Trigger                                                        |
| ----- | ----------------------------- | -------------------------------------------------------------- |
| E501  | DivisionByZero                | Numeric division where the divisor evaluates to zero (R-NUM-05).|
| E502  | OperandTypeMismatchAtRuntime  | An operand is the wrong runtime kind for its operator.         |
| E503  | UnknownStepAtRuntime          | Defensive: a step reference resolved at execution that should have been caught earlier. |
| E504  | NumericOverflow               | Integer overflow per R-NUM-06.                                 |
| E505  | IndexOutOfRange               | List or row index outside the valid range (R-LIST-04).         |
| E506  | UnknownField                  | Field access on a record whose field set lacks the requested name (R-REC-02). |
| E507  | CoercionFailure               | A raw text cell cannot be coerced to its column's inferred type at the row in question (R-NUM-09, R-VAL-02). |

### Warnings (Wxxx)

| Code  | Name                          | Trigger                                                        |
| ----- | ----------------------------- | -------------------------------------------------------------- |
| W001  | UnusedStep                    | A binding whose name is never referenced in any subsequent step or in the selection clause. |
| W002  | UnsupportedSqlLowering        | A function whose SQL lowering is not implemented; emitted by the SQL stage. |

## Rendering

**R-ERR-04 — Plain-string conversion at the boundary only.** Diagnostics are passed through the pipeline as records. Conversion to user-visible text happens only at: the diagnostics renderer (for console output), or the web layer's flattening (for JSON detailed-error records).

**R-ERR-05 — Renderer output shape.** A rendered diagnostic includes the source line containing each label, an underline pointing at each label's region, the primary message above the source line, and the suggestion text (if any) on a separate line below. Multiple labels render as multiple underlined regions.

## Test coverage

Pointers will live under `conformance/cross_cutting/error_model/`.

## Open questions

- E104: comment support is on the roadmap.
- W001: not yet emitted; tracked.
- The exact human wording of each message is subject to revision; the codes are stable.


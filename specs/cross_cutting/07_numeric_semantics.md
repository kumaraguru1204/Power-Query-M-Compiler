# Cross-cutting: Numeric Semantics

## Scope
Applies to the executor (arithmetic, comparison, coercion) and to the type-checker (numeric-operator inference).

## Rules

**R-NUM-01 — Two runtime numeric kinds.** Integer (64-bit signed) and Float (64-bit IEEE 754).

**R-NUM-02 — Three static numeric types.** Integer, Float, Currency. Currency is treated as Float for arithmetic at runtime; this is a known simplification documented in PROJECT_REPORT.

**R-NUM-03 — Operator-result type at compile time.** The result of a binary arithmetic operator is Integer if both operands are Integer; Float otherwise. The result of a unary minus is the operand's type.

**R-NUM-04 — Operand widening at runtime.** If either operand of an arithmetic or comparison operator is Float and the other is Integer, the Integer is widened to Float and the operation is performed in Float. The result is Float.

**R-NUM-05 — Division.** Integer-by-integer division produces an Integer when the divisor evenly divides the dividend, and a Float otherwise. (Implementation detail: division never truncates silently; if precision is needed, the system widens to Float.) Division by zero where neither operand is null is a runtime error R-ERR-EXEC-DIV0; null operands win first per R-NULL-02.

**R-NUM-06 — Overflow.** Integer addition, subtraction, and multiplication that overflow the 64-bit signed range produce a runtime error R-ERR-EXEC-OVERFLOW. (No silent wrap-around.) Float operations that overflow produce positive or negative infinity per IEEE 754.

**R-NUM-07 — Comparison.** Numeric comparison follows numerical order after widening per R-NUM-04. Float NaN compares unequal to everything including itself; less-than and greater-than against NaN produce false.

**R-NUM-08 — Literal type assignment.** A numeric literal with no decimal point is Integer (R-LEX-04); with a decimal point, Float (R-LEX-05). Negative literals are unary-minus over a positive literal but the parser folds the two into one literal (R-PARSE-07).

**R-NUM-09 — Coercion from text.** A text value coerces to Integer if it parses as a finite signed integer in the 64-bit range. It coerces to Float if it parses as a finite signed floating-point number. Coercion failure is R-ERR-EXEC-TYPE.

**R-NUM-10 — Currency coercion.** A text value coerces to Currency the same way it coerces to Float; the difference is recorded only in the static type, not the runtime value (R-NUM-02).

**R-NUM-11 — Boolean is not numeric.** A Boolean cannot be added to an integer; the operation is R-ERR-TYPE-OPERAND. Implicit Boolean-to-integer conversion does not happen.

## Pinned coercion table — column-type inference and parsing

The workbook-ingestion stage (§6.1 of `SOURCE_OF_TRUTH.md`) and the per-cell coercion routine (`crates/pq_types/src/inference.rs`, `coercion.rs`) accept the forms below and only the forms below. Anything else falls through to Text.

| Raw text input | Parses as | Notes |
| --- | --- | --- |
| `0`, `1`, `-1`, `42`, `9223372036854775807` | Integer | Rust `i64::from_str`. Leading `+` rejected. |
| `-9223372036854775808` | Integer | Min `i64`. |
| `9223372036854775808` | Float (overflow falls through to `f64::from_str`) | Beyond `i64` range. |
| `1.0`, `-3.14`, `1e10`, `1.5E-3`, `inf`, `NaN` | Float | Rust `f64::from_str` rules. |
| `true`, `false`, `True`, `FALSE` | Boolean | Case-insensitive match against the literals `true`/`false` only. |
| `Yes`, `No`, `1`/`0` as Boolean | Text (Boolean inference fails) | Only `true`/`false` are recognised. |
| `1,000`, `1.000,5` | Text | No locale-specific thousand or decimal separators. |
| `$1.50`, `1.50%`, `(5)` (accountancy) | Text | No currency / accountancy symbols are stripped. |
| Empty string | Text | Empty cells fall through. |
| Leading or trailing whitespace | Text | The parser does not trim. |

**Column-type inference order (R-NUM-09).** For a column to be inferred Integer, **every** non-empty cell must satisfy `i64::from_str`. The same all-must-pass rule applies for Float, then Boolean, then Text. Inference is order-sensitive: Integer wins if available, Float next, Boolean next, Text last (`crates/pq_types/src/inference.rs`).

**Mixed columns.** A single non-numeric cell in a column of integers falls the whole column to the next eligible type. Example: `"1"`, `"2"`, `"hello"` → Text.

**Currency, Date, DateTime, DateTimeZone, Time, Duration, Binary** — these types **never appear in inferred columns**. They only enter the type system through `Table.TransformColumnTypes`, which records the static type but does not parse or validate the underlying text at workbook ingestion time. Their text form is documented in `09_date_time_semantics.md`.

## Examples

*1 + 2* is an Integer 3 (R-NUM-03). *1 + 2.0* is a Float 3.0 (R-NUM-04). *6 / 2* is an Integer 3 (R-NUM-05); *7 / 2* is a Float 3.5 (R-NUM-05).

*9223372036854775807 + 1* is a runtime overflow error (R-NUM-06).

*null + 1* is null (R-NULL-02), *not* an error.

## Test coverage

Pointers will live under `conformance/cross_cutting/numeric/`.

## Open questions

- R-NUM-02: full Currency arithmetic with rounding rules is on the roadmap.
- R-NUM-06: silent wrap-around versus error is a deliberate choice; revisit if performance demands.


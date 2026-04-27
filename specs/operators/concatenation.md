# Operator: Concatenation

## Operator
Ampersand.

## Arity
Binary infix.

## Precedence and associativity
Sits at the additive level alongside plus and minus (R-PARSE-05). Left-associative.

## Operand types
Both operands must be Text. A non-Text operand is R-ERR-TYPE-OPERAND.

## Result type
Text — except where R-NULL-03 produces null.

## Runtime
Produces the concatenation of the two text values per R-TEXT-02.

## Null
Any null operand short-circuits to null (R-NULL-03).

## Type errors
A non-Text operand is R-ERR-TYPE-OPERAND (E401). Implicit coercion of numbers, booleans, or other types to text via concatenation is **not** performed; the user must call the relevant *.From* function explicitly.

## SQL lowering
Maps to SQL string concatenation. ANSI SQL spells this as *||*; the emitter uses *||* by default. Some dialects (T-SQL) require *+* or *CONCAT*; per-dialect adjustment is on the roadmap.

## Conformance
Pointers to fixtures will live under `conformance/operators/concatenation/`.

## Open questions
- Lowering to T-SQL needs a dialect-aware operator choice.


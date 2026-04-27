# Cross-cutting: Lexical Grammar

## Scope
Applies to the lexing stage. Cited by parsing, error model, and several family files.

## Rules

**R-LEX-01 — Whitespace.** Spaces, tabs, carriage returns, and line feeds separate tokens but produce no token of their own. A line feed advances the line counter and resets the column counter.

**R-LEX-02 — Comments.** Not currently supported. Any sequence beginning with two forward slashes or a slash-asterisk is a lexical error (R-ERR-LEX-04). Tracked as a planned extension in PROJECT_REPORT.

**R-LEX-03 — String literals.** A double-quote opens a string literal. Characters up to the matching unescaped double-quote form the literal's value. Two consecutive double-quotes inside a literal produce one literal double-quote in the value. End-of-input before the closing quote is R-ERR-LEX-01.

**R-LEX-04 — Integer literals.** A run of decimal digits with no decimal point produces an integer literal. The value must fit in a 64-bit signed integer; otherwise R-ERR-LEX-03.

**R-LEX-05 — Float literals.** A run of decimal digits containing exactly one decimal point produces a float literal. Two or more decimal points is R-ERR-LEX-03.

**R-LEX-06 — Identifiers.** A letter or underscore opens an identifier. Continuation characters are letters, digits, and underscores. The accumulated text is then matched case-sensitively against the keyword table (R-LEX-07): a hit produces the keyword token; a miss produces a generic identifier token.

**R-LEX-07 — Keywords.** The complete keyword set is *let*, *in*, *each*, *and*, *or*, *not*, *true*, *false*, *null*. Every other identifier is generic.

**R-LEX-08 — Quoted identifiers.** A hash followed immediately by a double-quote opens a quoted identifier. Characters between the quotes (with the same doubled-quote escape as R-LEX-03) form an identifier with arbitrary characters allowed.

**R-LEX-09 — Punctuation tokens.** The single-character punctuation tokens are period, comma, open and close parenthesis, open and close brace, open and close bracket. Each becomes a dedicated token kind.

**R-LEX-10 — Arithmetic and concatenation operators.** The single-character operator tokens are plus, minus, asterisk, forward slash, ampersand. Each becomes its dedicated kind.

**R-LEX-11 — Equals and fat-arrow.** A bare equals is the comparison-equals or assignment token. An equals immediately followed by a greater-than sign is the fat-arrow token used by explicit lambdas.

**R-LEX-12 — Less-than family.** A less-than may be: less-than alone; less-than-or-equal when followed by equals; not-equal when followed by greater-than.

**R-LEX-13 — Greater-than family.** A greater-than may be: greater-than alone; greater-than-or-equal when followed by equals.

**R-LEX-14 — Unknown character.** Any character that does not start one of the above is R-ERR-LEX-02.

**R-LEX-15 — End of input.** When the input is exhausted, an end-of-input token is appended; it carries the position immediately past the last character.

## Examples

The text *let x = 1 in x* lexes to: keyword *let*, identifier *x*, equals, integer 1, keyword *in*, identifier *x*, end of input.

The text *#"My Column"* lexes to a single quoted-identifier token whose value is the string *My Column*.

The text *<>* lexes to a single not-equal token (R-LEX-12), not to a less-than followed by a greater-than.

## Test coverage

Pointers will live under `conformance/cross_cutting/lexical/` once fixtures are written.

## Open questions

- R-LEX-02: comments are not yet supported. Decision needed on which syntaxes to accept.


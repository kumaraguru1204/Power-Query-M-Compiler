# Cross-cutting: Text Semantics

## Scope
Applies to the executor (text operators and Text.* functions) and to the type-checker.

## Rules

**R-TEXT-01 — Text is a sequence of Unicode code points.** Length is measured in code points, not bytes. Implementation detail: the executor stores text as UTF-8 internally; counting and slicing operate on code-point boundaries.

**R-TEXT-02 — Concatenation.** The ampersand operator on two Text operands produces their concatenation. Null on either side produces null (R-NULL-03).

**R-TEXT-03 — Equality is case-sensitive.** Two text values are equal iff their code-point sequences are identical. The text *"A"* is not equal to *"a"*.

**R-TEXT-04 — Comparison is by code-point order.** Less-than and greater-than compare lexicographically by code-point value, not by collation. *"B" < "a"* is true because uppercase B has a lower code point than lowercase a.

**R-TEXT-05 — Substring search is case-sensitive by default.** Text.Contains, Text.StartsWith, and Text.EndsWith compare code-point-by-code-point. Case-insensitive comparison is not currently supported; tracked in PROJECT_REPORT.

**R-TEXT-06 — Empty text matches at every position.** Text.Contains returns true when the needle is the empty text; Text.StartsWith and Text.EndsWith return true when the needle is the empty text.

**R-TEXT-07 — Null in text functions.** Any Text.* function with a null argument produces null (consistent with R-NULL-03 for concatenation; the analogue for the predicate functions).

**R-TEXT-08 — Coercion from non-text values.** Coercing an integer or float to Text produces its base-ten textual representation (no thousands separator, no leading zeros, point as decimal separator). Coercing a Boolean produces the lowercase text *true* or *false*. Coercing null produces the text *null*. Coercing dates and times follows ISO-8601-style formats; tracked in PROJECT_REPORT for full coverage.

**R-TEXT-09 — Coercion to non-text values.** See R-NUM-09, R-NULL-10. A text *"true"* or *"false"* (case-insensitive) coerces to Boolean.

## Examples

*"abc" & "def"* is *"abcdef"* (R-TEXT-02). *"abc" = "ABC"* is false (R-TEXT-03). Text.Contains(*"hello"*, *""*) is true (R-TEXT-06).

## Test coverage

Pointers will live under `conformance/cross_cutting/text/`.

## Open questions

- R-TEXT-04: collation-aware comparison would be a future extension.
- R-TEXT-05: case-insensitive variants of the predicates are deferred.
- R-TEXT-08: full date and time coercion needs a unified formatting policy.


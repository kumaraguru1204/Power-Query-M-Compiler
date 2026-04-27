# F02 — Binary text predicate

## Members

- **Text.Contains** — true iff the haystack contains the needle anywhere.
- **Text.StartsWith** — true iff the haystack begins with the needle.
- **Text.EndsWith** — true iff the haystack ends with the needle.

## Shared argument shape

Two required positional arguments: a haystack (Text) and a needle (Text).

## Shared type signature

*(Text, Text) → Boolean*. Either argument null produces null per R-NULL-04 analogue (R-TEXT-07).

## Shared schema-transform rule

Not applicable (these are expression-level functions used inside lambda bodies, not step-level functions).

## Shared runtime semantics

1. Evaluate the haystack and needle to text values.
2. If either is null, return null.
3. Otherwise apply the per-member matching rule per R-TEXT-05.
4. Empty needle is true regardless of haystack content (R-TEXT-06).

## Shared SQL lowering

Each member lowers to a SQL pattern-matching expression with the per-member shape from the appendix.

## Shared null/empty/error rules

- Null in either argument: null (not Boolean).
- Empty needle: true.
- Empty haystack and non-empty needle: false.

## Per-member appendix

| Function          | Match position             | SQL pattern                                   |
| ----------------- | -------------------------- | --------------------------------------------- |
| Text.Contains     | Anywhere in the haystack.  | haystack LIKE '%' || needle || '%'            |
| Text.StartsWith   | At position 0.             | haystack LIKE needle || '%'                    |
| Text.EndsWith     | At the end of the haystack.| haystack LIKE '%' || needle                    |

The SQL patterns assume needles do not contain LIKE special characters (% and _). Pattern-character escaping is on the roadmap.

## Conformance

Family-level fixtures: `conformance/families/F02_binary_text_predicate/`.
Per-member fixtures: `conformance/functions/Text.Contains/`, `Text.StartsWith/`, `Text.EndsWith/`.

## Open questions

- Case-insensitive variants are deferred (R-TEXT-05).
- LIKE special-character escaping needed before production use of SQL lowering.


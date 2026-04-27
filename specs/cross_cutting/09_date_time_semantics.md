# Cross-cutting: Date and Time Semantics

## Scope
Applies to the executor's coercion, comparison, and arithmetic on temporal values, and to type-checking.

## Status note

Temporal support in the system is **partial**. The static type vocabulary (`Date`, `DateTime`, `DateTimeZone`, `Time`, `Duration`) is defined in `crates/pq_types/src/column_type.rs` and round-trips through `Int64.Type`-style M type names, **but the runtime treats temporal values as opaque text**. There is no chrono dependency, no civil-date arithmetic, and no time-zone normalisation in the current implementation. The rules below describe the contract the implementation aims to honour; gaps are listed under "Open questions" and tracked in PROJECT_REPORT. Where the contract and the implementation disagree, the contract wins (a fix is owed).

## Pinned coercion table — recognised text forms

The workbook-ingestion stage (`crates/pq_types/src/inference.rs`) **does not infer any temporal type**. Every column whose cells look like dates is inferred as Text and stays Text unless a downstream `Table.TransformColumnTypes` re-tags the column. The table below pins what is and is not currently recognised when such re-tagging happens, so a re-implementer can match behaviour byte-for-byte.

| Raw text | Aim | Today's behaviour |
| --- | --- | --- |
| `2024-01-02` | Date | Recognised as Text at ingestion; stored as text after `TransformColumnTypes(..., type date)`; comparison and arithmetic operate on the text form, which is correct only because ISO-8601 date strings sort lexicographically (R-DT-04). |
| `2024-03-15T13:45:00` | DateTime | Same as above. ISO 8601 only; the literal `T` separator is required. |
| `2024-03-15 13:45:00` (space separator) | DateTime | Currently falls back to Text. |
| `2024-03-15T13:45:00Z` / `+05:30` | DateTimeZone | Stored as text; offset is preserved verbatim but no normalisation is performed. |
| `13:45:00` | Time | Stored as text. |
| `P1D`, `PT1H30M` (ISO 8601 durations) | Duration | Not recognised; stays Text. |
| `01/02/2024`, `2/1/2024`, `Mar 15 2024` | Date (locale forms) | **Not recognised.** Falls to Text. |
| `1700000000` (UNIX timestamp seconds) | DateTime | Inferred Integer. |
| `2024-01-02T13:45:00.123456789` (sub-second) | DateTime | Stored as text; sub-second precision is preserved verbatim. |

**What this means for a re-implementer.** A behaviour-equivalent rebuild may store temporal values as opaque strings and use lexicographic comparison for ISO-8601 forms; or it may go further than the current engine and parse to a richer civil-date representation. Either is valid as long as: (a) ingestion never auto-infers a temporal type, and (b) `TransformColumnTypes` accepts the static type re-tag without rejecting any value the current engine accepts.

## Rules

**R-DT-01 — Temporal type vocabulary.** Date (a calendar day with no time-of-day), DateTime (a calendar day with a time-of-day), DateTimeZone (a DateTime plus an offset), Time (a time-of-day with no date), Duration (a span of time, signed).

**R-DT-02 — Coercion from text — Date.** A text in the form *YYYY-MM-DD* may be re-tagged as Date via `Table.TransformColumnTypes(..., type date)`. Workbook-ingestion **never auto-infers Date**; cells stay Text until explicit re-tagging. Locale-specific forms (such as *DD/MM/YYYY*) are not currently recognised at all.

**R-DT-03 — Coercion from text — DateTime.** A text in the form *YYYY-MM-DDTHH:MM:SS* may be re-tagged as DateTime. Workbook-ingestion **never auto-infers DateTime**. Variations and sub-second precision are tracked in PROJECT_REPORT.

**R-DT-04 — Equality.** Two temporal values of the same kind are equal iff their underlying instants (Date: civil date; DateTime: civil date and clock time; DateTimeZone: instant relative to UTC; Time: clock time; Duration: duration in nanoseconds) are equal.

**R-DT-05 — Comparison.** Two temporal values of the same kind compare in chronological order. Comparing across kinds (Date with DateTime, etc.) is R-ERR-TYPE-OPERAND.

**R-DT-06 — Arithmetic — Duration plus Date.** A Date plus a Duration whose precision is a whole number of days produces a Date. A Date plus a Duration with a sub-day component produces a DateTime.

**R-DT-07 — Arithmetic — DateTime plus Duration.** A DateTime plus a Duration produces a DateTime.

**R-DT-08 — Subtraction.** Date minus Date produces Duration. DateTime minus DateTime produces Duration. Date minus Duration produces Date. DateTime minus Duration produces DateTime.

**R-DT-09 — Cross-kind arithmetic.** Multiplying or dividing a temporal value by a number is undefined; R-ERR-TYPE-OPERAND.

**R-DT-10 — Time-zone semantics.** DateTimeZone values are normalised to UTC for comparison and arithmetic; the original offset is preserved for round-trip rendering. The default zone for a bare DateTime is the local zone of the runtime; this is a deliberate simplification and is documented in PROJECT_REPORT.

## Examples

A column whose cells are *2024-01-02*, *2024-03-15* is inferred Date (R-DT-02). A column whose cells are *01/02/2024* is currently inferred Text — the slash form is not yet recognised (R-DT-02 status).

The expression Date subtraction *2024-03-15 minus 2024-01-02* produces a Duration of 73 days (R-DT-08).

## Test coverage

Pointers will live under `conformance/cross_cutting/date_time/`.

## Open questions

- R-DT-02 and R-DT-03: locale-specific text forms must be recognised; a small parser is owed.
- R-DT-10: configurable default zone needs design.


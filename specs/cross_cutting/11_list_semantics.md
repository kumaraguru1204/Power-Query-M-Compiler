# Cross-cutting: List Semantics

## Scope
Applies to every List.* family file, the executor's list-handling, and the SQL emitter (where lists become VALUES sub-queries).

## Rules

**R-LIST-01 — A list is an ordered sequence of values.** Order is preserved through every list-returning operation unless the operation explicitly re-orders.

**R-LIST-02 — Element type is inferred bottom-up.** A list literal whose element types all unify to T has type List-of-T. A list literal mixing types has type List-of-Any (R-TYPE-09).

**R-LIST-03 — Empty list element type defaults to None.** A bare empty list literal has type List-of-None at the type-checker; functions that consume it specify how empty input is handled.

**R-LIST-04 — Index origin.** List indexing is zero-based throughout the system. Index out of range is R-ERR-EXEC-INDEX.

**R-LIST-05 — Equality.** Two lists are equal iff they have the same length and their pairwise elements are equal under R-VAL-10.

**R-LIST-06 — Lists in row context.** When a list flows through an each-shorthand applied per element (members of F05), the underscore is bound to the current element and the lambda body is evaluated for each element in source order (R-EACH-03).

**R-LIST-07 — Null elements are permitted.** Null appears in lists like any other value. Aggregates handle null per their per-function rule (R-NULL-07).

**R-LIST-08 — Lists versus tables.** A list is *not* a table. A list of records superficially resembles a single-column table whose cells are records, but it is opaque to bracket-column-access. A function that needs a table from a list of records is Tables.FromRecords or similar.

**R-LIST-09 — Generation functions produce lists eagerly.** List.Numbers, List.Repeat, List.Random, and List.Generate produce a fully materialised list at evaluation time; lazy lists are not part of the runtime value vocabulary.

**R-LIST-10 — Comparable lists.** Two lists may be compared with equals and not-equals (per R-LIST-05). Less-than and greater-than between lists is currently undefined and produces R-ERR-TYPE-OPERAND.

## Examples

The list *{1, 2, 3}* has type List-of-Integer (R-LIST-02). The list *{1, "two"}* has type List-of-Any.

List.Numbers(start=1, count=3) produces *{1, 2, 3}* (R-LIST-09).

## Test coverage

Pointers will live under `conformance/cross_cutting/list/`.

## Open questions

- R-LIST-04: M's official documentation occasionally implies one-based indexing for some functions; tracked for review.
- R-LIST-09: lazy or streaming lists are out of scope for this implementation.


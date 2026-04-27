# Cross-cutting: Table Semantics

## Scope
Applies to every Table.* family file, the executor's table-flow handling, and the SQL emitter.

## Rules

**R-TBL-01 — A table is an ordered list of typed columns.** Column order is part of the table's identity. Two tables with the same columns in different order are not equal as tables.

**R-TBL-02 — Each column is named, typed, and stores raw text cells.** A column carries a name, an inferred type, and a vector of raw text values. Cells are coerced to typed runtime values on demand (R-VAL-02).

**R-TBL-03 — Row identity is positional.** A row is identified by its position (zero-based or one-based as stated by each operation). The system does not assign opaque row identifiers; positional identity is sufficient because tables are immutable in transit (R-INV-09).

**R-TBL-04 — Schema propagation defaults to passthrough.** A function that does not declare a schema-transform hook is assumed to leave the input schema unchanged. Functions that change the schema do so by declaring a schema-transform hook in the catalogue (R-CAT-04 in `pipeline/05_type_checking.md`).

**R-TBL-05 — Empty tables preserve schema.** A table with zero rows still has a defined schema (its columns and their types are present). A function that operates row-wise on an empty input table returns an empty output table whose schema is the function's normal output schema for the given input schema.

**R-TBL-06 — Tables are immutable in transit.** Each step produces a fresh table; no step mutates its input table in place (Source-of-Truth invariant R-INV-09).

**R-TBL-07 — Column equality is structural.** Two columns are equal iff their names match, their types match, and their cell sequences match (after coercion to runtime values, with R-VAL-10 equality).

**R-TBL-08 — Table equality is structural.** Two tables are equal iff they have the same columns in the same order. Source name and sheet name are *not* part of table equality.

**R-TBL-09 — A row as a record.** When a table is iterated row-wise (most often inside a row-context lambda), each row presents as a record whose field names are the column names and whose field values are the column cells coerced to runtime values (R-EACH-02).

**R-TBL-10 — Column-name uniqueness within a table.** A table never has two columns with the same name. Functions that would produce a name clash either rename one column (default suffixing scheme: append a numeric suffix starting at 1) or raise R-ERR-SCHEMA-CLASH; per-function rule documented in each family.

## Examples

A table loaded from a workbook with headers *Name*, *Age* has two columns; a row in that table is the record with fields *Name* and *Age* (R-TBL-09).

A row-filter whose predicate is *each false* on a four-row table returns a zero-row table with the same two columns *Name* and *Age* (R-TBL-05).

## Test coverage

Pointers will live under `conformance/cross_cutting/table/`.

## Open questions

- R-TBL-10: the default suffixing scheme is undocumented per function; tracked.


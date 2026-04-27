# Cross-cutting: Record Semantics

## Scope
Applies to record literals, record values at runtime, field access, and a small number of functions that produce records (Tables.FromRecords, Table.AddColumn for record-valued columns).

## Rules

**R-REC-01 — A record is an ordered map from field name to value.** Field names are unique within a record. Field-insertion order is preserved.

**R-REC-02 — Field access.** *r[name]* fetches the field named *name* from the record *r*. An unknown field is R-ERR-EXEC-FIELD by default; the missing-field policy can be per-call (the F08 RemoveColumns family describes this).

**R-REC-03 — Field equality is case-sensitive.** Two field names that differ in case are different fields.

**R-REC-04 — Records as row representations.** A row of a table presents as a record (R-TBL-09); the record's field names are the column names.

**R-REC-05 — Nested records.** A record's field value may itself be a record. Field access chains (R-PARSE-09): *r[a][b]* navigates two levels.

**R-REC-06 — Record equality.** Two records are equal iff their field-name sets are equal and their field values are pairwise equal under R-VAL-10. Field order is *not* part of equality.

**R-REC-07 — Record literal type.** A record literal has a Record type whose fields are the literal's fields with their inferred types (R-TYPE-10).

**R-REC-08 — Records in lists.** A list of records is a valid runtime value; many table-shaped operations accept either a list of records or a table interchangeably at the boundary. The opacity rule R-LIST-08 applies inside the list.

**R-REC-09 — Records do not coerce to text by default.** Coercing a record value to text is not defined; doing so is R-ERR-EXEC-TYPE.

## Examples

The literal *[Name = "Alice", Age = 30]* is a record value with two fields (R-REC-01). *[Name = "Alice", Age = 30][Name]* is *"Alice"* (R-REC-02). *[Name = "Alice"] = [Name = "Alice"]* is true regardless of insertion order (R-REC-06 is by-set, but R-REC-01 preserves source order for iteration).

The literal *[User = [Name = "A"]]* combined with the access chain *[User][Name]* yields *"A"* (R-REC-05).

## Test coverage

Pointers will live under `conformance/cross_cutting/record/`.

## Open questions

- R-REC-09: an explicit Record.ToText with a documented format would be useful.


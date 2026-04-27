# Tables.FromRecords

## 1. Identity

- **Full name.** Tables.FromRecords.
- **Namespace.** Tables.
- **Family.** None (table construction from a list of records).
- **Status.** Partial.

## 2. Argument shape

- **Argument 1.** A list of records (StepRefOrValue).
- **Argument 2 (optional).** A list of column names (constrains output column order).
- **Argument 3 (optional).** A missing-field policy.

## 3. Type signature

*(List-of-Record [, List-of-Text] [, missing-field-policy]) → Table*.

## 4. Schema-transform rule

Output schema's columns are the union of the records' field names (unless the optional column-name list restricts them), each with its inferred type from the records.

## 5. Runtime semantics

For each record in the list, project its fields into a row of the output table. Missing fields are filled per policy: Error (default; E506), Ignore (skipped — the cell becomes null), UseNull (explicit null).

## 6. SQL lowering

Not yet implemented. Emits W002.

## 7. Reference behaviour

Aligns with official M.

## 8. Conformance fixtures

`conformance/functions/Tables.FromRecords/`.

## 9. Open questions and known gaps

- Field-type unification across records needs more rigour.


# F11 — Workbook entry

## Members

- **Excel.Workbook** — the workbook initialiser; the only catalogued member.

## Shared argument shape

A specific positional argument list documented in `functions/unique/Excel.Workbook.md`. The first argument is parsed via the FileContentsArg hint, which extracts the path string from a *File.Contents("path")* sub-call.

## Shared type signature

*(File-contents-handle [, nullable Boolean] [, nullable Boolean]) → Table*.

## Shared schema-transform rule

The output schema is the input table's schema (the workbook payload's columns).

## Shared runtime semantics

The workbook initialiser materialises the input table directly (R-EXEC-01). The path string is recorded for traceability; the actual file is never read.

## Shared SQL lowering

Becomes the source values-CTE per R-SQL-02.

## Shared null/empty/error rules

- File path string is required; missing is a parse-time error.
- The two optional Boolean flags are accepted but currently ignored at runtime.

## Per-member appendix

Only one member; see `functions/unique/Excel.Workbook.md`.

## Conformance

Family-level fixtures: `conformance/families/F11_workbook_entry/`.
Per-member fixtures: `conformance/functions/Excel.Workbook/`.


# Excel.Workbook

## 1. Identity

- **Full name.** Excel.Workbook.
- **Namespace.** Excel.
- **Family.** F11 — workbook entry. See `families/F11_workbook_entry.md`.
- **Status.** Implemented as a **passthrough**: the workbook payload supplied to the engine *is* the input table; the path string is a label. Full file parsing is a future extension.

## 2. Argument shape

The parser recognises the call only when its first argument is the special form *File.Contents("path")* (the FileContentsArg argument hint). The recogniser then reads up to two further optional arguments.

- **Argument 1 (required).** Hint: FileContentsArg. Extracts the path string. The path is recorded but never opened.
- **Argument 2 (optional).** Hint: OptNullableBool. Use-headers flag. Absent or null → not set; true / false → recorded.
- **Argument 3 (optional).** Hint: OptNullableBool. Auto-detect-types flag. Same encoding as argument 2.

A call whose first argument is *not* the *File.Contents("path")* form is **not** the workbook initialiser and is parsed as a generic function call (which fails with E203 today, since Excel.Workbook is not registered in the catalogue under any other shape).

## 3. Type signature

*(File-contents-handle [, nullable Boolean] [, nullable Boolean]) → Table*.

## 4. Schema-transform rule

The output schema is the input table's schema (the workbook payload's columns and their inferred types).

## 5. Runtime semantics

The executor materialises the input table directly:

1. Clone the input table (immutability per R-INV-09).
2. Re-tag its source-name field with the path string from the call.
3. Bind the clone to the step's name in the environment.

The optional Boolean flags are accepted but currently have no runtime effect; tracked.

## 6. SQL lowering

Becomes the source values-CTE per R-SQL-02:

- One CTE named after the step (typically *Source*).
- Built as a VALUES clause: each input row contributes a tuple of literal values rendered per R-SQL-06.
- Column aliases come from the headers; column types come from the inferred types (driving CAST when a literal does not natively render with the right SQL type).

## 7. Reference behaviour

Official Power Query M parses the workbook at the path and returns a *table of sheets*; each sheet's *Data* field is itself a table. This implementation is intentionally **not** a real workbook reader: it treats the path string as a label and uses the supplied JSON payload as the data. The downstream Source{[Item="…", Kind="Sheet"]}[Data] navigation is accepted by the parser as a sheet-navigation step but is also a passthrough.

## 8. Conformance fixtures

`conformance/functions/Excel.Workbook/` will hold:

- *passthrough.json* — verifies that the path is recorded and the data is the payload.
- *flags_ignored.json* — verifies the optional Booleans are accepted but have no effect.
- *non_filecontents_first_arg.json* — verifies E203 when the first argument is not *File.Contents("…")*.

## 9. Open questions and known gaps

- Real workbook parsing is on the roadmap but out of scope for the current contract.
- The use-headers flag's Boolean values should eventually drive header detection; today the first row is always headers.


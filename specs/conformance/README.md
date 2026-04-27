# Conformance Fixtures

This folder is the executable backbone of the specs. Each fixture pins a single behavioural rule down to a concrete input/expected-output pair so that a regression in the implementation breaks the build immediately.

---

## 1. Layout

```
conformance/
├── README.md                   ← this file
│
├── cross_cutting/
│   ├── lexical/
│   ├── syntactic/
│   ├── type_system/
│   ├── value_model/
│   ├── row_context/
│   ├── null/
│   ├── numeric/
│   ├── text/
│   ├── date_time/
│   ├── table/
│   ├── list/
│   ├── record/
│   ├── error_model/
│   ├── sql/
│   └── invariants/
│
├── operators/
│   ├── arithmetic/
│   ├── comparison/
│   ├── logical/
│   ├── concatenation/
│   └── unary/
│
├── families/
│   ├── F01_pure_scalar_unary/
│   ├── F02_binary_text_predicate/
│   ├── F03_list_unary_aggregate/
│   ├── F04_list_set_operation/
│   ├── F05_list_higher_order/
│   ├── F06_table_row_trim/
│   ├── F07_table_row_filter/
│   ├── F08_table_column_shape/
│   ├── F09_table_column_content/
│   ├── F10_table_aggregation/
│   ├── F11_workbook_entry/
│   └── F12_list_construction/
│
├── functions/                  ← one folder per function with leaves or full specs
│   ├── List.Difference/
│   ├── List.Distinct/
│   ├── …
│   ├── Table.SelectRows/
│   ├── …
│   └── Excel.Workbook/
│
└── interfaces/
    ├── http/
    ├── cli/
    └── playground/
```

## 2. Fixture file shape

Each fixture is a small JSON document with exactly these top-level fields:

- *name* — a short identifier unique within its folder.
- *description* — one sentence describing what the fixture pins down.
- *spec_refs* — a list of rule identifiers (R-AREA-NN) the fixture exercises.
- *json* — the workbook payload string.
- *formula* — the M source string.
- *expected* — exactly one of:
  - *result_table* — a textual representation of the expected result table preview, or
  - *error* — an object with *category* (Json / Lex / Parse / Diagnostics / Execute) and a list of expected diagnostic codes.

## 3. Folder-level READMEs

Every fixture folder contains a small README listing every fixture file in it with a one-line description. This makes browsing a folder a fast scan rather than an open-each-file walk.

## 4. How fixtures are run

A test driver under `tests/` reads every fixture file, calls the engine's compile function on each, and compares the result to the *expected* field. Any mismatch is a test failure.

The driver is responsible for:
- iterating fixtures recursively under `conformance/`,
- resolving the *spec_refs* against the corresponding spec files (a missing reference is a test failure: every cited rule must exist),
- producing a per-fixture pass/fail report.

## 5. Adding a fixture

1. Decide which folder fits (cross-cutting / operator / family / function / interface).
2. Create the JSON file with a clear *name* and a one-sentence *description*.
3. List the *spec_refs* you are pinning.
4. Add a one-line entry to that folder's README.
5. Run the test driver locally; verify the new fixture passes against the current implementation. If it doesn't, either the implementation is wrong (file a bug) or the spec rule is wrong (update the spec).

## 6. Coverage discipline

- Every rule in `cross_cutting/*.md` should have at least one fixture that exercises it.
- Every family file's per-member appendix row should have at least one fixture per member.
- Every error code in `cross_cutting/13_error_model.md` should have at least one fixture that triggers it.
- Every per-function leaf or full spec should have at least one fixture per documented edge case.

A coverage report (which rules have at least one fixture, which do not) is part of the periodic spec audit. As the audit identifies gaps, fixtures are added to close them.

## 7. Initial state

This folder ships with the structure above but most fixture files are not yet written. The first wave of fixtures should target the highest-value rules first:

1. The error-model codes (so every error path is exercised at least once).
2. The cross-cutting null and numeric rules (the ones most often subtly wrong).
3. The family files' shared rules (one per family).
4. The unique functions' edge cases.

Per-leaf fixtures fill in over time as the per-function leaves are hardened.


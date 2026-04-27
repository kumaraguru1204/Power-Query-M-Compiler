# M Engine — Specifications

> **Status.** Authoritative behavioural specification of every part of the system that the bird's-eye `SOURCE_OF_TRUTH.md` deliberately leaves out.
>
> **Audience.** Anyone reading, extending, or rebuilding the system in any programming language.
>
> **Form.** Plain English. No programming-language syntax, no code samples, no field names with punctuation.

---

## 1. Relationship to `SOURCE_OF_TRUTH.md`

`SOURCE_OF_TRUTH.md` is the bird's-eye view of the system: nine pipeline stages, five error categories, thirteen invariants, the function-catalogue concept, the entry-point binaries. It is necessary but not sufficient — it deliberately covers the *framework* and not the per-function *behaviour*.

This `specs/` directory is the close-up. Together with `SOURCE_OF_TRUTH.md`, it is the **complete contract** for the system. A competent engineer holding both, in any programming language, can rebuild a behaviourally equivalent system.

If a sentence in this directory ever disagrees with `SOURCE_OF_TRUTH.md`, the bird's-eye view wins; one of the two must be edited in the same change to reconcile them.

## 2. Folder layout

```
specs/
├── README.md                   ← this file
├── 00_conventions.md           ← templates, rule-id scheme, promotion criteria
│
├── cross_cutting/              ← shared rules cited by everything else
│   ├── 01_lexical_grammar.md
│   ├── 02_syntactic_grammar.md
│   ├── 03_type_system.md
│   ├── 04_value_model.md
│   ├── 05_row_context_and_each.md
│   ├── 06_null_propagation.md
│   ├── 07_numeric_semantics.md
│   ├── 08_text_semantics.md
│   ├── 09_date_time_semantics.md
│   ├── 10_table_semantics.md
│   ├── 11_list_semantics.md
│   ├── 12_record_semantics.md
│   ├── 13_error_model.md
│   ├── 14_sql_lowering_principles.md
│   └── 15_invariants.md
│
├── operators/                  ← per-operator-family
│   ├── arithmetic.md
│   ├── comparison.md
│   ├── logical.md
│   ├── concatenation.md
│   └── unary.md
│
├── pipeline/                   ← deeper than SOURCE_OF_TRUTH §6
│   ├── 01_workbook_ingestion.md
│   ├── 02_lexing.md
│   ├── 03_parsing.md
│   ├── 04_resolution.md
│   ├── 05_type_checking.md
│   ├── 06_formatting.md
│   ├── 07_execution.md
│   ├── 08_sql_emission.md
│   └── 09_packaging.md
│
├── interfaces/                 ← entry-point contracts
│   ├── http_api.md
│   ├── cli.md
│   └── playground.md
│
├── families/                   ← the primary unit of semantic specification
│   ├── README.md               ← taxonomy + function-to-family table
│   ├── F01_pure_scalar_unary.md
│   ├── F02_binary_text_predicate.md
│   ├── F03_list_unary_aggregate.md
│   ├── F04_list_set_operation.md
│   ├── F05_list_higher_order.md
│   ├── F06_table_row_trim.md
│   ├── F07_table_row_filter.md
│   ├── F08_table_column_shape.md
│   ├── F09_table_column_content.md
│   ├── F10_table_aggregation.md
│   ├── F11_workbook_entry.md
│   └── F12_list_construction.md
│
├── functions/                  ← per-function leaves (only when needed)
│   ├── README.md
│   ├── list/                   ← thin leaves (~8 lines each)
│   ├── table/                  ← thin leaves (~8 lines each)
│   └── unique/                 ← full per-function specs (~150 lines each)
│
└── conformance/                ← test-fixture catalogue
    └── README.md
```

## 3. Reading order

For someone new to the project who wants to understand it from scratch:

1. Read `SOURCE_OF_TRUTH.md` first — top to bottom.
2. Read `cross_cutting/15_invariants.md` — the 13 rules.
3. Skim `cross_cutting/` files 01 to 14 in numeric order.
4. Read `pipeline/` files 01 to 09 in numeric order.
5. Read `families/README.md` — the function taxonomy.
6. Pick any one family file; read it.
7. Pick any one per-function leaf in `functions/`; read it.
8. Read `interfaces/`.
9. Read `conformance/README.md` and inspect a few fixture folders.

After step 9, you can reconstruct the system.

## 4. Reading order for a focused change

For someone changing one M function:

1. Open `families/README.md`. Find the row for that function.
2. Open the family file it points to. Read the shared rules.
3. Open the per-function leaf if one exists (or the full per-function spec under `functions/unique/` if it lives there).
4. Open the conformance fixtures for the function.
5. Make the change. Update the spec if the change alters behaviour.

## 5. The discipline that keeps this directory true

Every spec file is **declarative**, not aspirational. It describes how the system actually behaves. When the implementation diverges from a spec, the divergence is a bug — either fix the code or update the spec; never let them disagree silently.

Conformance fixtures are how the discipline is enforced mechanically. A fixture that asserts behaviour the spec describes makes the spec executable. As fixtures grow, drift becomes impossible: the build fails.


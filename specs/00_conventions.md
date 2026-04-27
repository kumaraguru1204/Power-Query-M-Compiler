# Spec Conventions

This file defines the rules every other file in `specs/` follows. It exists so that any reader can predict the structure of any file before opening it, and any author can write a new file by following a fixed template.

---

## 1. Language

- English. No programming-language syntax. No identifiers with punctuation. No code samples.
- M-language fragments shown for illustration are written in italics, never in code blocks.
- File names of M functions appear as plain words: *Table.SelectRows*, not as code.

## 2. Rule identifiers

Every numbered rule in a cross-cutting file has a stable identifier of the form **R-AREA-NN**, where AREA is a short uppercase tag and NN is a two-digit sequence. Examples:

- R-LEX-01, R-LEX-02 — lexical-grammar rules.
- R-PARSE-01 — parsing rules.
- R-TYPE-01 — type-system rules.
- R-EACH-01 — row-context-and-each rules.
- R-NULL-01 — null-propagation rules.
- R-NUM-01 — numeric-semantics rules.
- R-TEXT-01 — text-semantics rules.
- R-DT-01 — date-time-semantics rules.
- R-TBL-01 — table-semantics rules.
- R-LIST-01 — list-semantics rules.
- R-REC-01 — record-semantics rules.
- R-ERR-01 — error-model rules.
- R-SQL-01 — SQL-lowering rules.
- R-INV-01 — invariants.

Rule identifiers are stable: once assigned, they are never reused. New rules append at the end of their area's sequence. A rule that is removed is marked withdrawn but its identifier is not reissued.

Other files cite rules by identifier: *"see R-EACH-01 in cross_cutting/05"*.

## 3. Per-family file template

Every file in `families/` follows this fixed structure:

1. **Family identifier and name.** F0X plus a human label.
2. **Members.** A bullet list of every function in the family with a one-line "differs in" note.
3. **Shared argument shape.** How the parser reads arguments for every member.
4. **Shared type signature.** The polymorphic signature with one degree of freedom.
5. **Shared schema-transform rule.** How the output shape derives from the input shape.
6. **Shared runtime semantics.** The one algorithm every member follows.
7. **Shared SQL lowering.** The one SQL pattern, with the per-member operation as a placeholder.
8. **Shared null/empty/error rules.** Cited from cross-cutting files.
9. **Per-member appendix.** A table whose rows fill in the per-member details.
10. **Conformance.** Pointers to fixture folders.

A family file is **complete** when every member's per-member-appendix row is filled in and a conformance fixture exists for the shared rules.

## 4. Per-function leaf template (thin)

Every file in `functions/list/`, `functions/table/`, and similar subfolders follows this fixed structure:

1. **Function name** as the title.
2. **Family.** A back-reference to the family file in `families/`.
3. **Per-member operation.** One sentence.
4. **Edge cases or divergences.** Anything not captured by the family rules.
5. **Conformance.** Pointer to the function's fixture folder.

A thin leaf is typically eight lines or fewer. If a leaf grows beyond about thirty lines, it is a signal to promote it to a full per-function spec under `functions/unique/`.

## 5. Per-function full-spec template (unique)

Every file in `functions/unique/` follows this fixed structure:

1. **Identity.** Full name, namespace, status (implemented / partial / not-yet / buggy with a link to `PROJECT_REPORT.md`).
2. **Argument shape.** Per-argument: hint variant, positional or optional, accepted M shapes.
3. **Type signature.** Per accepted arity: parameter types, return type, type-variable bindings.
4. **Schema-transform rule.** How the output schema derives from the input schema and the call arguments.
5. **Runtime semantics.** Step-by-step what the evaluator does, including error conditions and diagnostic codes.
6. **SQL lowering.** The exact SQL pattern, with substitution rules and explicit unsupported-fallback notes.
7. **Reference behaviour.** Citation of official Power Query M behaviour and any deliberate divergence.
8. **Conformance fixtures.** Pointer to the function's fixture folder.
9. **Open questions and known gaps.** Free-text section linking to `PROJECT_REPORT.md` TODOs.

## 6. Per-cross-cutting-file template

Every file in `cross_cutting/` follows this fixed structure:

1. **Scope.** Which stages and which function families this file's rules apply to.
2. **Rules.** Numbered with stable identifiers (R-AREA-NN).
3. **Examples.** A handful of small scenarios, in plain English.
4. **Test coverage.** Pointers into `conformance/` that exercise each rule.
5. **Open questions.** Anything still under design.

## 7. Per-pipeline-stage file template

Every file in `pipeline/` follows the same five-part shape used by `SOURCE_OF_TRUTH.md` §6, but with finer detail:

1. **Input.**
2. **What it does.**
3. **Output.**
4. **Failure modes.**
5. **Storage shape.**

## 8. Promotion criteria — when does a function need its own file?

Three tests. If any one of them is true, the function gets its own file (a thin leaf in `functions/list|table/...`, or a full spec in `functions/unique/`). Otherwise, the function lives entirely inside its family file's per-member appendix and has **no** separate file.

1. **Argument-hint divergence.** The function's argument-hint list differs from every other member of its family.
2. **Schema divergence.** The function's schema-transform rule cannot be expressed as a parameter to the family's shared rule.
3. **SQL divergence.** The function's SQL lowering is not a substitution into the family's pattern.

A function passing all three tests but with a still-tractable spec gets a thin leaf. A function with cross-cutting complexity gets a full spec under `functions/unique/`.

## 9. Conformance-fixture conventions

Each fixture is a small JSON document containing:

- a workbook payload,
- a formula,
- the expected outcome (either a result table preview or a typed error category with diagnostic code).

Fixtures are organised by topic:

- `conformance/families/F0X_*/` — fixtures testing shared family rules.
- `conformance/functions/<Function>/` — fixtures testing per-function specifics.
- `conformance/cross_cutting/<area>/` — fixtures testing cross-cutting rules.

Each fixture folder has a small README listing every fixture file with a one-line description of what it pins down.

## 10. Editing rules

- Any change to behaviour must update the spec **and** the corresponding fixture in the same commit.
- Adding a new rule appends at the end of its area; never insert in the middle.
- Withdrawing a rule marks it withdrawn but never deletes it; its identifier is permanently reserved.
- Adding a new function: place it in its family's per-member appendix; create a leaf only if a promotion criterion is met; create a fixture folder.
- Adding a new family: append a new F0X file; update `families/README.md`'s taxonomy table.


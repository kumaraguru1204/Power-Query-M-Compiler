# Per-function specifications — Index

This folder holds per-function spec files, organised in three groups:

- **`list/`** — thin leaves for List.* functions whose family file (in `families/`) covers their behaviour but which need a per-function note for unique edge cases.
- **`table/`** — thin leaves for Table.* functions in the same situation.
- **`unique/`** — full per-function specs for functions that fit no family or whose unique structure makes a family slot impossible.

A function not listed here is fully specified by its family file's per-member appendix; consult `families/README.md` to find the function and its assigned family.

---

## Files in this directory

### list/

Thin leaves for List.* functions. Each file is short (typically 8 lines) and exists to record:

- the per-function operation in one sentence;
- function-specific edge cases not captured by the family rules;
- a pointer to the function's conformance fixtures.

### table/

Same pattern as `list/`, for Table.* functions.

### unique/

Full per-function specs following the template in `00_conventions.md` §5. These functions either belong to no family or have enough divergence from their would-be family to deserve a complete standalone spec.

---

## Promotion criteria

A function is moved from family-only to a thin leaf, or from a thin leaf to a unique full spec, when one of the three conditions in `00_conventions.md` §8 is met. Re-evaluate every time the function is touched.


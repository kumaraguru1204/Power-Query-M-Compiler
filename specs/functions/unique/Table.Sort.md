# Table.Sort

## 1. Identity

- **Full name.** Table.Sort.
- **Namespace.** Table.
- **Family.** None (the sort key shape is unique).
- **Status.** Implemented for the typical shape (one or more columns with optional direction).

## 2. Argument shape

Two required positional arguments:

- **Argument 1.** Step reference. Hint: StepRef.
- **Argument 2.** Sort key list — a list of pairs *{column-name, direction}* where direction is the keyword *Order.Ascending* or *Order.Descending*. A bare column name is permitted as shorthand for ascending. Hint: SortList.

## 3. Type signature

*(Table, List-of-SortKey) → Table*.

## 4. Schema-transform rule

Schema preserved.

## 5. Runtime semantics

1. Evaluate the input table.
2. Stable-sort the rows by the multi-key tuple defined by the sort-key list.
3. Each per-key comparison uses the column's type's natural order (numeric, lexicographic, chronological).
4. Null sorts *first* in ascending order and *last* in descending (matching official M).

## 6. SQL lowering

ORDER BY clause whose elements are *"column" ASC* or *"column" DESC*.

Null ordering: an explicit *NULLS FIRST* / *NULLS LAST* per direction is appended where the dialect supports it (PostgreSQL does; ANSI baseline does not).

## 7. Reference behaviour

Aligns with official M's Table.Sort. The sort lambda form (a comparator lambda) supported by official M is not implemented in this engine; tracked.

## 8. Conformance fixtures

`conformance/functions/Table.Sort/`:

- *single_key_asc.json*
- *single_key_desc.json*
- *multi_key.json*
- *nulls_first_asc.json*

## 9. Open questions and known gaps

- Comparator-lambda form not supported.
- Nullability ordering relies on dialect support; ANSI fallback is best-effort.


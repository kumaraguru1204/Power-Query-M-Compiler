# Table.Group

## 1. Identity

- **Full name.** Table.Group.
- **Namespace.** Table.
- **Family.** None (irreducibly unique — schema depends on call arguments).
- **Status.** Implemented for the basic shape. Advanced grouping kinds are partial.

## 2. Argument shape

Three required positional arguments:

- **Argument 1.** Step reference (the input table). Hint: StepRef.
- **Argument 2.** Column-name list — the grouping keys. Hint: ColumnList.
- **Argument 3.** Aggregation list — a list of triples *{new-column-name, group-table-lambda, type}*. Hint: AggregateList.

Optional fourth argument: a group-kind keyword (Local or Global). Today only Local is supported.

## 3. Type signature

*(Table, List-of-Text, List-of-AggregateSpec [, group-kind]) → Table*.

## 4. Schema-transform rule

The output schema has:

- one column per grouping key (with the same type as the input column);
- one column per aggregation entry (with the entry's declared type, or the lambda's inferred return type if absent).

A clash between a key column and an aggregation column is E402.

## 5. Runtime semantics

1. Evaluate the input table.
2. Build groups by tuple-equality on the grouping-key columns. The first row in each group establishes the key tuple.
3. For each group, build a sub-table containing only the rows of that group with the original schema, then apply each aggregation lambda to the sub-table. The lambda's argument is the sub-table.
4. Emit one output row per group with the key columns and the aggregation results.

Output row order: the order in which each group's *first* row appeared in the input.

## 6. SQL lowering

Lowers to a SELECT with GROUP BY on the key columns. Each aggregation lambda must be one of the recognised SQL-translatable shapes: *each Table.RowCount(_)* → COUNT(*); *each List.Sum(Table.Column(_, "X"))* → SUM("X"); *each List.Min/Max/Average(...)* → the corresponding SQL aggregate.

A non-recognised aggregation lambda emits W002 and a placeholder.

## 7. Reference behaviour

Aligns with official Power Query M's *Table.Group* in Local mode. Global mode (which produces one row regardless of grouping) is on the roadmap.

## 8. Conformance fixtures

`conformance/functions/Table.Group/`:

- *count_per_group.json*
- *sum_per_group.json*
- *multi_key.json*
- *unsupported_aggregator.json* (verifies W002 fallback)

## 9. Open questions and known gaps

- Global mode unsupported.
- Aggregation lambdas with arbitrary bodies do not lower to SQL; this is a fundamental SQL limitation, not a bug.


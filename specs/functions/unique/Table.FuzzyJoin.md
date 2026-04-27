# Table.FuzzyJoin

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FuzzyJoin |
| Required arity | 5 |
| Max arity | 5 |
| Overloads | 1 |
| Argument hints | `step,cols,step,cols,join` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, Table, List<Text>, Any) -> Table |
| Doc | Table.FuzzyJoin(prev, {key}, other, {key}, JoinKind.X) |

## Behaviour

Table.FuzzyJoin(prev, {key}, other, {key}, JoinKind.X)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FuzzyJoin/](../../conformance/functions/Table.FuzzyJoin/) (create the folder when adding fixtures).
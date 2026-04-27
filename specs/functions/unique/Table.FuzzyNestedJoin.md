# Table.FuzzyNestedJoin

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FuzzyNestedJoin |
| Required arity | 6 |
| Max arity | 6 |
| Overloads | 1 |
| Argument hints | `step,cols,step,cols,str,join` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, Table, List<Text>, Text, Any) -> Table |
| Doc | Table.FuzzyNestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X) |

## Behaviour

Table.FuzzyNestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FuzzyNestedJoin/](../../conformance/functions/Table.FuzzyNestedJoin/) (create the folder when adding fixtures).
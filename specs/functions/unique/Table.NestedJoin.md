# Table.NestedJoin

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | NestedJoin |
| Required arity | 6 |
| Max arity | 6 |
| Overloads | 1 |
| Argument hints | `step,cols,step,cols,str,join` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, Table, List<Text>, Text, Any) -> Table |
| Doc | Table.NestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X) |

## Behaviour

Table.NestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.NestedJoin/](../../conformance/functions/Table.NestedJoin/) (create the folder when adding fixtures).
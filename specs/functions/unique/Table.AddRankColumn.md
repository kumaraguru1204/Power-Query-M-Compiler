# Table.AddRankColumn

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | AddRankColumn |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,str,sort,rec?` |
| Schema-transform hook | No |
| Primary signature | (Table, Text, List<Any>) -> Table |
| Doc | Table.AddRankColumn(prev, newCol, {{col,Order.X},...}, optional options) |

## Behaviour

Table.AddRankColumn(prev, newCol, {{col,Order.X},...}, optional options)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.AddRankColumn/](../../conformance/functions/Table.AddRankColumn/) (create the folder when adding fixtures).
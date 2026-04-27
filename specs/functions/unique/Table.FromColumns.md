# Table.FromColumns

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FromColumns |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `val,cols` |
| Schema-transform hook | No |
| Primary signature | (List<List<Any>>, List<Text>) -> Table |
| Doc | Table.FromColumns({list1,...},{col,...}) |

## Behaviour

Table.FromColumns({list1,...},{col,...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FromColumns/](../../conformance/functions/Table.FromColumns/) (create the folder when adding fixtures).
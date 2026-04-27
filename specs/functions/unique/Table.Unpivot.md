# Table.Unpivot

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Unpivot |
| Required arity | 4 |
| Max arity | 4 |
| Overloads | 1 |
| Argument hints | `step,cols,str,str` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, Text, Text) -> Table |
| Doc | Table.Unpivot(prev, {col,...}, attrCol, valCol) |

## Behaviour

Table.Unpivot(prev, {col,...}, attrCol, valCol)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.Unpivot/](../../conformance/functions/Table.Unpivot/) (create the folder when adding fixtures).
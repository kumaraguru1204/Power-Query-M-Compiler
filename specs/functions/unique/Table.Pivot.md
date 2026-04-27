# Table.Pivot

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Pivot |
| Required arity | 4 |
| Max arity | 4 |
| Overloads | 1 |
| Argument hints | `step,cols,str,str` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Any>, Text, Text) -> Table |
| Doc | Table.Pivot(prev, {val,...}, attrCol, valCol) |

## Behaviour

Table.Pivot(prev, {val,...}, attrCol, valCol)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.Pivot/](../../conformance/functions/Table.Pivot/) (create the folder when adding fixtures).
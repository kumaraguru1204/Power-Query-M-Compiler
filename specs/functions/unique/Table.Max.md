# Table.Max

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Max |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,str,val` |
| Schema-transform hook | No |
| Primary signature | (Table, Text, Any) -> Record |
| Doc | Table.Max(prev, col, default) |

## Behaviour

Table.Max(prev, col, default)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.Max/](../../conformance/functions/Table.Max/) (create the folder when adding fixtures).
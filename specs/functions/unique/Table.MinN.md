# Table.MinN

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | MinN |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,int,str` |
| Schema-transform hook | No |
| Primary signature | (Table, Number, Text) -> Table |
| Doc | Table.MinN(prev, n, col) |

## Behaviour

Table.MinN(prev, n, col)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.MinN/](../../conformance/functions/Table.MinN/) (create the folder when adding fixtures).
# Table.Combine

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Combine |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `steplist` |
| Schema-transform hook | No |
| Primary signature | (List<Table>) -> Table |
| Doc | Table.Combine({t1,t2,...}) |

## Behaviour

Table.Combine({t1,t2,...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.Combine/](../../conformance/functions/Table.Combine/) (create the folder when adding fixtures).
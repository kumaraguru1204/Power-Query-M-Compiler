# Table.InsertRows

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | InsertRows |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,int,reclist` |
| Schema-transform hook | No |
| Primary signature | (Table, Number, List<Record>) -> Table |
| Doc | Table.InsertRows(prev, offset, {row,...}) |

## Behaviour

Table.InsertRows(prev, offset, {row,...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.InsertRows/](../../conformance/functions/Table.InsertRows/) (create the folder when adding fixtures).
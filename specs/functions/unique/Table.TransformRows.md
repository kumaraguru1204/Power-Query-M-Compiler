# Table.TransformRows

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | TransformRows |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,each` |
| Schema-transform hook | Yes |
| Primary signature | (Table, (Record) -> Record) -> Table |
| Doc | Table.TransformRows(prev, each expr) |

## Behaviour

Table.TransformRows(prev, each expr)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.TransformRows/](../../conformance/functions/Table.TransformRows/) (create the folder when adding fixtures).
# Table.FromRecords

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FromRecords |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `step|val` |
| Schema-transform hook | No |
| Primary signature | (List<Record>) -> Table |
| Doc | Table.FromRecords(records) |

## Behaviour

Table.FromRecords(records)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FromRecords/](../../conformance/functions/Table.FromRecords/) (create the folder when adding fixtures).
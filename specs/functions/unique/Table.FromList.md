# Table.FromList

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FromList |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 2 |
| Argument hints | `step|val,val?` |
| Schema-transform hook | No |
| Primary signature | (List<T>) -> Table |
| Doc | Table.FromList(list, optional each splitter) |

## Behaviour

Table.FromList(list, optional each splitter)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FromList/](../../conformance/functions/Table.FromList/) (create the folder when adding fixtures).
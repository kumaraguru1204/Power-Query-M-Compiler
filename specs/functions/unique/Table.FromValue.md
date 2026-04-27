# Table.FromValue

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FromValue |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `val,rec?` |
| Schema-transform hook | No |
| Primary signature | (Any) -> Table |
| Doc | Table.FromValue(value, optional [DefaultFieldName=...]) |

## Behaviour

Table.FromValue(value, optional [DefaultFieldName=...])

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FromValue/](../../conformance/functions/Table.FromValue/) (create the folder when adding fixtures).
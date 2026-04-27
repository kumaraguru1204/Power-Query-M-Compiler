# Table.FromRows

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FromRows |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `val,cols` |
| Schema-transform hook | No |
| Primary signature | (List<Any>, List<Text>) -> Table |
| Doc | Table.FromRows({row,...},{col,...}) |

## Behaviour

Table.FromRows({row,...},{col,...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FromRows/](../../conformance/functions/Table.FromRows/) (create the folder when adding fixtures).
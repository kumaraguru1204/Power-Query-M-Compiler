# Text.Combine

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Text |
| Name | Combine |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 2 |
| Argument hints | `step|val,val?` |
| Schema-transform hook | No |
| Primary signature | (List<Text>) -> Text |
| Doc | Text.Combine(list) or Text.Combine(list, separator) |

## Behaviour

Text.Combine(list) or Text.Combine(list, separator)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Text.Combine/](../../conformance/functions/Text.Combine/) (create the folder when adding fixtures).
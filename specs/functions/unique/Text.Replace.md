# Text.Replace

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Text |
| Name | Replace |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `val,str,str` |
| Schema-transform hook | No |
| Primary signature | (Text, Text, Text) -> Text |
| Doc | Text.Replace(text,old,new) |

## Behaviour

Text.Replace(text,old,new)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Text.Replace/](../../conformance/functions/Text.Replace/) (create the folder when adding fixtures).
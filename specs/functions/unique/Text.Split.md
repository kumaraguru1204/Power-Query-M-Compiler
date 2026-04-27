# Text.Split

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Text |
| Name | Split |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `val,str` |
| Schema-transform hook | No |
| Primary signature | (Text, Text) -> List<Text> |
| Doc | Text.Split(text,delimiter) |

## Behaviour

Text.Split(text,delimiter)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Text.Split/](../../conformance/functions/Text.Split/) (create the folder when adding fixtures).
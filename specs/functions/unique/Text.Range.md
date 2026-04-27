# Text.Range

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Text |
| Name | Range |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `val,int,int` |
| Schema-transform hook | No |
| Primary signature | (Text, Number, Number) -> Text |
| Doc | Text.Range(text,offset,count) |

## Behaviour

Text.Range(text,offset,count)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Text.Range/](../../conformance/functions/Text.Range/) (create the folder when adding fixtures).
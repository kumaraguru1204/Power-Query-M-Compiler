# Text.PadEnd

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Text |
| Name | PadEnd |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `val,int,str` |
| Schema-transform hook | No |
| Primary signature | (Text, Number, Text) -> Text |
| Doc | Text.PadEnd(text,width,pad) |

## Behaviour

Text.PadEnd(text,width,pad)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Text.PadEnd/](../../conformance/functions/Text.PadEnd/) (create the folder when adding fixtures).
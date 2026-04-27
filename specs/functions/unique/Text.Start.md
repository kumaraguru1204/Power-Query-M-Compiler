# Text.Start

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Text |
| Name | Start |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `val,int` |
| Schema-transform hook | No |
| Primary signature | (Text, Number) -> Text |
| Doc | Text.Start(text, count) |

## Behaviour

Text.Start(text, count)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Text.Start/](../../conformance/functions/Text.Start/) (create the folder when adding fixtures).
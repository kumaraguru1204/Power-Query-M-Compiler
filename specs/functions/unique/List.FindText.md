# List.FindText

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | FindText |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step|val,str` |
| Schema-transform hook | No |
| Primary signature | (List<Any>, Text) -> List<Any> |
| Doc | List.FindText(list, text) -> values (including record fields) containing text |

## Behaviour

List.FindText(list, text) -> values (including record fields) containing text

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.FindText/](../../conformance/functions/List.FindText/) (create the folder when adding fixtures).
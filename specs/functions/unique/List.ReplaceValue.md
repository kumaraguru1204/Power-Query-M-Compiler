# List.ReplaceValue

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | ReplaceValue |
| Required arity | 4 |
| Max arity | 4 |
| Overloads | 1 |
| Argument hints | `step|val,val,val,each` |
| Schema-transform hook | No |
| Primary signature | (List<T>, T, T, (T, T) -> Boolean) -> List<T> |
| Doc | List.ReplaceValue(list, oldValue, newValue, replacer) -> list with value replaced |

## Behaviour

List.ReplaceValue(list, oldValue, newValue, replacer) -> list with value replaced

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.ReplaceValue/](../../conformance/functions/List.ReplaceValue/) (create the folder when adding fixtures).
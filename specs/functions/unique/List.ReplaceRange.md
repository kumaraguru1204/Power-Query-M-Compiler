# List.ReplaceRange

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | ReplaceRange |
| Required arity | 4 |
| Max arity | 4 |
| Overloads | 1 |
| Argument hints | `step|val,int,int,step|val` |
| Schema-transform hook | No |
| Primary signature | (List<T>, Number, Number, List<T>) -> List<T> |
| Doc | List.ReplaceRange(list, index, count, replaceWith) -> list with range replaced |

## Behaviour

List.ReplaceRange(list, index, count, replaceWith) -> list with range replaced

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.ReplaceRange/](../../conformance/functions/List.ReplaceRange/) (create the folder when adding fixtures).
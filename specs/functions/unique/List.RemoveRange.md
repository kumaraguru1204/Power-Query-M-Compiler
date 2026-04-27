# List.RemoveRange

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | RemoveRange |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 2 |
| Argument hints | `step|val,int,int?` |
| Schema-transform hook | No |
| Primary signature | (List<T>, Number) -> List<T> |
| Doc | List.RemoveRange(list, index, optional count) -> list with range of values removed |

## Behaviour

List.RemoveRange(list, index, optional count) -> list with range of values removed

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.RemoveRange/](../../conformance/functions/List.RemoveRange/) (create the folder when adding fixtures).
# List.InsertRange

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | InsertRange |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step|val,int,step|val` |
| Schema-transform hook | No |
| Primary signature | (List<T>, Number, List<T>) -> List<T> |
| Doc | List.InsertRange(list, index, values) -> list with values inserted at index |

## Behaviour

List.InsertRange(list, index, values) -> list with values inserted at index

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.InsertRange/](../../conformance/functions/List.InsertRange/) (create the folder when adding fixtures).
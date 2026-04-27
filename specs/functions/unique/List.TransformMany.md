# List.TransformMany

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | TransformMany |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step|val,each,each` |
| Schema-transform hook | No |
| Primary signature | (List<T>, (T) -> List<U>, (T, U) -> R) -> List<R> |
| Doc | List.TransformMany(list, listTransform, resultTransform) -> flattened transformed list |

## Behaviour

List.TransformMany(list, listTransform, resultTransform) -> flattened transformed list

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.TransformMany/](../../conformance/functions/List.TransformMany/) (create the folder when adding fixtures).
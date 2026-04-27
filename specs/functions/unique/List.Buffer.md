# List.Buffer

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Buffer |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `step|val` |
| Schema-transform hook | No |
| Primary signature | (List<T>) -> List<T> |
| Doc | List.Buffer(list) -> buffers a list in memory |

## Behaviour

List.Buffer(list) -> buffers a list in memory

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.Buffer/](../../conformance/functions/List.Buffer/) (create the folder when adding fixtures).
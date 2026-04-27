# List.Accumulate

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Accumulate |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step|val,val,each` |
| Schema-transform hook | No |
| Primary signature | (List<T>, U, (U, T) -> U) -> U |
| Doc | List.Accumulate(list, seed, (acc, x) => expr) -> accumulated summary value |

## Behaviour

List.Accumulate(list, seed, (acc, x) => expr) -> accumulated summary value

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.Accumulate/](../../conformance/functions/List.Accumulate/) (create the folder when adding fixtures).
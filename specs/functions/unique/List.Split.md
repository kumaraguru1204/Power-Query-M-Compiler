# List.Split

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Split |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step|val,int` |
| Schema-transform hook | No |
| Primary signature | (List<T>, Number) -> List<List<T>> |
| Doc | List.Split(list, pageSize) -> list of sub-lists each of length pageSize |

## Behaviour

List.Split(list, pageSize) -> list of sub-lists each of length pageSize

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.Split/](../../conformance/functions/List.Split/) (create the folder when adding fixtures).
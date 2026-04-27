# List.Alternate

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Alternate |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 2 |
| Argument hints | `step|val,int,int,int?` |
| Schema-transform hook | No |
| Primary signature | (List<T>, Number, Number) -> List<T> |
| Doc | List.Alternate(list, skip, take, optional offset) -> odd-numbered offset elements |

## Behaviour

List.Alternate(list, skip, take, optional offset) -> odd-numbered offset elements

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.Alternate/](../../conformance/functions/List.Alternate/) (create the folder when adding fixtures).
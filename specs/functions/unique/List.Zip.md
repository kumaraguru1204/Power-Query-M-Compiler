# List.Zip

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Zip |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `step|val` |
| Schema-transform hook | No |
| Primary signature | (List<List<Any>>) -> List<List<Any>> |
| Doc | List.Zip({list1, list2, ...}) -> list of lists combining items at same position |

## Behaviour

List.Zip({list1, list2, ...}) -> list of lists combining items at same position

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.Zip/](../../conformance/functions/List.Zip/) (create the folder when adding fixtures).
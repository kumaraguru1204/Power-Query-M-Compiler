# List.ReplaceMatchingItems

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | ReplaceMatchingItems |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 2 |
| Argument hints | `step|val,step|val,val?` |
| Schema-transform hook | No |
| Primary signature | (List<T>, List<List<T>>) -> List<T> |
| Doc | List.ReplaceMatchingItems(list, replacements, optional equationCriteria) -> replacements applied |

## Behaviour

List.ReplaceMatchingItems(list, replacements, optional equationCriteria) -> replacements applied

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.ReplaceMatchingItems/](../../conformance/functions/List.ReplaceMatchingItems/) (create the folder when adding fixtures).
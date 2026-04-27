# List.MaxN

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | MaxN |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 2 |
| Argument hints | `step|val,val,val?` |
| Schema-transform hook | No |
| Primary signature | (List<T>, Any) -> List<T> |
| Doc | List.MaxN(list, countOrCondition, optional comparisonCriteria) -> maximum N values |

## Behaviour

List.MaxN(list, countOrCondition, optional comparisonCriteria) -> maximum N values

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.MaxN/](../../conformance/functions/List.MaxN/) (create the folder when adding fixtures).
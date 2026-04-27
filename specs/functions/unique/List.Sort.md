# List.Sort

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Sort |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 2 |
| Argument hints | `step|val,val?` |
| Schema-transform hook | No |
| Primary signature | (List<T>) -> List<T> |
| Doc | List.Sort(list, optional comparisonCriteria) -> sorted list |

## Behaviour

List.Sort(list, optional comparisonCriteria) -> sorted list

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/List.Sort/](../../conformance/functions/List.Sort/) (create the folder when adding fixtures).
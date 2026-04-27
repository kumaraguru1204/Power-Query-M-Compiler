# List.Covariance

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Covariance |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step|val,step|val` |
| Schema-transform hook | No |
| Primary signature | (List<Number>, List<Number>) -> Number |
| Doc | List.Covariance(list1, list2) -> covariance between two number lists |

## Behaviour

List.Covariance(list1, list2) -> covariance between two number lists

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/List.Covariance/](../../conformance/functions/List.Covariance/) (create the folder when adding fixtures).
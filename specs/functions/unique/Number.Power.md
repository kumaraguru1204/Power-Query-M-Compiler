# Number.Power

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Number |
| Name | Power |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `val,val` |
| Schema-transform hook | No |
| Primary signature | (Number, Number) -> Number |
| Doc | Number.Power(base,exponent) |

## Behaviour

Number.Power(base,exponent)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Number.Power/](../../conformance/functions/Number.Power/) (create the folder when adding fixtures).
# Number.Mod

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Number |
| Name | Mod |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `val,val` |
| Schema-transform hook | No |
| Primary signature | (Number, Number) -> Number |
| Doc | Number.Mod(n,divisor) |

## Behaviour

Number.Mod(n,divisor)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Number.Mod/](../../conformance/functions/Number.Mod/) (create the folder when adding fixtures).
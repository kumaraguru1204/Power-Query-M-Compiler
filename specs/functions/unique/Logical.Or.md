# Logical.Or

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Logical |
| Name | Or |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `val,val` |
| Schema-transform hook | No |
| Primary signature | (Boolean, Boolean) -> Boolean |
| Doc | Logical.Or(a,b) |

## Behaviour

Logical.Or(a,b)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Logical.Or/](../../conformance/functions/Logical.Or/) (create the folder when adding fixtures).
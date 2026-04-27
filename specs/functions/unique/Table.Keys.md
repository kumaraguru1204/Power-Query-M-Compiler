# Table.Keys

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Keys |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `step` |
| Schema-transform hook | No |
| Primary signature | (Table) -> List<Any> |
| Doc | Table.Keys(prev) |

## Behaviour

Table.Keys(prev)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.Keys/](../../conformance/functions/Table.Keys/) (create the folder when adding fixtures).
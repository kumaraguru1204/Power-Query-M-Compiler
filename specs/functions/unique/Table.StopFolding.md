# Table.StopFolding

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | StopFolding |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `step` |
| Schema-transform hook | Yes |
| Primary signature | (Table) -> Table |
| Doc | Table.StopFolding(prev) |

## Behaviour

Table.StopFolding(prev)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.StopFolding/](../../conformance/functions/Table.StopFolding/) (create the folder when adding fixtures).
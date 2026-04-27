# Table.FromPartitions

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FromPartitions |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `str,step` |
| Schema-transform hook | No |
| Primary signature | (Text, Table) -> Table |
| Doc | Table.FromPartitions(col, partitions) |

## Behaviour

Table.FromPartitions(col, partitions)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.FromPartitions/](../../conformance/functions/Table.FromPartitions/) (create the folder when adding fixtures).
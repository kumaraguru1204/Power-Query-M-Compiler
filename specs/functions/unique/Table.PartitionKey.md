# Table.PartitionKey

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | PartitionKey |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `step` |
| Schema-transform hook | No |
| Primary signature | (Table) -> Any |
| Doc | Table.PartitionKey(prev) |

## Behaviour

Table.PartitionKey(prev)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.PartitionKey/](../../conformance/functions/Table.PartitionKey/) (create the folder when adding fixtures).
# Table.ReplacePartitionKey

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ReplacePartitionKey |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,str` |
| Schema-transform hook | No |
| Primary signature | (Table, Text) -> Table |
| Doc | Table.ReplacePartitionKey(prev, key) |

## Behaviour

Table.ReplacePartitionKey(prev, key)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.ReplacePartitionKey/](../../conformance/functions/Table.ReplacePartitionKey/) (create the folder when adding fixtures).
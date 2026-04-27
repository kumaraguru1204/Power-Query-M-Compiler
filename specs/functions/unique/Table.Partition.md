# Table.Partition

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Partition |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,str,int` |
| Schema-transform hook | No |
| Primary signature | (Table, Text, Number) -> List<Table> |
| Doc | Table.Partition(prev, col, groups) |

## Behaviour

Table.Partition(prev, col, groups)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.Partition/](../../conformance/functions/Table.Partition/) (create the folder when adding fixtures).
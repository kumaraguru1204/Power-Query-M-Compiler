# Table.SplitAt

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | SplitAt |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,int` |
| Schema-transform hook | No |
| Primary signature | (Table, Number) -> List<Table> |
| Doc | Table.SplitAt(prev, n) |

## Behaviour

Table.SplitAt(prev, n)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.SplitAt/](../../conformance/functions/Table.SplitAt/) (create the folder when adding fixtures).
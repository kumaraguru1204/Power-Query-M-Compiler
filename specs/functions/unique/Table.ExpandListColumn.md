# Table.ExpandListColumn

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ExpandListColumn |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,str` |
| Schema-transform hook | No |
| Primary signature | (Table, Text) -> Table |
| Doc | Table.ExpandListColumn(prev, col) |

## Behaviour

Table.ExpandListColumn(prev, col)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.ExpandListColumn/](../../conformance/functions/Table.ExpandListColumn/) (create the folder when adding fixtures).
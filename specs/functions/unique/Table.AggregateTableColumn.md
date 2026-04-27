# Table.AggregateTableColumn

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | AggregateTableColumn |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,str,agg` |
| Schema-transform hook | No |
| Primary signature | (Table, Text, List<Any>) -> Table |
| Doc | Table.AggregateTableColumn(prev, col, {{name,each expr,type},...}) |

## Behaviour

Table.AggregateTableColumn(prev, col, {{name,each expr,type},...})

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.AggregateTableColumn/](../../conformance/functions/Table.AggregateTableColumn/) (create the folder when adding fixtures).
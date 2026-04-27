# Table.CombineColumnsToRecord

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | CombineColumnsToRecord |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,cols,str` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, Text) -> Table |
| Doc | Table.CombineColumnsToRecord(prev, {col,...}, newCol) |

## Behaviour

Table.CombineColumnsToRecord(prev, {col,...}, newCol)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.CombineColumnsToRecord/](../../conformance/functions/Table.CombineColumnsToRecord/) (create the folder when adding fixtures).
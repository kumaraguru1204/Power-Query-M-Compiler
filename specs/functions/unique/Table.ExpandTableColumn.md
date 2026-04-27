# Table.ExpandTableColumn

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ExpandTableColumn |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 2 |
| Argument hints | `step|val,val,cols,val?` |
| Schema-transform hook | No |
| Primary signature | (Table, Text, List<Text>) -> Table |
| Doc | Table.ExpandTableColumn(table, column, {col,...}, optional {newCol,...}) |

## Behaviour

Table.ExpandTableColumn(table, column, {col,...}, optional {newCol,...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.ExpandTableColumn/](../../conformance/functions/Table.ExpandTableColumn/) (create the folder when adding fixtures).
# Table.AddFuzzyClusterColumn

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | AddFuzzyClusterColumn |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,str,str,rec?` |
| Schema-transform hook | No |
| Primary signature | (Table, Text, Text) -> Table |
| Doc | Table.AddFuzzyClusterColumn(prev, col, newCol, optional options) |

## Behaviour

Table.AddFuzzyClusterColumn(prev, col, newCol, optional options)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.AddFuzzyClusterColumn/](../../conformance/functions/Table.AddFuzzyClusterColumn/) (create the folder when adding fixtures).
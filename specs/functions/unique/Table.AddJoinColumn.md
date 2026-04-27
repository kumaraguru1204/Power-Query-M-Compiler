# Table.AddJoinColumn

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | AddJoinColumn |
| Required arity | 5 |
| Max arity | 5 |
| Overloads | 1 |
| Argument hints | `step,cols,step,cols,str,join?` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, Table, List<Text>, Text) -> Table |
| Doc | Table.AddJoinColumn(prev, {key}, other, {key}, newCol, optional JoinKind) |

## Behaviour

Table.AddJoinColumn(prev, {key}, other, {key}, newCol, optional JoinKind)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.AddJoinColumn/](../../conformance/functions/Table.AddJoinColumn/) (create the folder when adding fixtures).
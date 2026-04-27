# Table.Contains

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Contains |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,rec,val?` |
| Schema-transform hook | No |
| Primary signature | (Table, Record) -> Boolean |
| Doc | Table.Contains(prev, [col=val,...], optional equationCriteria) |

## Behaviour

Table.Contains(prev, [col=val,...], optional equationCriteria)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.Contains/](../../conformance/functions/Table.Contains/) (create the folder when adding fixtures).
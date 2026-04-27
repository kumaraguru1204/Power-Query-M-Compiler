# Table.PositionOf

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | PositionOf |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,rec` |
| Schema-transform hook | No |
| Primary signature | (Table, Record) -> Number |
| Doc | Table.PositionOf(prev, [col=val,...]) |

## Behaviour

Table.PositionOf(prev, [col=val,...])

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.PositionOf/](../../conformance/functions/Table.PositionOf/) (create the folder when adding fixtures).
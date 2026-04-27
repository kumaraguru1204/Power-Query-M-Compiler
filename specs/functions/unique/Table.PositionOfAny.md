# Table.PositionOfAny

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | PositionOfAny |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,reclist` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Record>) -> List<Number> |
| Doc | Table.PositionOfAny(prev, {[...],...}) |

## Behaviour

Table.PositionOfAny(prev, {[...],...})

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.PositionOfAny/](../../conformance/functions/Table.PositionOfAny/) (create the folder when adding fixtures).
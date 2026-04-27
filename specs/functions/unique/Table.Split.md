# Table.Split

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | Split |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,int` |
| Schema-transform hook | No |
| Primary signature | (Table, Number) -> List<Table> |
| Doc | Table.Split(prev, pageSize) |

## Behaviour

Table.Split(prev, pageSize)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.Split/](../../conformance/functions/Table.Split/) (create the folder when adding fixtures).
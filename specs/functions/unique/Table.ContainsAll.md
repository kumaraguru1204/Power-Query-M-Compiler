# Table.ContainsAll

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ContainsAll |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,reclist,val?` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Record>) -> Boolean |
| Doc | Table.ContainsAll(prev, {[...],...}, optional equationCriteria) |

## Behaviour

Table.ContainsAll(prev, {[...],...}, optional equationCriteria)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.ContainsAll/](../../conformance/functions/Table.ContainsAll/) (create the folder when adding fixtures).
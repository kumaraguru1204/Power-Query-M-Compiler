# Table.ReplaceRows

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ReplaceRows |
| Required arity | 4 |
| Max arity | 4 |
| Overloads | 1 |
| Argument hints | `step,int,int,reclist` |
| Schema-transform hook | No |
| Primary signature | (Table, Number, Number, List<Record>) -> Table |
| Doc | Table.ReplaceRows(prev, offset, count, {row,...}) |

## Behaviour

Table.ReplaceRows(prev, offset, count, {row,...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.ReplaceRows/](../../conformance/functions/Table.ReplaceRows/) (create the folder when adding fixtures).
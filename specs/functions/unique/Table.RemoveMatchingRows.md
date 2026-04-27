# Table.RemoveMatchingRows

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | RemoveMatchingRows |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,reclist` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Record>) -> Table |
| Doc | Table.RemoveMatchingRows(prev, {[...],...}) |

## Behaviour

Table.RemoveMatchingRows(prev, {[...],...})

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.RemoveMatchingRows/](../../conformance/functions/Table.RemoveMatchingRows/) (create the folder when adding fixtures).
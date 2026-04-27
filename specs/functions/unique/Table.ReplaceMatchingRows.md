# Table.ReplaceMatchingRows

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ReplaceMatchingRows |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,reclist,rec` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Record>, Record) -> Table |
| Doc | Table.ReplaceMatchingRows(prev, {[old]}, [new]) |

## Behaviour

Table.ReplaceMatchingRows(prev, {[old]}, [new])

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.ReplaceMatchingRows/](../../conformance/functions/Table.ReplaceMatchingRows/) (create the folder when adding fixtures).
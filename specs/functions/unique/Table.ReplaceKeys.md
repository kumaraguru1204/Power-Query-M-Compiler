# Table.ReplaceKeys

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ReplaceKeys |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 1 |
| Argument hints | `step,cols` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>) -> Table |
| Doc | Table.ReplaceKeys(prev, {key,...}) |

## Behaviour

Table.ReplaceKeys(prev, {key,...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.ReplaceKeys/](../../conformance/functions/Table.ReplaceKeys/) (create the folder when adding fixtures).
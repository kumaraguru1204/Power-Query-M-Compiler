# Table.AddKey

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | AddKey |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,cols,val` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, Any) -> Table |
| Doc | Table.AddKey(prev, {col,...}, isPrimary) |

## Behaviour

Table.AddKey(prev, {col,...}, isPrimary)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.AddKey/](../../conformance/functions/Table.AddKey/) (create the folder when adding fixtures).
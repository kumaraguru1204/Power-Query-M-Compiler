# Table.ConformToPageReader

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | ConformToPageReader |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `step` |
| Schema-transform hook | Yes |
| Primary signature | (Table) -> Table |
| Doc | Table.ConformToPageReader(prev) |

## Behaviour

Table.ConformToPageReader(prev)

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Table.ConformToPageReader/](../../conformance/functions/Table.ConformToPageReader/) (create the folder when adding fixtures).
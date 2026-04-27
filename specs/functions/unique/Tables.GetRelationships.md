# Tables.GetRelationships

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Tables |
| Name | GetRelationships |
| Required arity | 1 |
| Max arity | 1 |
| Overloads | 1 |
| Argument hints | `cols` |
| Schema-transform hook | No |
| Primary signature | (List<Table>) -> Table |
| Doc | Tables.GetRelationships({table,...}) |

## Behaviour

Tables.GetRelationships({table,...})

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/Tables.GetRelationships/](../../conformance/functions/Tables.GetRelationships/) (create the folder when adding fixtures).
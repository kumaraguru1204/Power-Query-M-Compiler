# List.Percentile

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | List |
| Name | Percentile |
| Required arity | 2 |
| Max arity | 2 |
| Overloads | 2 |
| Argument hints | `step|val,val,rec?` |
| Schema-transform hook | No |
| Primary signature | (List<T>, Any) -> Any |
| Doc | List.Percentile(list, percentiles, optional options) -> sample percentile(s) |

## Behaviour

List.Percentile(list, percentiles, optional options) -> sample percentile(s)

## Implementation status

- Executor: No (passthrough or generic).
- SQL emitter: No - falls back to the unsupported placeholder.

## Conformance

Fixtures live under [conformance/functions/List.Percentile/](../../conformance/functions/List.Percentile/) (create the folder when adding fixtures).
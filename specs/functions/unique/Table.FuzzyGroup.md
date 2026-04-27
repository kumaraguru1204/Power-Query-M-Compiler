# Table.FuzzyGroup

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | Table |
| Name | FuzzyGroup |
| Required arity | 3 |
| Max arity | 3 |
| Overloads | 1 |
| Argument hints | `step,cols,agg` |
| Schema-transform hook | No |
| Primary signature | (Table, List<Text>, List<Any>) -> Table |
| Doc | Table.FuzzyGroup(prev, {key,...}, {{name,each expr,type},...}) |

## Behaviour

Table.FuzzyGroup(prev, {key,...}, {{name,each expr,type},...})

## Implementation status

- Executor: Yes (dedicated evaluator).
- SQL emitter: Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).

## Conformance

Fixtures live under [conformance/functions/Table.FuzzyGroup/](../../conformance/functions/Table.FuzzyGroup/) (create the folder when adding fixtures).
# 17. SQL Lowering Templates — Per-Function

> **Status.** Pinned reference of the SQL shapes the engine emits per function. Mirror of [`crates/pq_sql/src/generator.rs`](../../crates/pq_sql/src/generator.rs). When the generator changes, update this file in the same commit.
>
> **Audience.** Anyone reimplementing the engine in another language, or anyone diffing emitted SQL against the spec.
>
> **Notation.** ``{INPUT}`` = the previous step's CTE name. ``{COL}`` = a quoted column identifier. ``{N}`` = an integer literal. ``{EXPR}`` = a lowered M expression (see §3 "Expression lowering"). Each template assumes the step is wrapped in a `WITH STEP_NAME AS ( ... )` CTE by the orchestrator.

---

## 1. Per-step CTE skeleton

Every step lowers to:

```sql
{STEP_NAME} AS (
{TEMPLATE_BODY}
)
```

Steps are emitted in source order; the final query is `SELECT * FROM {OUTPUT_STEP_NAME}` plus any pending `ORDER BY` recorded by `Table.Sort`.

The Source step lowers to `SELECT * FROM "{stem-of-path}"`. NavigateSheet lowers to `SELECT * FROM {input}`. Value bindings lower to `SELECT {EXPR} AS "Value"` (for scalars) or `SELECT {EXPR} AS "Value" UNION ALL ...` (for list literals).

---

## 2. Per-function templates

Each row pins the canonical template. ``y`` in [`16_function_catalogue.md`](16_function_catalogue.md) marks SQL-supported functions; functions not listed here fall through to the unsupported-placeholder fallback (`/* unsupported: name(...) */`).

### Table — row-level

| Function | Template body |
| --- | --- |
| `Table.SelectRows` | `SELECT * FROM {INPUT} WHERE {EXPR}` |
| `Table.RemoveRowsWithErrors` | `SELECT * FROM {INPUT} WHERE {COL} IS NOT NULL [AND ...]` |
| `Table.SelectRowsWithErrors` | `SELECT * FROM {INPUT} WHERE {COL} IS NULL [OR ...]` |
| `Table.MatchesAllRows` | `SELECT NOT EXISTS (SELECT 1 FROM {INPUT} WHERE NOT ({EXPR})) AS "Value"` |
| `Table.MatchesAnyRows` | `SELECT EXISTS (SELECT 1 FROM {INPUT} WHERE {EXPR}) AS "Value"` |
| `Table.FindText` | `SELECT * FROM {INPUT} WHERE LOWER(CAST({COL} AS TEXT)) LIKE '%text%' [OR ...]` |
| `Table.Distinct` | `SELECT DISTINCT [ON ({COLS})] * FROM {INPUT}` |

### Table — row trim / paging

| Function | Template body |
| --- | --- |
| `Table.FirstN` (count) | `SELECT * FROM {INPUT} LIMIT {N}` |
| `Table.FirstN` (predicate) | window-based take-while via `ROW_NUMBER() OVER ()` and a `MIN(_rn) WHERE NOT (pred)` cutoff |
| `Table.LastN` (count) | `SELECT * FROM (SELECT *, ROW_NUMBER() OVER() AS _rn FROM {INPUT}) AS _t WHERE _rn > (SELECT COUNT(*) FROM {INPUT}) - {N}` |
| `Table.LastN` (predicate) | reverse-window take-while (mirror of FirstN) |
| `Table.Skip`, `Table.RemoveFirstN` | `SELECT * FROM {INPUT} OFFSET {N}` |
| `Table.RemoveLastN` | `SELECT * FROM (... _rn ...) WHERE _rn <= (SELECT COUNT(*) FROM {INPUT}) - {N}` |
| `Table.Range` (with count) | `SELECT * FROM {INPUT} LIMIT {COUNT} OFFSET {OFFSET}` |
| `Table.Range` (no count)   | `SELECT * FROM {INPUT} OFFSET {OFFSET}` |
| `Table.RemoveRows` | window-based: `... WHERE _rn < {OFF} OR _rn >= {OFF} + {COUNT}` |
| `Table.AlternateRows` | window-based modulo: `WHERE _rn >= {OFF} AND (_rn - {OFF}) % ({SK} + {TK}) < {TK}` |
| `Table.ReverseRows` | `SELECT * FROM (... _rn ...) ORDER BY _rn DESC` |
| `Table.Repeat` | `SELECT * FROM {INPUT} CROSS JOIN generate_series(1, {N}) AS _g` |
| `Table.Sort` | no body change; sets pending `ORDER BY {COL} [ASC\|DESC] [, ...]` for the final SELECT |

### Table — column shape

| Function | Template body |
| --- | --- |
| `Table.RemoveColumns` | `SELECT {KEPT_COLS} FROM {INPUT}` |
| `Table.SelectColumns` | `SELECT {LISTED_COLS} FROM {INPUT}` (missing columns: `NULL AS {col}` if `MissingField.UseNull`; dropped if `MissingField.Ignore`) |
| `Table.RenameColumns` | `SELECT {OLD} AS {NEW}, ... FROM {INPUT}` |
| `Table.ReorderColumns` | `SELECT {ORDERED_COLS}, {REMAINING_COLS} FROM {INPUT}` |
| `Table.TransformColumnNames` | `SELECT {COL} AS {f({COL})}, ... FROM {INPUT}` |
| `Table.PrefixColumns` | `SELECT {COL} AS "prefix.{COL}", ... FROM {INPUT}` |
| `Table.DemoteHeaders` | `SELECT 'col1' AS Column1, ... UNION ALL SELECT CAST({COL} AS TEXT) AS Column1, ... FROM {INPUT}` |
| `Table.PromoteHeaders` | `SELECT * FROM {INPUT}` *(no-op at SQL level; rename happens in pipeline)* |
| `Table.HasColumns` | `SELECT (col1 IS NOT NULL OR col1 IS NULL) AND ... AS "Value"` *(boolean folding)* |
| `Table.ColumnsOfType` | `SELECT ARRAY[ {col1}, {col2}, ... ] AS "Value"` over columns matching the type list |

### Table — column content

| Function | Template body |
| --- | --- |
| `Table.AddColumn` | `SELECT *, {EXPR} AS {NEW_COL} FROM {INPUT}` |
| `Table.AddIndexColumn` | `SELECT *, (ROW_NUMBER() OVER() - 1) * {STEP} + {START} AS {NEW_COL} FROM {INPUT}` |
| `Table.DuplicateColumn` | `SELECT *, {SRC_COL} AS {NEW_COL} FROM {INPUT}` |
| `Table.FillDown` | `SELECT COALESCE({COL}, LAG({COL} IGNORE NULLS) OVER(ORDER BY _rn)) AS {COL}, ... FROM (... _rn ...)` |
| `Table.FillUp` | as FillDown but with `LEAD(...)` |
| `Table.TransformColumns` | `SELECT {EXPR} AS {COL}, ... FROM {INPUT}` (per-column; default expression applied to columns not in the transform list) |
| `Table.TransformColumnTypes` | `SELECT CAST({COL} AS {SQL_TYPE}) AS {COL}, ... FROM {INPUT}` |
| `Table.ReplaceValue` | `SELECT CASE WHEN {COL} = {OLD} THEN {NEW} ELSE {COL} END AS {COL}, ... FROM {INPUT}` |
| `Table.ReplaceErrorValues` | `SELECT COALESCE({COL}, {DEFAULT}) AS {COL}, ... FROM {INPUT}` |
| `Table.CombineColumns` | `SELECT *, CONCAT_WS({SEP}, {COL1}, {COL2}, ...) AS {NEW_COL} FROM {INPUT}` |
| `Table.SplitColumn` | `SELECT *, SPLIT_PART({COL}, {SEP}, 1) AS {COL}.1, ..., FROM {INPUT}` |
| `Table.ExpandRecordColumn`, `Table.ExpandTableColumn` | per-field cross join: `SELECT t.*, {COL}.{F1} AS {F1}, ... FROM {INPUT} t` |
| `Table.Pivot` | `SELECT {KEY_COLS}, MAX(CASE WHEN {ATTR}='v1' THEN {VAL} END) AS v1, ... FROM {INPUT} GROUP BY {KEY_COLS}` |
| `Table.Unpivot` | `SELECT {KEEP}, '{COL_i}' AS {ATTR}, {COL_i} AS {VAL} FROM {INPUT} UNION ALL ...` (one branch per pivoted column) |
| `Table.UnpivotOtherColumns` | inverse of `Unpivot`: pivoted columns are everything not in the keep list |

### Table — joins / ranking

| Function | Template body |
| --- | --- |
| `Table.Join`, `Table.FuzzyJoin` | `SELECT l.*, r.* FROM {LEFT} l {KIND} JOIN {RIGHT} r ON l.{LK1}=r.{RK1} [AND ...]` where KIND maps `JoinKind.Inner→INNER`, `LeftOuter→LEFT`, `RightOuter→RIGHT`, `FullOuter→FULL`, `LeftAnti→` `WHERE r.{RK} IS NULL` style |
| `Table.NestedJoin`, `Table.FuzzyNestedJoin` | as Join but the right-hand side is collected as an `ARRAY_AGG(...) AS {NEW_COL}` per left key |
| `Table.AddRankColumn` | `SELECT *, RANK() OVER (ORDER BY {SORT}) AS {NEW_COL} FROM {INPUT}` |
| `Table.Max`, `Table.Min` | `SELECT * FROM {INPUT} ORDER BY {COL} DESC\|ASC LIMIT 1` |
| `Table.MaxN`, `Table.MinN` | `SELECT * FROM {INPUT} ORDER BY {COL} DESC\|ASC LIMIT {N}` |
| `Table.Combine` | `SELECT * FROM {STEP1} UNION ALL SELECT * FROM {STEP2} UNION ALL ...` |
| `Table.Group`, `Table.FuzzyGroup` | `SELECT {KEYS}, {AGG_EXPR} AS {AGG_NAME}, ... FROM {INPUT} GROUP BY {KEYS}` |
| `Table.InsertRows` | `SELECT * FROM (... _rn ...) WHERE _rn < {OFF} UNION ALL VALUES (...) UNION ALL ... WHERE _rn >= {OFF}` |

### Table — information / scalars

| Function | Template body |
| --- | --- |
| `Table.RowCount` | `SELECT COUNT(*) AS "Value" FROM {INPUT}` |
| `Table.ColumnCount` | `SELECT {N} AS "Value"` *(N comes from the input schema length)* |
| `Table.ColumnNames` | `SELECT ARRAY['col1','col2',...] AS "Value"` |
| `Table.IsEmpty` | `SELECT (COUNT(*) = 0) AS "Value" FROM {INPUT}` |
| `Table.IsDistinct` | `SELECT (COUNT(*) = COUNT(DISTINCT (col1, col2, ...))) AS "Value" FROM {INPUT}` |
| `Table.Schema` | `SELECT 'col1' AS "Name", 'TYPE' AS "Kind" UNION ALL ...` |
| `Table.Profile` | passthrough `SELECT * FROM {INPUT}` *(profile data only computed at runtime)* |
| `Table.Column` | `SELECT ARRAY_AGG({COL}) AS "Value" FROM {INPUT}` |
| `Table.PartitionValues` | passthrough |
| `Table.Transpose` | `SELECT * FROM {INPUT} /* TRANSPOSE */` *(comment-marked, schema unknown)* |

### Table — construction / conversion

| Function | Template body |
| --- | --- |
| `Table.FromColumns`, `Table.FromList`, `Table.FromRecords`, `Table.FromRows`, `Table.FromValue` | passthrough `SELECT * FROM {SOURCE}` *(materialisation happens in the pipeline)* |
| `Table.ToColumns`, `Table.ToList`, `Table.ToRecords`, `Table.ToRows` | passthrough |
| `Table.Buffer`, `Table.StopFolding`, `Table.ConformToPageReader` | passthrough |

### List — set, higher-order

| Function | Template body |
| --- | --- |
| `List.Generate` | recursive CTE: `WITH RECURSIVE _g(x) AS ( SELECT {INIT} UNION ALL SELECT {NEXT} FROM _g WHERE {COND} ) SELECT ARRAY_AGG(x) FROM _g` |
| `List.Select` | `SELECT ARRAY(SELECT u FROM UNNEST({LIST}) u WHERE {PRED})` |
| `List.Transform` | `SELECT ARRAY(SELECT {EXPR} FROM UNNEST({LIST}) u)` |
| `List.Intersect` | `SELECT ARRAY(SELECT u FROM UNNEST({L1}) u INTERSECT SELECT u FROM UNNEST({L2}) u)` |
| `List.Difference` | `SELECT ARRAY(SELECT u FROM UNNEST({L1}) u EXCEPT SELECT u FROM UNNEST({L2}) u)` |
| `List.RemoveItems` | as `Difference` |

### List — scalar (used inside `Table.Group` aggregates and ad-hoc value bindings)

| Function | Lowering |
| --- | --- |
| `List.Sum` | `SUM(...)` |
| `List.Count` | `COUNT(...)` |
| `List.NonNullCount` | `COUNT(...)` *(SQL `COUNT` already skips NULL)* |
| `List.Average` | `AVG(...)` |
| `List.Min` | `MIN(...)` |
| `List.Max` | `MAX(...)` |
| `List.Median` | `PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY ...)` |
| `List.StandardDeviation` | `STDDEV_SAMP(...)` |
| `List.Covariance` | `COVAR_SAMP(...)` |
| `List.Product` | `EXP(SUM(LN(...)))` |
| `List.Mode`, `List.Modes` | `MODE() WITHIN GROUP (ORDER BY ...)` |
| `List.IsEmpty` | `(COUNT(...) = 0)` |
| `List.AllTrue` | `BOOL_AND(...)` |
| `List.AnyTrue` | `BOOL_OR(...)` |
| `List.Contains` | `({VAL} = ANY(...))` |
| `List.First` | `(...)[1]` |
| `List.Last` | `(...)[ARRAY_LENGTH(..., 1)]` |
| `List.Reverse` | `ARRAY_REVERSE(...)` |
| `List.Sort` | `ARRAY_SORT(...)` |
| `List.Distinct` | `ARRAY_DISTINCT(...)` |
| `List.Positions` | `ARRAY_POSITIONS(...)` |
| `List.Numbers` | `ARRAY(SELECT {start} + i * {step} FROM generate_series(0, {count} - 1) i)` |
| `List.Random` | `(SELECT ARRAY_AGG(random()) FROM generate_series(1, {count}))` |

### Text

| Function | Lowering |
| --- | --- |
| `Text.Length` | `LENGTH(...)` |
| `Text.From` | `CAST(... AS TEXT)` |
| `Text.Upper` | `UPPER(...)` |
| `Text.Lower` | `LOWER(...)` |
| `Text.Trim` | `TRIM(...)` |
| `Text.TrimStart` | `LTRIM(...)` |
| `Text.TrimEnd` | `RTRIM(...)` |
| `Text.Contains` | `(POSITION({sub} IN {text}) > 0)` (or `LOWER(...)` wrapping under `Comparer.OrdinalIgnoreCase`) |
| `Text.StartsWith` | `({text} LIKE {sub} \|\| '%')` |
| `Text.EndsWith` | `({text} LIKE '%' \|\| {sub})` |
| `Text.Replace` | `REPLACE({text}, {old}, {new})` |
| `Text.Range` | `SUBSTRING({text} FROM {offset}+1 FOR {count})` |
| `Text.Split` | `STRING_TO_ARRAY(...)` |
| `Text.Combine` | `STRING_AGG(... [, sep])` or `ARRAY_TO_STRING(list, sep)` |
| `Text.PadStart` | `LPAD({text}, {width}, {pad})` |
| `Text.PadEnd` | `RPAD({text}, {width}, {pad})` |

### Number

| Function | Lowering |
| --- | --- |
| `Number.From` | `CAST(... AS FLOAT)` |
| `Number.Round` | `ROUND(...)` |
| `Number.RoundUp` | `CEIL(...)` |
| `Number.RoundDown` | `FLOOR(...)` |
| `Number.Abs` | `ABS(...)` |
| `Number.Sqrt` | `SQRT(...)` |
| `Number.Power` | `POWER(base, exp)` |
| `Number.Log` | `LN(...)` |
| `Number.Mod` | `MOD(...)` |
| `Number.Sign` | `SIGN(...)` |

### Logical

| Function | Lowering |
| --- | --- |
| `Logical.From` | `CAST(... AS BOOLEAN)` |
| `Logical.Not` | `(NOT ...)` |
| `Logical.And` | `({a} AND {b})` |
| `Logical.Or` | `({a} OR {b})` |
| `Logical.Xor` | `({a} <> {b})` |

---

## 3. Expression lowering

`{EXPR}` in a template means an M expression lowered into SQL by `emit_expr` in [`crates/pq_sql/src/generator.rs`](../../crates/pq_sql/src/generator.rs). The lowering rules:

| M expression | SQL output |
| --- | --- |
| Integer / Float / Boolean / String / Null literal | the equivalent SQL literal |
| Bracketed column access `[Col]` | `"Col"` |
| Identifier in row context | `"Col"` (column) or step name (table reference) |
| `a + b`, `a - b`, `a * b`, `a / b` | `(a + b)` etc. |
| `a & b` | `(a \|\| b)` |
| `a = b`, `<>`, `<`, `<=`, `>`, `>=` | the obvious SQL comparison |
| `a and b`, `a or b`, `not a` | the obvious SQL logical |
| `if c then t else e` | `CASE WHEN c THEN t ELSE e END` |
| `each body` | the `body` is lowered with `_` resolving to the row context |
| function call (`Text.Upper("x")` etc.) | per-function template above |
| any expression with no per-function lowering | `/* unsupported: ... */` placeholder |

---

## 4. Functions intentionally unsupported in SQL

The following functions are accepted by the parser/types but lower to the unsupported placeholder and rely on the executor for runtime semantics. This is *by design* — they have no portable SQL equivalent, or the cost of a faithful lowering exceeds the benefit. See [`16_function_catalogue.md`](16_function_catalogue.md) for the canonical "sql" column.

- `Excel.Workbook` — handled by the workbook ingestion stage; not a SQL operation.
- `Tables.GetRelationships` — model-level metadata, no SQL form.
- `Table.First`, `Table.FirstValue`, `Table.Last`, `Table.SingleRow` — single-row extraction (caller should use `LIMIT 1`).
- `Table.Partition`, `Table.Split`, `Table.SplitAt`, `Table.FromPartitions` — table-of-tables shapes; SQL has no first-class nested table.
- `Table.AddJoinColumn`, `Table.AddFuzzyClusterColumn`, `Table.AddKey`, `Table.Keys`, `Table.PartitionKey`, `Table.ReplaceKeys`, `Table.ReplacePartitionKey` — key/partition metadata.
- `Table.Contains`, `Table.ContainsAll`, `Table.ContainsAny`, `Table.PositionOf`, `Table.PositionOfAny`, `Table.RemoveMatchingRows`, `Table.ReplaceMatchingRows`, `Table.CombineColumnsToRecord`, `Table.AggregateTableColumn`, `Table.ExpandListColumn`, `Table.ApproximateRowCount`, `Table.ReplaceRows` — record-shape pattern matching or specialised structural ops not yet lowered.
- All `List.*` functions not listed in §2 above (the bulk of `List.*` runs in the executor only).

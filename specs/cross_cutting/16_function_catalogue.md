# 16. Function Catalogue - Enumerated

> Status. Auto-generated reference. Regenerate with `cargo run -p pq_grammar --example dump_catalogue`.
>  Total functions: 212. This table is the *extensional* counterpart to SOURCE_OF_TRUTH.md 5.4 (which describes the catalogue *structurally*).

Legend.

- min/max - required-arity / max-arity for the primary signature.
- N - number of overloads (signatures) registered.
- arg hints - the parser per-position argument-shape codes (see legend below).
- schema - `y` if a schema-transform hook is registered.
- exec - `y` if the executor has a dedicated evaluator; blank means parser/types accept it but execution is a passthrough or generic value-binding.
- sql - `y` if the SQL emitter has a dedicated lowering; `-` means the unsupported-placeholder fallback applies.

Argument-hint codes:

| code | meaning |
| --- | --- |
| `str` | string literal |
| `step` | bare step-name reference |
| `each` | `each` lambda body |
| `typelist` | `{{col, type}}` list |
| `cols` | `{col,...}` list |
| `rename` | `{{old, new}}` list |
| `sort` | `{{col, Order.X}}` list |
| `int` | integer literal |
| `val` | arbitrary expression |
| `rec` | record literal |
| `reclist` | `{[...],...}` list |
| `agg` | aggregate-descriptor list |
| `join` | `JoinKind.X` |
| `xform` | transform-pair list |
| `steplist` | `{step,...}` list |
| `filepath` | `File.Contents("...")` shape |
| `nbool?` | optional nullable bool |
| `cols\|str` | bare string or list |
| `missing?` | optional `MissingField.X` |
| `btypelist` | bare `{type X, type Y}` list |
| `culture\|rec?` | optional culture string or record |
| `step\|val` | step ref or any expression |
| trailing `?` | optional |

## Excel (1)

| Function | min | max | N | arg hints | schema | exec | sql | doc |
| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |
| `Excel.Workbook` | 1 | 3 | 1 | `filepath,nbool?,nbool?` |  |  | - | Excel.Workbook(File.Contents(path), optional useHeaders, optional delayTypes) - Returns the contents of the Excel workbook as a table. |

## Tables (1)

| Function | min | max | N | arg hints | schema | exec | sql | doc |
| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |
| `Tables.GetRelationships` | 1 | 1 | 1 | `cols` |  |  | - | Tables.GetRelationships({table,...}) |

## Table (107)

| Function | min | max | N | arg hints | schema | exec | sql | doc |
| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |
| `Table.FromColumns` | 2 | 2 | 1 | `val,cols` |  | y | y | Table.FromColumns({list1,...},{col,...}) |
| `Table.FromList` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | Table.FromList(list, optional each splitter) |
| `Table.FromRecords` | 1 | 1 | 1 | `step\|val` |  | y | y | Table.FromRecords(records) |
| `Table.FromRows` | 2 | 2 | 1 | `val,cols` |  | y | y | Table.FromRows({row,...},{col,...}) |
| `Table.FromValue` | 1 | 1 | 1 | `val,rec?` |  | y | y | Table.FromValue(value, optional [DefaultFieldName=...]) |
| `Table.ToColumns` | 1 | 1 | 1 | `step` |  | y | y | Table.ToColumns(prev) |
| `Table.ToList` | 1 | 1 | 2 | `step,val?` |  | y | y | Table.ToList(prev, optional each combiner) |
| `Table.ToRecords` | 1 | 1 | 1 | `step` |  | y | y | Table.ToRecords(prev) |
| `Table.ToRows` | 1 | 1 | 1 | `step` |  | y | y | Table.ToRows(prev) |
| `Table.ApproximateRowCount` | 1 | 1 | 1 | `step` |  | y | - | Table.ApproximateRowCount(prev) |
| `Table.ColumnCount` | 1 | 1 | 1 | `step` |  | y | y | Table.ColumnCount(prev) |
| `Table.IsEmpty` | 1 | 1 | 1 | `step` |  | y | y | Table.IsEmpty(prev) |
| `Table.PartitionValues` | 1 | 1 | 1 | `step` |  | y | y | Table.PartitionValues(prev) |
| `Table.Profile` | 1 | 1 | 1 | `step` |  | y | y | Table.Profile(prev) |
| `Table.RowCount` | 1 | 1 | 1 | `step\|val` |  | y | y | Table.RowCount(table) - number of rows; accepts a step ref or any table-producing expression |
| `Table.Schema` | 1 | 1 | 1 | `step` |  | y | y | Table.Schema(prev) |
| `Table.AlternateRows` | 4 | 4 | 1 | `step,int,int,int` |  | y | y | Table.AlternateRows(prev, offset, skip, take) |
| `Table.Combine` | 1 | 1 | 1 | `steplist` |  | y | y | Table.Combine({t1,t2,...}) |
| `Table.FindText` | 2 | 2 | 1 | `step,str` |  | y | y | Table.FindText(prev, text) |
| `Table.First` | 2 | 2 | 1 | `step,val` |  |  | - | Table.First(prev, default) |
| `Table.FirstN` | 2 | 2 | 2 | `step\|val,val` | y | y | y | Table.FirstN(prev, countOrCondition) - count (number) or each predicate (take-while) |
| `Table.FirstValue` | 2 | 2 | 1 | `step,val` |  |  | - | Table.FirstValue(prev, default) |
| `Table.FromPartitions` | 2 | 2 | 1 | `str,step` |  | y | - | Table.FromPartitions(col, partitions) |
| `Table.InsertRows` | 3 | 3 | 1 | `step,int,reclist` |  | y | y | Table.InsertRows(prev, offset, {row,...}) |
| `Table.Last` | 2 | 2 | 1 | `step,val` |  |  | - | Table.Last(prev, default) |
| `Table.LastN` | 2 | 2 | 2 | `step\|val,val` | y | y | y | Table.LastN(prev, countOrCondition) - count (number) or each predicate (take-while from bottom) |
| `Table.MatchesAllRows` | 2 | 2 | 1 | `step,each` |  | y | y | Table.MatchesAllRows(prev, each pred) |
| `Table.MatchesAnyRows` | 2 | 2 | 1 | `step,each` |  | y | y | Table.MatchesAnyRows(prev, each pred) |
| `Table.Partition` | 3 | 3 | 1 | `step,str,int` |  |  | - | Table.Partition(prev, col, groups) |
| `Table.Range` | 2 | 2 | 2 | `step\|val,val,val?` | y | y | y | Table.Range(prev, offset, optional count) - count omitted/null means all rows after offset |
| `Table.RemoveFirstN` | 2 | 2 | 1 | `step,int` |  | y | y | Table.RemoveFirstN(prev, n) |
| `Table.RemoveLastN` | 2 | 2 | 1 | `step,int` |  | y | y | Table.RemoveLastN(prev, n) |
| `Table.RemoveRows` | 3 | 3 | 1 | `step,int,int` |  | y | y | Table.RemoveRows(prev, offset, count) |
| `Table.RemoveRowsWithErrors` | 2 | 2 | 1 | `step,cols` | y | y | y | Table.RemoveRowsWithErrors(prev, {col,...}) |
| `Table.Repeat` | 2 | 2 | 1 | `step,int` |  | y | y | Table.Repeat(prev, n) |
| `Table.ReplaceRows` | 4 | 4 | 1 | `step,int,int,reclist` |  | y | - | Table.ReplaceRows(prev, offset, count, {row,...}) |
| `Table.ReverseRows` | 1 | 1 | 1 | `step` | y | y | y | Table.ReverseRows(prev) |
| `Table.SelectRows` | 2 | 2 | 1 | `step\|val,each` | y | y | y | Table.SelectRows(prev, each predicate) |
| `Table.SelectRowsWithErrors` | 2 | 2 | 1 | `step,cols` | y | y | y | Table.SelectRowsWithErrors(prev, {col,...}) |
| `Table.SingleRow` | 1 | 1 | 1 | `step` |  | y | - | Table.SingleRow(prev) |
| `Table.Skip` | 2 | 2 | 1 | `step,int` |  | y | y | Table.Skip(prev, n) |
| `Table.SplitAt` | 2 | 2 | 1 | `step,int` |  |  | - | Table.SplitAt(prev, n) |
| `Table.Column` | 2 | 2 | 1 | `step,str` |  | y | y | Table.Column(prev, col) -> List<T> |
| `Table.ColumnNames` | 1 | 1 | 1 | `step\|val` |  | y | y | Table.ColumnNames(table) -- list of column names; accepts a step ref or any table-producing expression |
| `Table.ColumnsOfType` | 2 | 2 | 1 | `step\|val,btypelist` |  | y | y | Table.ColumnsOfType(prev, {type,...}) |
| `Table.DemoteHeaders` | 1 | 1 | 1 | `step` |  | y | y | Table.DemoteHeaders(prev) |
| `Table.DuplicateColumn` | 3 | 3 | 1 | `step,str,str` | y | y | y | Table.DuplicateColumn(prev, col, newCol) |
| `Table.HasColumns` | 2 | 2 | 2 | `step,cols\|str` |  | y | y | Table.HasColumns(prev, {col,...} or "col") |
| `Table.Pivot` | 4 | 4 | 1 | `step,cols,str,str` |  | y | y | Table.Pivot(prev, {val,...}, attrCol, valCol) |
| `Table.PrefixColumns` | 2 | 2 | 1 | `step,str` |  | y | y | Table.PrefixColumns(prev, prefix) |
| `Table.PromoteHeaders` | 1 | 1 | 1 | `step` |  | y | y | Table.PromoteHeaders(prev) |
| `Table.RemoveColumns` | 2 | 2 | 4 | `step,cols\|str,missing?` | y | y | y | Table.RemoveColumns(prev, {col,...}, optional MissingField.X) |
| `Table.ReorderColumns` | 2 | 2 | 1 | `step,cols` | y | y | y | Table.ReorderColumns(prev, {col,...}) |
| `Table.RenameColumns` | 2 | 2 | 1 | `step,rename` | y | y | y | Table.RenameColumns(prev, {{old, new},...}) |
| `Table.SelectColumns` | 2 | 2 | 4 | `step,cols\|str,missing?` | y | y | y | Table.SelectColumns(prev, {col,...}, optional MissingField.X) |
| `Table.TransformColumnNames` | 2 | 2 | 1 | `step,each` |  | y | y | Table.TransformColumnNames(prev, each expr) |
| `Table.Unpivot` | 4 | 4 | 1 | `step,cols,str,str` |  | y | y | Table.Unpivot(prev, {col,...}, attrCol, valCol) |
| `Table.UnpivotOtherColumns` | 4 | 4 | 1 | `step,cols,str,str` |  | y | y | Table.UnpivotOtherColumns(prev, {col,...}, attrCol, valCol) |
| `Table.AddColumn` | 3 | 3 | 2 | `step,str,each,val?` | y | y | y | Table.AddColumn(prev, name, each expr, optional type) |
| `Table.AddFuzzyClusterColumn` | 3 | 3 | 1 | `step,str,str,rec?` |  |  | - | Table.AddFuzzyClusterColumn(prev, col, newCol, optional options) |
| `Table.AddIndexColumn` | 2 | 2 | 3 | `step,str,int?,int?` | y | y | y | Table.AddIndexColumn(prev, newCol, optional start, optional step) |
| `Table.AddJoinColumn` | 5 | 5 | 1 | `step,cols,step,cols,str,join?` |  |  | - | Table.AddJoinColumn(prev, {key}, other, {key}, newCol, optional JoinKind) |
| `Table.AddKey` | 3 | 3 | 1 | `step,cols,val` |  | y | - | Table.AddKey(prev, {col,...}, isPrimary) |
| `Table.AggregateTableColumn` | 3 | 3 | 1 | `step,str,agg` |  |  | - | Table.AggregateTableColumn(prev, col, {{name,each expr,type},...}) |
| `Table.CombineColumns` | 4 | 4 | 1 | `step,cols,each,str` |  | y | y | Table.CombineColumns(prev, {col,...}, each combiner, newCol) |
| `Table.CombineColumnsToRecord` | 3 | 3 | 1 | `step,cols,str` |  |  | - | Table.CombineColumnsToRecord(prev, {col,...}, newCol) |
| `Table.ExpandListColumn` | 2 | 2 | 1 | `step,str` |  |  | - | Table.ExpandListColumn(prev, col) |
| `Table.ExpandRecordColumn` | 3 | 3 | 1 | `step,str,cols` |  | y | y | Table.ExpandRecordColumn(prev, col, {field,...}) |
| `Table.ExpandTableColumn` | 3 | 3 | 2 | `step\|val,val,cols,val?` |  | y | y | Table.ExpandTableColumn(table, column, {col,...}, optional {newCol,...}) |
| `Table.FillDown` | 2 | 2 | 1 | `step,cols` | y | y | y | Table.FillDown(prev, {col,...}) |
| `Table.FillUp` | 2 | 2 | 1 | `step,cols` | y | y | y | Table.FillUp(prev, {col,...}) |
| `Table.FuzzyGroup` | 3 | 3 | 1 | `step,cols,agg` |  | y | y | Table.FuzzyGroup(prev, {key,...}, {{name,each expr,type},...}) |
| `Table.FuzzyJoin` | 5 | 5 | 1 | `step,cols,step,cols,join` |  | y | y | Table.FuzzyJoin(prev, {key}, other, {key}, JoinKind.X) |
| `Table.FuzzyNestedJoin` | 6 | 6 | 1 | `step,cols,step,cols,str,join` |  | y | y | Table.FuzzyNestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X) |
| `Table.Group` | 3 | 3 | 4 | `step\|val,cols\|str,agg,val?,val?` |  | y | y | Table.Group(prev, key\|{key,...}, {{name,each expr,type},...} or {name,each expr,type}, optional groupKind, optional comparer) |
| `Table.Join` | 5 | 5 | 1 | `step,cols,step,cols,join` |  | y | y | Table.Join(prev, {key}, other, {key}, JoinKind.X) |
| `Table.Keys` | 1 | 1 | 1 | `step` |  | y | - | Table.Keys(prev) |
| `Table.NestedJoin` | 6 | 6 | 1 | `step,cols,step,cols,str,join` |  | y | y | Table.NestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X) |
| `Table.PartitionKey` | 1 | 1 | 1 | `step` |  | y | - | Table.PartitionKey(prev) |
| `Table.ReplaceErrorValues` | 2 | 2 | 1 | `step,xform` |  | y | y | Table.ReplaceErrorValues(prev, {{col,value},...}) |
| `Table.ReplaceKeys` | 2 | 2 | 1 | `step,cols` |  | y | - | Table.ReplaceKeys(prev, {key,...}) |
| `Table.ReplacePartitionKey` | 2 | 2 | 1 | `step,str` |  | y | - | Table.ReplacePartitionKey(prev, key) |
| `Table.ReplaceValue` | 4 | 4 | 1 | `step,val,val,each` | y | y | y | Table.ReplaceValue(prev, old, new, each replacer) |
| `Table.Sort` | 2 | 2 | 1 | `step,sort` | y | y | y | Table.Sort(prev, {{col, Order.Ascending},...}) |
| `Table.Split` | 2 | 2 | 1 | `step,int` |  |  | - | Table.Split(prev, pageSize) |
| `Table.SplitColumn` | 3 | 3 | 1 | `step,str,each` |  | y | y | Table.SplitColumn(prev, col, each splitter) |
| `Table.TransformColumns` | 2 | 2 | 2 | `step\|val,xform,val?,missing?` | y | y | y | Table.TransformColumns(prev, {{col,each expr},...}, optional defaultTransform, optional missingField) |
| `Table.TransformColumnTypes` | 2 | 2 | 2 | `step\|val,typelist,culture\|rec?` | y | y | y | Table.TransformColumnTypes(prev, {{col,type},...}, optional culture) |
| `Table.TransformRows` | 2 | 2 | 1 | `step,each` | y | y | y | Table.TransformRows(prev, each expr) |
| `Table.Transpose` | 1 | 1 | 1 | `step` |  | y | y | Table.Transpose(prev) |
| `Table.Contains` | 2 | 2 | 1 | `step,rec,val?` |  |  | - | Table.Contains(prev, [col=val,...], optional equationCriteria) |
| `Table.ContainsAll` | 2 | 2 | 1 | `step,reclist,val?` |  |  | - | Table.ContainsAll(prev, {[...],...}, optional equationCriteria) |
| `Table.ContainsAny` | 2 | 2 | 1 | `step,reclist` |  |  | - | Table.ContainsAny(prev, {[...],...}) |
| `Table.Distinct` | 2 | 2 | 1 | `step,cols` | y | y | y | Table.Distinct(prev, {col,...}) |
| `Table.IsDistinct` | 1 | 1 | 1 | `step` |  | y | y | Table.IsDistinct(prev) |
| `Table.PositionOf` | 2 | 2 | 1 | `step,rec` |  |  | - | Table.PositionOf(prev, [col=val,...]) |
| `Table.PositionOfAny` | 2 | 2 | 1 | `step,reclist` |  |  | - | Table.PositionOfAny(prev, {[...],...}) |
| `Table.RemoveMatchingRows` | 2 | 2 | 1 | `step,reclist` |  |  | - | Table.RemoveMatchingRows(prev, {[...],...}) |
| `Table.ReplaceMatchingRows` | 3 | 3 | 1 | `step,reclist,rec` |  |  | - | Table.ReplaceMatchingRows(prev, {[old]}, [new]) |
| `Table.AddRankColumn` | 3 | 3 | 1 | `step,str,sort,rec?` |  | y | y | Table.AddRankColumn(prev, newCol, {{col,Order.X},...}, optional options) |
| `Table.Max` | 3 | 3 | 1 | `step,str,val` |  | y | y | Table.Max(prev, col, default) |
| `Table.MaxN` | 3 | 3 | 1 | `step,int,str` |  | y | y | Table.MaxN(prev, n, col) |
| `Table.Min` | 3 | 3 | 1 | `step,str,val` |  | y | y | Table.Min(prev, col, default) |
| `Table.MinN` | 3 | 3 | 1 | `step,int,str` |  | y | y | Table.MinN(prev, n, col) |
| `Table.Buffer` | 1 | 1 | 1 | `step` | y | y | - | Table.Buffer(prev) |
| `Table.ConformToPageReader` | 1 | 1 | 1 | `step` | y | y | - | Table.ConformToPageReader(prev) |
| `Table.StopFolding` | 1 | 1 | 1 | `step` | y | y | - | Table.StopFolding(prev) |

## List (70)

| Function | min | max | N | arg hints | schema | exec | sql | doc |
| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |
| `List.Count` | 1 | 1 | 1 | `step\|val` |  | y | y | List.Count(list) -> number of items in list |
| `List.IsEmpty` | 1 | 1 | 1 | `step\|val` |  | y | y | List.IsEmpty(list) -> true if list contains no items |
| `List.NonNullCount` | 1 | 1 | 1 | `step\|val` |  | y | y | List.NonNullCount(list) -> number of non-null items in list |
| `List.Alternate` | 3 | 3 | 2 | `step\|val,int,int,int?` |  |  | - | List.Alternate(list, skip, take, optional offset) -> odd-numbered offset elements |
| `List.Buffer` | 1 | 1 | 1 | `step\|val` |  |  | - | List.Buffer(list) -> buffers a list in memory |
| `List.Distinct` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Distinct(list, optional equationCriteria) -> list with duplicates removed |
| `List.FindText` | 2 | 2 | 1 | `step\|val,str` |  |  | - | List.FindText(list, text) -> values (including record fields) containing text |
| `List.First` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.First(list, optional default) -> first value or default if empty |
| `List.FirstN` | 2 | 2 | 1 | `step\|val,val` |  |  | - | List.FirstN(list, countOrCondition) -> first N items or items matching condition |
| `List.InsertRange` | 3 | 3 | 1 | `step\|val,int,step\|val` |  |  | - | List.InsertRange(list, index, values) -> list with values inserted at index |
| `List.IsDistinct` | 1 | 1 | 2 | `step\|val,val?` |  |  | - | List.IsDistinct(list, optional equationCriteria) -> true if no duplicates in list |
| `List.Last` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Last(list, optional default) -> last value or default if empty |
| `List.LastN` | 2 | 2 | 1 | `step\|val,val` |  |  | - | List.LastN(list, countOrCondition) -> last N items or items matching condition |
| `List.MatchesAll` | 2 | 2 | 1 | `step\|val,each` |  |  | - | List.MatchesAll(list, condition) -> true if all values satisfy condition |
| `List.MatchesAny` | 2 | 2 | 1 | `step\|val,each` |  |  | - | List.MatchesAny(list, condition) -> true if any value satisfies condition |
| `List.Positions` | 1 | 1 | 1 | `step\|val` |  | y | y | List.Positions(list) -> list of offsets for the input list |
| `List.Range` | 2 | 2 | 2 | `step\|val,int,int?` |  |  | - | List.Range(list, offset, optional count) -> subset of list beginning at offset |
| `List.Select` | 2 | 2 | 1 | `step\|val,each` |  | y | y | List.Select(list, each predicate) -> values matching condition |
| `List.Single` | 1 | 1 | 1 | `step\|val` |  |  | - | List.Single(list) -> the one item in a single-element list; error otherwise |
| `List.SingleOrDefault` | 1 | 1 | 2 | `step\|val,val?` |  |  | - | List.SingleOrDefault(list, optional default) -> single item or default for empty list |
| `List.Skip` | 2 | 2 | 1 | `step\|val,val` |  |  | - | List.Skip(list, countOrCondition) -> list with leading items removed |
| `List.Accumulate` | 3 | 3 | 1 | `step\|val,val,each` |  |  | - | List.Accumulate(list, seed, (acc, x) => expr) -> accumulated summary value |
| `List.Combine` | 1 | 1 | 1 | `step\|val` |  | y | - | List.Combine({list1, list2, ...}) -> single combined list |
| `List.RemoveFirstN` | 2 | 2 | 1 | `step\|val,val` |  |  | - | List.RemoveFirstN(list, countOrCondition) -> list with first N elements removed |
| `List.RemoveItems` | 2 | 2 | 1 | `step\|val,step\|val` |  | y | y | List.RemoveItems(list1, list2) -> list1 minus items present in list2 |
| `List.RemoveLastN` | 2 | 2 | 1 | `step\|val,val` |  |  | - | List.RemoveLastN(list, countOrCondition) -> list with last N elements removed |
| `List.RemoveMatchingItems` | 2 | 2 | 2 | `step\|val,step\|val,val?` |  |  | - | List.RemoveMatchingItems(list, values, optional equationCriteria) -> all occurrences removed |
| `List.RemoveNulls` | 1 | 1 | 1 | `step\|val` |  |  | - | List.RemoveNulls(list) -> list with all null values removed |
| `List.RemoveRange` | 2 | 2 | 2 | `step\|val,int,int?` |  |  | - | List.RemoveRange(list, index, optional count) -> list with range of values removed |
| `List.Repeat` | 2 | 2 | 1 | `step\|val,int` |  | y | - | List.Repeat(list, count) -> list repeated count times |
| `List.ReplaceMatchingItems` | 2 | 2 | 2 | `step\|val,step\|val,val?` |  |  | - | List.ReplaceMatchingItems(list, replacements, optional equationCriteria) -> replacements applied |
| `List.ReplaceRange` | 4 | 4 | 1 | `step\|val,int,int,step\|val` |  |  | - | List.ReplaceRange(list, index, count, replaceWith) -> list with range replaced |
| `List.ReplaceValue` | 4 | 4 | 1 | `step\|val,val,val,each` |  |  | - | List.ReplaceValue(list, oldValue, newValue, replacer) -> list with value replaced |
| `List.Reverse` | 1 | 1 | 1 | `step\|val` |  | y | y | List.Reverse(list) -> list in reversed order |
| `List.Split` | 2 | 2 | 1 | `step\|val,int` |  |  | - | List.Split(list, pageSize) -> list of sub-lists each of length pageSize |
| `List.Transform` | 2 | 2 | 1 | `step\|val,each` |  | y | y | List.Transform(list, each expr) -> new list of transformed values |
| `List.TransformMany` | 3 | 3 | 1 | `step\|val,each,each` |  |  | - | List.TransformMany(list, listTransform, resultTransform) -> flattened transformed list |
| `List.Zip` | 1 | 1 | 1 | `step\|val` |  |  | - | List.Zip({list1, list2, ...}) -> list of lists combining items at same position |
| `List.AllTrue` | 1 | 1 | 1 | `step\|val` |  | y | y | List.AllTrue(list) -> true if all expressions are true |
| `List.AnyTrue` | 1 | 1 | 1 | `step\|val` |  | y | y | List.AnyTrue(list) -> true if any expression is true |
| `List.Contains` | 2 | 2 | 2 | `step\|val,val,val?` |  | y | y | List.Contains(list, value, optional equationCriteria) -> true if list contains value |
| `List.ContainsAll` | 2 | 2 | 2 | `step\|val,step\|val,val?` |  |  | - | List.ContainsAll(list, values, optional equationCriteria) -> true if list includes all values |
| `List.ContainsAny` | 2 | 2 | 2 | `step\|val,step\|val,val?` |  |  | - | List.ContainsAny(list, values, optional equationCriteria) -> true if list includes any value |
| `List.PositionOf` | 2 | 2 | 2 | `step\|val,val,val?,val?` |  |  | - | List.PositionOf(list, value, optional occurrence, optional equationCriteria) -> offset(s) |
| `List.PositionOfAny` | 2 | 2 | 2 | `step\|val,step\|val,val?,val?` |  |  | - | List.PositionOfAny(list, values, optional occurrence, optional equationCriteria) -> first offset |
| `List.Difference` | 2 | 2 | 2 | `step\|val,step\|val,val?` |  | y | y | List.Difference(list1, list2, optional equationCriteria) -> items in list1 not in list2 |
| `List.Intersect` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Intersect(lists, optional equationCriteria) -> intersection of list values |
| `List.Union` | 1 | 1 | 2 | `step\|val,val?` |  |  | - | List.Union(lists, optional equationCriteria) -> union of list values |
| `List.Max` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Max(list, optional default) -> maximum value or default for empty list |
| `List.MaxN` | 2 | 2 | 2 | `step\|val,val,val?` |  |  | - | List.MaxN(list, countOrCondition, optional comparisonCriteria) -> maximum N values |
| `List.Median` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Median(list, optional comparisonCriteria) -> median value in list |
| `List.Min` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Min(list, optional default) -> minimum value or default for empty list |
| `List.MinN` | 2 | 2 | 2 | `step\|val,val,val?` |  |  | - | List.MinN(list, countOrCondition, optional comparisonCriteria) -> minimum N values |
| `List.Percentile` | 2 | 2 | 2 | `step\|val,val,rec?` |  |  | - | List.Percentile(list, percentiles, optional options) -> sample percentile(s) |
| `List.Sort` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Sort(list, optional comparisonCriteria) -> sorted list |
| `List.Average` | 1 | 1 | 1 | `step\|val` |  | y | y | List.Average(list) -> average of number/date/datetime/datetimezone/duration values |
| `List.Mode` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Mode(list, optional equationCriteria) -> most frequently occurring value |
| `List.Modes` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | List.Modes(list, optional equationCriteria) -> list of most frequently occurring values |
| `List.StandardDeviation` | 1 | 1 | 1 | `step\|val` |  | y | y | List.StandardDeviation(list) -> sample-based standard deviation estimate |
| `List.Sum` | 1 | 1 | 1 | `step\|val` |  | y | y | List.Sum(list) -> sum of number or duration items |
| `List.Covariance` | 2 | 2 | 1 | `step\|val,step\|val` |  | y | y | List.Covariance(list1, list2) -> covariance between two number lists |
| `List.Product` | 1 | 1 | 1 | `step\|val` |  | y | y | List.Product(list) -> product of numbers in list |
| `List.Dates` | 3 | 3 | 1 | `val,int,val` |  |  | - | List.Dates(start, count, step) -> list of date values |
| `List.DateTimes` | 3 | 3 | 1 | `val,int,val` |  |  | - | List.DateTimes(start, count, step) -> list of datetime values |
| `List.DateTimeZones` | 3 | 3 | 1 | `val,int,val` |  |  | - | List.DateTimeZones(start, count, step) -> list of datetimezone values |
| `List.Durations` | 3 | 3 | 1 | `val,int,val` |  |  | - | List.Durations(start, count, step) -> list of duration values |
| `List.Generate` | 3 | 3 | 2 | `val,each,each,val?` |  | y | y | List.Generate(initial, condition, next, optional selector) -> generated list |
| `List.Numbers` | 2 | 2 | 2 | `val,int,val?` |  | y | y | List.Numbers(start, count, optional increment) -> list of numbers |
| `List.Random` | 1 | 1 | 2 | `int,val?` |  | y | y | List.Random(count, optional seed) -> list of random numbers between 0 and 1 |
| `List.Times` | 3 | 3 | 1 | `val,int,val` |  |  | - | List.Times(start, count, step) -> list of time values |

## Text (18)

| Function | min | max | N | arg hints | schema | exec | sql | doc |
| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |
| `Text.Length` | 1 | 1 | 1 | `val` |  | y | y | Text.Length(text) |
| `Text.From` | 1 | 1 | 1 | `val` |  | y | y | Text.From(value) |
| `Text.Upper` | 1 | 1 | 1 | `val` |  | y | y | Text.Upper(text) |
| `Text.Lower` | 1 | 1 | 1 | `val` |  | y | y | Text.Lower(text) |
| `Text.Trim` | 1 | 1 | 1 | `val` |  | y | y | Text.Trim(text) |
| `Text.TrimStart` | 1 | 1 | 1 | `val` |  | y | y | Text.TrimStart(text) |
| `Text.TrimEnd` | 1 | 1 | 1 | `val` |  | y | y | Text.TrimEnd(text) |
| `Text.PadStart` | 3 | 3 | 1 | `val,int,str` |  | y | y | Text.PadStart(text,width,pad) |
| `Text.PadEnd` | 3 | 3 | 1 | `val,int,str` |  | y | y | Text.PadEnd(text,width,pad) |
| `Text.Contains` | 2 | 2 | 2 | `val,val,val?` |  | y | y | Text.Contains(text, substring, optional comparer) |
| `Text.StartsWith` | 2 | 2 | 2 | `val,val,val?` |  | y | y | Text.StartsWith(text, substring, optional comparer) |
| `Text.EndsWith` | 2 | 2 | 2 | `val,val,val?` |  | y | y | Text.EndsWith(text, substring, optional comparer) |
| `Text.Start` | 2 | 2 | 1 | `val,int` |  | y | - | Text.Start(text, count) |
| `Text.End` | 2 | 2 | 1 | `val,int` |  | y | - | Text.End(text, count) |
| `Text.Range` | 3 | 3 | 1 | `val,int,int` |  | y | y | Text.Range(text,offset,count) |
| `Text.Replace` | 3 | 3 | 1 | `val,str,str` |  | y | y | Text.Replace(text,old,new) |
| `Text.Split` | 2 | 2 | 1 | `val,str` |  | y | y | Text.Split(text,delimiter) |
| `Text.Combine` | 1 | 1 | 2 | `step\|val,val?` |  | y | y | Text.Combine(list) or Text.Combine(list, separator) |

## Number (10)

| Function | min | max | N | arg hints | schema | exec | sql | doc |
| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |
| `Number.From` | 1 | 1 | 1 | `val` |  | y | y | Number.From(value) |
| `Number.Round` | 1 | 1 | 3 | `val,int?,int?` |  | y | y | Number.Round(n), Number.Round(n, digits), Number.Round(n, digits, mode) |
| `Number.RoundUp` | 1 | 1 | 1 | `val` |  | y | y | Number.RoundUp(n) |
| `Number.RoundDown` | 1 | 1 | 1 | `val` |  | y | y | Number.RoundDown(n) |
| `Number.Abs` | 1 | 1 | 1 | `val` |  | y | y | Number.Abs(n) |
| `Number.Sqrt` | 1 | 1 | 1 | `val` |  | y | y | Number.Sqrt(n) |
| `Number.Power` | 2 | 2 | 1 | `val,val` |  | y | y | Number.Power(base,exponent) |
| `Number.Log` | 1 | 1 | 1 | `val` |  | y | y | Number.Log(n) |
| `Number.Mod` | 2 | 2 | 1 | `val,val` |  | y | y | Number.Mod(n,divisor) |
| `Number.Sign` | 1 | 1 | 1 | `val` |  | y | y | Number.Sign(n) |

## Logical (5)

| Function | min | max | N | arg hints | schema | exec | sql | doc |
| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |
| `Logical.From` | 1 | 1 | 1 | `val` |  | y | y | Logical.From(value) |
| `Logical.Not` | 1 | 1 | 1 | `val` |  | y | y | Logical.Not(b) |
| `Logical.And` | 2 | 2 | 1 | `val,val` |  | y | y | Logical.And(a,b) |
| `Logical.Or` | 2 | 2 | 1 | `val,val` |  | y | y | Logical.Or(a,b) |
| `Logical.Xor` | 2 | 2 | 1 | `val,val` |  | y | y | Logical.Xor(a,b) |


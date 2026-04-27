# Function Families - Taxonomy and Index

This file is the master index of every M function the system supports. For each function it names the family the function belongs to and links to the right place to read its specification.

> Auto-synced from the actual function catalogue (see [`cross_cutting/16_function_catalogue.md`](../cross_cutting/16_function_catalogue.md)). Total: 212 functions.

---

## 1. The twelve families

| Id  | Family                          | Spec file                             | Per-function files? |
| --- | ------------------------------- | ------------------------------------- | ------------------- |
| F01 | Pure scalar unary               | F01_pure_scalar_unary.md              | No (table only)     |
| F02 | Binary text predicate           | F02_binary_text_predicate.md          | No (table only)     |
| F03 | List unary aggregate            | F03_list_unary_aggregate.md           | No (table only)     |
| F04 | List set operation              | F04_list_set_operation.md             | Thin leaves         |
| F05 | List higher-order               | F05_list_higher_order.md              | Thin leaves         |
| F06 | Table row trim                  | F06_table_row_trim.md                 | No (table only)     |
| F07 | Table row filter                | F07_table_row_filter.md               | Thin leaves         |
| F08 | Table column shape              | F08_table_column_shape.md             | No (table only)     |
| F09 | Table column content            | F09_table_column_content.md           | Thin leaves         |
| F10 | Table aggregation               | F10_table_aggregation.md              | Thin leaves         |
| F11 | Workbook entry                  | F11_workbook_entry.md                 | Full spec           |
| F12 | List construction               | F12_list_construction.md              | No (table only)     |

A row marked Unc. (Unclassified) lives as a stand-alone spec under `functions/unique/`.

---

## 2. The function-to-family assignment

Functions are grouped by namespace, then alphabetical.

### Excel namespace (1)

| Function | Family | Spec location |
| -------- | ------ | ------------- |
| `Excel.Workbook` | F11 | functions/unique/Excel.Workbook.md |

### Tables namespace (1)

| Function | Family | Spec location |
| -------- | ------ | ------------- |
| `Tables.GetRelationships` | Unc. | functions/unique/Tables.GetRelationships.md |

### Table namespace (107)

| Function | Family | Spec location |
| -------- | ------ | ------------- |
| `Table.AddColumn` | F09 | functions/table/Table.AddColumn.md |
| `Table.AddFuzzyClusterColumn` | Unc. | functions/unique/Table.AddFuzzyClusterColumn.md |
| `Table.AddIndexColumn` | F09 | functions/table/Table.AddIndexColumn.md |
| `Table.AddJoinColumn` | Unc. | functions/unique/Table.AddJoinColumn.md |
| `Table.AddKey` | Unc. | functions/unique/Table.AddKey.md |
| `Table.AddRankColumn` | Unc. | functions/unique/Table.AddRankColumn.md |
| `Table.AggregateTableColumn` | Unc. | functions/unique/Table.AggregateTableColumn.md |
| `Table.AlternateRows` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.ApproximateRowCount` | F10 | functions/table/Table.ApproximateRowCount.md |
| `Table.Buffer` | Unc. | functions/unique/Table.Buffer.md |
| `Table.Column` | F10 | functions/table/Table.Column.md |
| `Table.ColumnCount` | F10 | functions/table/Table.ColumnCount.md |
| `Table.ColumnNames` | F10 | functions/table/Table.ColumnNames.md |
| `Table.ColumnsOfType` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.Combine` | Unc. | functions/unique/Table.Combine.md |
| `Table.CombineColumns` | F09 | functions/table/Table.CombineColumns.md |
| `Table.CombineColumnsToRecord` | Unc. | functions/unique/Table.CombineColumnsToRecord.md |
| `Table.ConformToPageReader` | Unc. | functions/unique/Table.ConformToPageReader.md |
| `Table.Contains` | Unc. | functions/unique/Table.Contains.md |
| `Table.ContainsAll` | Unc. | functions/unique/Table.ContainsAll.md |
| `Table.ContainsAny` | Unc. | functions/unique/Table.ContainsAny.md |
| `Table.DemoteHeaders` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.Distinct` | F07 | functions/table/Table.Distinct.md |
| `Table.DuplicateColumn` | F09 | functions/table/Table.DuplicateColumn.md |
| `Table.ExpandListColumn` | Unc. | functions/unique/Table.ExpandListColumn.md |
| `Table.ExpandRecordColumn` | Unc. | functions/unique/Table.ExpandRecordColumn.md |
| `Table.ExpandTableColumn` | Unc. | functions/unique/Table.ExpandTableColumn.md |
| `Table.FillDown` | F09 | functions/table/Table.FillDown.md |
| `Table.FillUp` | F09 | functions/table/Table.FillUp.md |
| `Table.FindText` | F07 | functions/table/Table.FindText.md |
| `Table.First` | F10 | functions/table/Table.First.md |
| `Table.FirstN` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.FirstValue` | F10 | functions/table/Table.FirstValue.md |
| `Table.FromColumns` | Unc. | functions/unique/Table.FromColumns.md |
| `Table.FromList` | Unc. | functions/unique/Table.FromList.md |
| `Table.FromPartitions` | Unc. | functions/unique/Table.FromPartitions.md |
| `Table.FromRecords` | Unc. | functions/unique/Table.FromRecords.md |
| `Table.FromRows` | Unc. | functions/unique/Table.FromRows.md |
| `Table.FromValue` | Unc. | functions/unique/Table.FromValue.md |
| `Table.FuzzyGroup` | Unc. | functions/unique/Table.FuzzyGroup.md |
| `Table.FuzzyJoin` | Unc. | functions/unique/Table.FuzzyJoin.md |
| `Table.FuzzyNestedJoin` | Unc. | functions/unique/Table.FuzzyNestedJoin.md |
| `Table.Group` | Unc. | functions/unique/Table.Group.md |
| `Table.HasColumns` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.InsertRows` | Unc. | functions/unique/Table.InsertRows.md |
| `Table.IsDistinct` | F10 | functions/table/Table.IsDistinct.md |
| `Table.IsEmpty` | F10 | functions/table/Table.IsEmpty.md |
| `Table.Join` | Unc. | functions/unique/Table.Join.md |
| `Table.Keys` | Unc. | functions/unique/Table.Keys.md |
| `Table.Last` | F10 | functions/table/Table.Last.md |
| `Table.LastN` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.MatchesAllRows` | F07 | functions/table/Table.MatchesAllRows.md |
| `Table.MatchesAnyRows` | F07 | functions/table/Table.MatchesAnyRows.md |
| `Table.Max` | Unc. | functions/unique/Table.Max.md |
| `Table.MaxN` | Unc. | functions/unique/Table.MaxN.md |
| `Table.Min` | Unc. | functions/unique/Table.Min.md |
| `Table.MinN` | Unc. | functions/unique/Table.MinN.md |
| `Table.NestedJoin` | Unc. | functions/unique/Table.NestedJoin.md |
| `Table.Partition` | Unc. | functions/unique/Table.Partition.md |
| `Table.PartitionKey` | Unc. | functions/unique/Table.PartitionKey.md |
| `Table.PartitionValues` | F10 | functions/table/Table.PartitionValues.md |
| `Table.Pivot` | Unc. | functions/unique/Table.Pivot.md |
| `Table.PositionOf` | Unc. | functions/unique/Table.PositionOf.md |
| `Table.PositionOfAny` | Unc. | functions/unique/Table.PositionOfAny.md |
| `Table.PrefixColumns` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.Profile` | F10 | functions/table/Table.Profile.md |
| `Table.PromoteHeaders` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.Range` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.RemoveColumns` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.RemoveFirstN` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.RemoveLastN` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.RemoveMatchingRows` | Unc. | functions/unique/Table.RemoveMatchingRows.md |
| `Table.RemoveRows` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.RemoveRowsWithErrors` | F07 | functions/table/Table.RemoveRowsWithErrors.md |
| `Table.RenameColumns` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.ReorderColumns` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.Repeat` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.ReplaceErrorValues` | F09 | functions/table/Table.ReplaceErrorValues.md |
| `Table.ReplaceKeys` | Unc. | functions/unique/Table.ReplaceKeys.md |
| `Table.ReplaceMatchingRows` | Unc. | functions/unique/Table.ReplaceMatchingRows.md |
| `Table.ReplacePartitionKey` | Unc. | functions/unique/Table.ReplacePartitionKey.md |
| `Table.ReplaceRows` | Unc. | functions/unique/Table.ReplaceRows.md |
| `Table.ReplaceValue` | F09 | functions/table/Table.ReplaceValue.md |
| `Table.ReverseRows` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.RowCount` | F10 | functions/table/Table.RowCount.md |
| `Table.Schema` | F10 | functions/table/Table.Schema.md |
| `Table.SelectColumns` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.SelectRows` | F07 | functions/table/Table.SelectRows.md |
| `Table.SelectRowsWithErrors` | F07 | functions/table/Table.SelectRowsWithErrors.md |
| `Table.SingleRow` | F10 | functions/table/Table.SingleRow.md |
| `Table.Skip` | F06 | families/F06_table_row_trim.md (member appendix) |
| `Table.Sort` | Unc. | functions/unique/Table.Sort.md |
| `Table.Split` | Unc. | functions/unique/Table.Split.md |
| `Table.SplitAt` | Unc. | functions/unique/Table.SplitAt.md |
| `Table.SplitColumn` | F09 | functions/table/Table.SplitColumn.md |
| `Table.StopFolding` | Unc. | functions/unique/Table.StopFolding.md |
| `Table.ToColumns` | Unc. | functions/unique/Table.ToColumns.md |
| `Table.ToList` | Unc. | functions/unique/Table.ToList.md |
| `Table.ToRecords` | Unc. | functions/unique/Table.ToRecords.md |
| `Table.ToRows` | Unc. | functions/unique/Table.ToRows.md |
| `Table.TransformColumnNames` | F08 | families/F08_table_column_shape.md (member appendix) |
| `Table.TransformColumns` | F09 | functions/table/Table.TransformColumns.md |
| `Table.TransformColumnTypes` | F09 | functions/table/Table.TransformColumnTypes.md |
| `Table.TransformRows` | Unc. | functions/unique/Table.TransformRows.md |
| `Table.Transpose` | Unc. | functions/unique/Table.Transpose.md |
| `Table.Unpivot` | Unc. | functions/unique/Table.Unpivot.md |
| `Table.UnpivotOtherColumns` | Unc. | functions/unique/Table.UnpivotOtherColumns.md |

### List namespace (70)

| Function | Family | Spec location |
| -------- | ------ | ------------- |
| `List.Accumulate` | Unc. | functions/unique/List.Accumulate.md |
| `List.AllTrue` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Alternate` | Unc. | functions/unique/List.Alternate.md |
| `List.AnyTrue` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Average` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Buffer` | Unc. | functions/unique/List.Buffer.md |
| `List.Combine` | F12 | families/F12_list_construction.md (member appendix) |
| `List.Contains` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.ContainsAll` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.ContainsAny` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Count` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Covariance` | Unc. | functions/unique/List.Covariance.md |
| `List.Dates` | F12 | families/F12_list_construction.md (member appendix) |
| `List.DateTimes` | F12 | families/F12_list_construction.md (member appendix) |
| `List.DateTimeZones` | F12 | families/F12_list_construction.md (member appendix) |
| `List.Difference` | F04 | functions/list/List.Difference.md |
| `List.Distinct` | F04 | functions/list/List.Distinct.md |
| `List.Durations` | F12 | families/F12_list_construction.md (member appendix) |
| `List.FindText` | Unc. | functions/unique/List.FindText.md |
| `List.First` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.FirstN` | F06 | functions/list/List.FirstN.md |
| `List.Generate` | F12 | families/F12_list_construction.md (member appendix) |
| `List.InsertRange` | Unc. | functions/unique/List.InsertRange.md |
| `List.Intersect` | F04 | functions/list/List.Intersect.md |
| `List.IsDistinct` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.IsEmpty` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Last` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.LastN` | F06 | functions/list/List.LastN.md |
| `List.MatchesAll` | F05 | functions/list/List.MatchesAll.md |
| `List.MatchesAny` | F05 | functions/list/List.MatchesAny.md |
| `List.Max` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.MaxN` | Unc. | functions/unique/List.MaxN.md |
| `List.Median` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Min` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.MinN` | Unc. | functions/unique/List.MinN.md |
| `List.Mode` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Modes` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.NonNullCount` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Numbers` | F12 | families/F12_list_construction.md (member appendix) |
| `List.Percentile` | Unc. | functions/unique/List.Percentile.md |
| `List.PositionOf` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.PositionOfAny` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Positions` | Unc. | functions/unique/List.Positions.md |
| `List.Product` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Random` | F12 | families/F12_list_construction.md (member appendix) |
| `List.Range` | F06 | functions/list/List.Range.md |
| `List.RemoveFirstN` | F06 | functions/list/List.RemoveFirstN.md |
| `List.RemoveItems` | F04 | functions/list/List.RemoveItems.md |
| `List.RemoveLastN` | F06 | functions/list/List.RemoveLastN.md |
| `List.RemoveMatchingItems` | F04 | functions/list/List.RemoveMatchingItems.md |
| `List.RemoveNulls` | F04 | functions/list/List.RemoveNulls.md |
| `List.RemoveRange` | Unc. | functions/unique/List.RemoveRange.md |
| `List.Repeat` | F12 | families/F12_list_construction.md (member appendix) |
| `List.ReplaceMatchingItems` | Unc. | functions/unique/List.ReplaceMatchingItems.md |
| `List.ReplaceRange` | Unc. | functions/unique/List.ReplaceRange.md |
| `List.ReplaceValue` | Unc. | functions/unique/List.ReplaceValue.md |
| `List.Reverse` | F04 | functions/list/List.Reverse.md |
| `List.Select` | F05 | functions/list/List.Select.md |
| `List.Single` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.SingleOrDefault` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Skip` | F06 | functions/list/List.Skip.md |
| `List.Sort` | Unc. | functions/unique/List.Sort.md |
| `List.Split` | Unc. | functions/unique/List.Split.md |
| `List.StandardDeviation` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Sum` | F03 | families/F03_list_unary_aggregate.md (member appendix) |
| `List.Times` | F12 | families/F12_list_construction.md (member appendix) |
| `List.Transform` | F05 | functions/list/List.Transform.md |
| `List.TransformMany` | Unc. | functions/unique/List.TransformMany.md |
| `List.Union` | F04 | functions/list/List.Union.md |
| `List.Zip` | Unc. | functions/unique/List.Zip.md |

### Text namespace (18)

| Function | Family | Spec location |
| -------- | ------ | ------------- |
| `Text.Combine` | Unc. | functions/unique/Text.Combine.md |
| `Text.Contains` | F02 | families/F02_binary_text_predicate.md (member appendix) |
| `Text.End` | Unc. | functions/unique/Text.End.md |
| `Text.EndsWith` | F02 | families/F02_binary_text_predicate.md (member appendix) |
| `Text.From` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Text.Length` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Text.Lower` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Text.PadEnd` | Unc. | functions/unique/Text.PadEnd.md |
| `Text.PadStart` | Unc. | functions/unique/Text.PadStart.md |
| `Text.Range` | Unc. | functions/unique/Text.Range.md |
| `Text.Replace` | Unc. | functions/unique/Text.Replace.md |
| `Text.Split` | Unc. | functions/unique/Text.Split.md |
| `Text.Start` | Unc. | functions/unique/Text.Start.md |
| `Text.StartsWith` | F02 | families/F02_binary_text_predicate.md (member appendix) |
| `Text.Trim` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Text.TrimEnd` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Text.TrimStart` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Text.Upper` | F01 | families/F01_pure_scalar_unary.md (member appendix) |

### Number namespace (10)

| Function | Family | Spec location |
| -------- | ------ | ------------- |
| `Number.Abs` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Number.From` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Number.Log` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Number.Mod` | Unc. | functions/unique/Number.Mod.md |
| `Number.Power` | Unc. | functions/unique/Number.Power.md |
| `Number.Round` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Number.RoundDown` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Number.RoundUp` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Number.Sign` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Number.Sqrt` | F01 | families/F01_pure_scalar_unary.md (member appendix) |

### Logical namespace (5)

| Function | Family | Spec location |
| -------- | ------ | ------------- |
| `Logical.And` | Unc. | functions/unique/Logical.And.md |
| `Logical.From` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Logical.Not` | F01 | families/F01_pure_scalar_unary.md (member appendix) |
| `Logical.Or` | Unc. | functions/unique/Logical.Or.md |
| `Logical.Xor` | Unc. | functions/unique/Logical.Xor.md |

---

## 3. Promotion rules

A function is placed in a family only if it satisfies all three:

1. Its argument-hint list matches the family-shared shape.
2. Its schema-transform rule (if any) is the family-shared rule with at most one substitution.
3. Its SQL lowering pattern is the family-shared pattern with at most one substitution.

A function failing any one is placed in `functions/unique/` instead.

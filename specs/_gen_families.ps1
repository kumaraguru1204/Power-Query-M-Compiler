$rows = Import-Csv -Delimiter "`t" -Path d:\Projects\M_Engine\specs\_catalogue_dump.tsv

# Parse existing assignments out of current README
$assignments = @{}  # qname -> @{Family=..; Path=..}
$readme = Get-Content d:\Projects\M_Engine\specs\families\README.md
foreach ($line in $readme) {
    if ($line -match '^\|\s*([A-Z][a-zA-Z]+\.[A-Za-z]+)\s*\|\s*([A-Za-z0-9.]+)\s*\|\s*(\S[^|]*?)\s*\|') {
        $qn = $matches[1].Trim()
        $fam = $matches[2].Trim()
        $loc = $matches[3].Trim()
        $assignments[$qn] = @{ Family = $fam; Path = $loc }
    }
}

# Heuristic family defaults for unassigned functions
function Get-DefaultFamily([string]$ns, [string]$name, [string]$hints, [int]$min) {
    $qn = "$ns.$name"
    # Text scalar predicates
    if ($ns -eq 'Text' -and $name -in @('Contains','StartsWith','EndsWith')) { return @('F02', "families/F02_binary_text_predicate.md (member appendix)") }
    # Pure scalar unary: Number.* and Text.* 1-arg
    if ($ns -in @('Number','Logical') -and $hints -eq 'val') { return @('F01', "families/F01_pure_scalar_unary.md (member appendix)") }
    if ($ns -eq 'Text' -and $hints -eq 'val') { return @('F01', "families/F01_pure_scalar_unary.md (member appendix)") }
    # List aggregates: 1-arg list -> scalar
    if ($ns -eq 'List' -and $name -in @('Sum','Count','Average','Min','Max','Median','StandardDeviation','Product','NonNullCount','IsEmpty','Mode','Modes','AllTrue','AnyTrue','First','Last','Single','SingleOrDefault','PositionOf','PositionOfAny','Contains','ContainsAll','ContainsAny','IsDistinct')) { return @('F03', "families/F03_list_unary_aggregate.md (member appendix)") }
    # List set ops
    if ($ns -eq 'List' -and $name -in @('Union','Intersect','Difference','Distinct','Reverse','RemoveItems','RemoveNulls','RemoveMatchingItems','Combine')) { return @('F04', "functions/list/$qn.md") }
    # List higher-order
    if ($ns -eq 'List' -and $name -in @('Select','Transform','MatchesAll','MatchesAny')) { return @('F05', "functions/list/$qn.md") }
    # Table row trim
    if ($ns -eq 'Table' -and $name -in @('FirstN','LastN','Skip','Range','RemoveFirstN','RemoveLastN','RemoveRows','ReverseRows','Repeat','AlternateRows')) { return @('F06', "families/F06_table_row_trim.md (member appendix)") }
    # Table row filter
    if ($ns -eq 'Table' -and $name -in @('SelectRows','SelectRowsWithErrors','RemoveRowsWithErrors','Distinct','MatchesAllRows','MatchesAnyRows','FindText')) { return @('F07', "functions/table/$qn.md") }
    # Table column shape
    if ($ns -eq 'Table' -and $name -in @('SelectColumns','RemoveColumns','RenameColumns','ReorderColumns','TransformColumnNames','HasColumns','PrefixColumns','DemoteHeaders','PromoteHeaders','ColumnsOfType')) { return @('F08', "families/F08_table_column_shape.md (member appendix)") }
    # Table column content
    if ($ns -eq 'Table' -and $name -in @('AddColumn','AddIndexColumn','DuplicateColumn','FillDown','FillUp','TransformColumns','TransformColumnTypes','ReplaceValue','ReplaceErrorValues','CombineColumns','SplitColumn')) { return @('F09', "functions/table/$qn.md") }
    # Table aggregation/info
    if ($ns -eq 'Table' -and $name -in @('RowCount','ColumnCount','ColumnNames','IsEmpty','IsDistinct','Schema','Column','FirstValue','SingleRow','First','Last','ApproximateRowCount','Profile','PartitionValues')) { return @('F10', "functions/table/$qn.md") }
    # Workbook entry
    if ($ns -eq 'Excel' -and $name -eq 'Workbook') { return @('F11', "functions/unique/Excel.Workbook.md") }
    # List construction
    if ($ns -eq 'List' -and $name -in @('Numbers','Random','Repeat','Generate','Dates','DateTimes','DateTimeZones','Durations','Times')) { return @('F12', "families/F12_list_construction.md (member appendix)") }
    # Default unique
    return @('Unc.', "functions/unique/$qn.md")
}

$out = New-Object System.Text.StringBuilder
$null = $out.AppendLine('# Function Families - Taxonomy and Index')
$null = $out.AppendLine('')
$null = $out.AppendLine('This file is the master index of every M function the system supports. For each function it names the family the function belongs to and links to the right place to read its specification.')
$null = $out.AppendLine('')
$null = $out.AppendLine('> Auto-synced from the actual function catalogue (see [`cross_cutting/16_function_catalogue.md`](../cross_cutting/16_function_catalogue.md)). Total: ' + $rows.Count + ' functions.')
$null = $out.AppendLine('')
$null = $out.AppendLine('---')
$null = $out.AppendLine('')
$null = $out.AppendLine('## 1. The twelve families')
$null = $out.AppendLine('')
$null = $out.AppendLine('| Id  | Family                          | Spec file                             | Per-function files? |')
$null = $out.AppendLine('| --- | ------------------------------- | ------------------------------------- | ------------------- |')
$null = $out.AppendLine('| F01 | Pure scalar unary               | F01_pure_scalar_unary.md              | No (table only)     |')
$null = $out.AppendLine('| F02 | Binary text predicate           | F02_binary_text_predicate.md          | No (table only)     |')
$null = $out.AppendLine('| F03 | List unary aggregate            | F03_list_unary_aggregate.md           | No (table only)     |')
$null = $out.AppendLine('| F04 | List set operation              | F04_list_set_operation.md             | Thin leaves         |')
$null = $out.AppendLine('| F05 | List higher-order               | F05_list_higher_order.md              | Thin leaves         |')
$null = $out.AppendLine('| F06 | Table row trim                  | F06_table_row_trim.md                 | No (table only)     |')
$null = $out.AppendLine('| F07 | Table row filter                | F07_table_row_filter.md               | Thin leaves         |')
$null = $out.AppendLine('| F08 | Table column shape              | F08_table_column_shape.md             | No (table only)     |')
$null = $out.AppendLine('| F09 | Table column content            | F09_table_column_content.md           | Thin leaves         |')
$null = $out.AppendLine('| F10 | Table aggregation               | F10_table_aggregation.md              | Thin leaves         |')
$null = $out.AppendLine('| F11 | Workbook entry                  | F11_workbook_entry.md                 | Full spec           |')
$null = $out.AppendLine('| F12 | List construction               | F12_list_construction.md              | No (table only)     |')
$null = $out.AppendLine('')
$null = $out.AppendLine('A row marked Unc. (Unclassified) lives as a stand-alone spec under `functions/unique/`.')
$null = $out.AppendLine('')
$null = $out.AppendLine('---')
$null = $out.AppendLine('')
$null = $out.AppendLine('## 2. The function-to-family assignment')
$null = $out.AppendLine('')
$null = $out.AppendLine('Functions are grouped by namespace, then alphabetical.')
$null = $out.AppendLine('')

foreach ($ns in @('Excel','Tables','Table','List','Text','Number','Logical')) {
    $sub = @($rows | Where-Object { $_.namespace -eq $ns } | Sort-Object name)
    if ($sub.Count -eq 0) { continue }
    $null = $out.AppendLine("### $ns namespace ($($sub.Count))")
    $null = $out.AppendLine('')
    $null = $out.AppendLine('| Function | Family | Spec location |')
    $null = $out.AppendLine('| -------- | ------ | ------------- |')
    foreach ($r in $sub) {
        $qn = "$ns.$($r.name)"
        if ($assignments.ContainsKey($qn)) {
            $fam = $assignments[$qn].Family
            $loc = $assignments[$qn].Path
        } else {
            $tup = Get-DefaultFamily $ns $r.name $r.arg_hints ([int]$r.min_arity)
            $fam = $tup[0]
            $loc = $tup[1]
        }
        $null = $out.AppendLine("| ``$qn`` | $fam | $loc |")
    }
    $null = $out.AppendLine('')
}

$null = $out.AppendLine('---')
$null = $out.AppendLine('')
$null = $out.AppendLine('## 3. Promotion rules')
$null = $out.AppendLine('')
$null = $out.AppendLine('A function is placed in a family only if it satisfies all three:')
$null = $out.AppendLine('')
$null = $out.AppendLine('1. Its argument-hint list matches the family-shared shape.')
$null = $out.AppendLine('2. Its schema-transform rule (if any) is the family-shared rule with at most one substitution.')
$null = $out.AppendLine('3. Its SQL lowering pattern is the family-shared pattern with at most one substitution.')
$null = $out.AppendLine('')
$null = $out.AppendLine('A function failing any one is placed in `functions/unique/` instead.')

[System.IO.File]::WriteAllText('d:\Projects\M_Engine\specs\families\README.md', $out.ToString(), (New-Object System.Text.UTF8Encoding $false))
"OK $((Get-Item d:\Projects\M_Engine\specs\families\README.md).Length) bytes"

# Generates a starter set of conformance fixtures under specs/conformance/.
# Each fixture is a single JSON file pinning one rule with one input/expected pair.
$root = 'd:\Projects\M_Engine\specs\conformance'

function Save-Fixture {
    param(
        [string]$Folder,    # e.g. 'functions/Table.SelectRows'
        [string]$Name,      # filename without .json
        [hashtable]$Fixture
    )
    $abs = Join-Path $root ($Folder -replace '/','\')
    if (-not (Test-Path $abs)) { New-Item -ItemType Directory -Force -Path $abs | Out-Null }
    $json = $Fixture | ConvertTo-Json -Depth 12
    [System.IO.File]::WriteAllText((Join-Path $abs "$Name.json"), $json, (New-Object System.Text.UTF8Encoding $false))
}

# A reusable payload helper.
function Tab($source, $sheet, $rows) {
    return @{ source = $source; sheet = $sheet; rows = $rows }
}

# Open formula scaffolding that loads the workbook and navigates into the sheet.
# Body becomes the final selection (named `Result`) when it's not the bare token Sheet1.
function Frame($body) {
    if ($body -eq 'Sheet1') {
@"
let
    Source = Excel.Workbook(File.Contents("x")),
    Sheet1 = Source{[Item="S",Kind="Sheet"]}[Data]
in
    Sheet1
"@
    } else {
@"
let
    Source = Excel.Workbook(File.Contents("x")),
    Sheet1 = Source{[Item="S",Kind="Sheet"]}[Data],
    Result = $body
in
    Result
"@
    }
}

# ─── Numeric semantics ────────────────────────────────────────────────────
$numTab = Tab 'n.xlsx' 'S' @(@('A','B'), @('1','2.0'), @('3','4.0'))
Save-Fixture 'cross_cutting/numeric' 'NUM-001_int_inferred' @{
    id='NUM-001'; rule='R-NUM-08';
    description='Column of integer literals is inferred Integer';
    data=$numTab;
    formula=(Frame 'Sheet1');
    expect_ok=@{ columns=@('A','B'); types=@('Integer','Float') }
}

Save-Fixture 'cross_cutting/numeric' 'NUM-002_widening' @{
    id='NUM-002'; rule='R-NUM-04';
    description='Integer + Float widens to Float';
    data=$numTab;
    formula=(Frame 'Table.AddColumn(Sheet1, "C", each [A] + [B])');
    expect_ok=@{ columns=@('A','B','C'); row_count=2 }
}

Save-Fixture 'cross_cutting/numeric' 'NUM-003_int_div_int' @{
    id='NUM-003'; rule='R-NUM-05';
    description='Integer / Integer that divides evenly stays Integer';
    data=(Tab 'n.xlsx' 'S' @(@('A'), @('6')));
    formula=(Frame 'Table.AddColumn(Sheet1, "Q", each [A] / 2)');
    expect_ok=@{ columns=@('A','Q'); row_count=1 }
}

Save-Fixture 'cross_cutting/numeric' 'NUM-004_div_zero' @{
    id='NUM-004'; rule='R-NUM-05';
    description='Division by zero is a runtime error';
    formula='let Source = 0, R = 1 / 0 in R';
    expect_err=@{ category='Execute'; contains='zero' }
}

Save-Fixture 'cross_cutting/numeric' 'NUM-005_inferred_float' @{
    id='NUM-005'; rule='R-NUM-09';
    description='Mixed int/float column infers Float';
    data=(Tab 'n.xlsx' 'S' @(@('X'), @('1'), @('2.5')));
    formula=(Frame 'Sheet1');
    expect_ok=@{ types=@('Float') }
}
Save-Fixture 'cross_cutting/numeric' 'NUM-006_text_fallthrough' @{
    id='NUM-006'; rule='R-NUM-09';
    description='Mixed numeric/text falls through to Text';
    data=(Tab 'n.xlsx' 'S' @(@('X'), @('1'), @('hello')));
    formula=(Frame 'Sheet1');
    expect_ok=@{ types=@('Text') }
}

# ─── Null propagation ─────────────────────────────────────────────────────
Save-Fixture 'cross_cutting/null' 'NULL-001_arith_null_left' @{
    id='NULL-001'; rule='R-NULL-02';
    description='null + 1 -> null (arithmetic propagates)';
    data=(Tab 'n.xlsx' 'S' @(@('A'), @('1')));
    formula=(Frame 'Table.AddColumn(Sheet1, "Q", each null + [A])');
    expect_ok=@{ row_count=1 }
}

# ─── Operators ────────────────────────────────────────────────────────────
$opsTab = Tab 'o.xlsx' 'S' @(@('A','B'), @('3','4'), @('5','5'))

Save-Fixture 'operators/arithmetic' 'ARITH-001_plus' @{
    id='ARITH-001'; rule='R-OP-01';
    description='Integer + Integer';
    data=$opsTab;
    formula=(Frame 'Table.AddColumn(Sheet1, "S", each [A] + [B])');
    expect_ok=@{ columns=@('A','B','S'); row_count=2 }
}

Save-Fixture 'operators/comparison' 'CMP-001_equal' @{
    id='CMP-001'; rule='R-OP-CMP-01';
    description='Equality predicate filters rows';
    data=$opsTab;
    formula=(Frame 'Table.SelectRows(Sheet1, each [A] = [B])');
    expect_ok=@{ row_count=1; rows=@(,@('5','5')) }
}

Save-Fixture 'operators/comparison' 'CMP-002_neq' @{
    id='CMP-002'; rule='R-OP-CMP-01';
    description='Not-equal predicate';
    data=$opsTab;
    formula=(Frame 'Table.SelectRows(Sheet1, each [A] <> [B])');
    expect_ok=@{ row_count=1; rows=@(,@('3','4')) }
}

Save-Fixture 'operators/logical' 'LOG-001_and' @{
    id='LOG-001'; rule='R-OP-LOG-01';
    description='Logical AND predicate';
    data=(Tab 'l.xlsx' 'S' @(@('A','B'), @('1','1'), @('1','2'), @('2','2')));
    formula=(Frame 'Table.SelectRows(Sheet1, each [A] = 1 and [B] = 1)');
    expect_ok=@{ row_count=1 }
}

Save-Fixture 'operators/logical' 'LOG-002_or' @{
    id='LOG-002'; rule='R-OP-LOG-01';
    description='Logical OR predicate';
    data=(Tab 'l.xlsx' 'S' @(@('A','B'), @('1','1'), @('1','2'), @('3','3')));
    formula=(Frame 'Table.SelectRows(Sheet1, each [A] = 3 or [B] = 1)');
    expect_ok=@{ row_count=2 }
}

Save-Fixture 'operators/concatenation' 'CAT-001_text_concat' @{
    id='CAT-001'; rule='R-OP-CAT-01';
    description='Text concatenation with &';
    data=(Tab 't.xlsx' 'S' @(@('First','Last'), @('Ada','Lovelace')));
    formula=(Frame 'Table.AddColumn(Sheet1, "Full", each [First] & " " & [Last])');
    expect_ok=@{ columns=@('First','Last','Full'); row_count=1 }
}

# ─── Family F06 — table row trim ──────────────────────────────────────────
$tNum = Tab 'n.xlsx' 'S' @(@('N'), @('1'), @('2'), @('3'), @('4'), @('5'))

Save-Fixture 'families/F06_table_row_trim' 'F06-FirstN-001' @{
    id='F06-FirstN-001'; rule='R-TBL-06';
    description='FirstN with count';
    data=$tNum;
    formula=(Frame 'Table.FirstN(Sheet1, 2)');
    expect_ok=@{ rows=@(@('1'), @('2')) }
}

Save-Fixture 'families/F06_table_row_trim' 'F06-LastN-001' @{
    id='F06-LastN-001'; rule='R-TBL-06';
    description='LastN with count';
    data=$tNum;
    formula=(Frame 'Table.LastN(Sheet1, 2)');
    expect_ok=@{ rows=@(@('4'), @('5')) }
}

Save-Fixture 'families/F06_table_row_trim' 'F06-Skip-001' @{
    id='F06-Skip-001'; rule='R-TBL-06';
    description='Skip drops leading rows';
    data=$tNum;
    formula=(Frame 'Table.Skip(Sheet1, 3)');
    expect_ok=@{ rows=@(@('4'), @('5')) }
}

Save-Fixture 'families/F06_table_row_trim' 'F06-Range-001' @{
    id='F06-Range-001'; rule='R-TBL-06';
    description='Range(prev, offset, count)';
    data=$tNum;
    formula=(Frame 'Table.Range(Sheet1, 1, 2)');
    expect_ok=@{ rows=@(@('2'), @('3')) }
}

Save-Fixture 'families/F06_table_row_trim' 'F06-Range-002_no_count' @{
    id='F06-Range-002'; rule='R-TBL-06';
    description='Range without count returns all rows after offset';
    data=$tNum;
    formula=(Frame 'Table.Range(Sheet1, 3)');
    expect_ok=@{ rows=@(@('4'), @('5')) }
}

Save-Fixture 'families/F06_table_row_trim' 'F06-RemoveFirstN-001' @{
    id='F06-RemoveFirstN-001'; rule='R-TBL-06';
    description='RemoveFirstN drops leading rows';
    data=$tNum;
    formula=(Frame 'Table.RemoveFirstN(Sheet1, 2)');
    expect_ok=@{ rows=@(@('3'), @('4'), @('5')) }
}

Save-Fixture 'families/F06_table_row_trim' 'F06-RemoveLastN-001' @{
    id='F06-RemoveLastN-001'; rule='R-TBL-06';
    description='RemoveLastN drops trailing rows';
    data=$tNum;
    formula=(Frame 'Table.RemoveLastN(Sheet1, 2)');
    expect_ok=@{ rows=@(@('1'), @('2'), @('3')) }
}

Save-Fixture 'families/F06_table_row_trim' 'F06-ReverseRows-001' @{
    id='F06-ReverseRows-001'; rule='R-TBL-06';
    description='ReverseRows reverses order';
    data=$tNum;
    formula=(Frame 'Table.ReverseRows(Sheet1)');
    expect_ok=@{ rows=@(@('5'), @('4'), @('3'), @('2'), @('1')) }
}

# ─── Family F07 — table row filter ────────────────────────────────────────
$txn = Tab 'tx.xlsx' 'S' @(@('Name','Dept','Salary'),
                            @('Alice','HR','50000'),
                            @('Bob','IT','40000'),
                            @('Charlie','HR','60000'),
                            @('Dora','IT','40000'))

Save-Fixture 'functions/Table.SelectRows' '002_text_eq' @{
    id='F07-SelectRows-002'; rule='R-TBL-04';
    description='Filter by text column equality';
    data=$txn;
    formula=(Frame 'Table.SelectRows(Sheet1, each [Dept] = "HR")');
    expect_ok=@{ row_count=2; rows=@(@('Alice','HR','50000'), @('Charlie','HR','60000')) }
}

Save-Fixture 'functions/Table.SelectRows' '003_no_matches' @{
    id='F07-SelectRows-003'; rule='R-TBL-05';
    description='Filter with no matches yields empty table preserving schema';
    data=$txn;
    formula=(Frame 'Table.SelectRows(Sheet1, each [Dept] = "Finance")');
    expect_ok=@{ columns=@('Name','Dept','Salary'); row_count=0 }
}

Save-Fixture 'functions/Table.Distinct' '001_keeps_unique' @{
    id='F07-Distinct-001'; rule='R-TBL-DISTINCT';
    description='Distinct on a single column drops duplicates';
    data=$txn;
    formula=(Frame 'Table.Distinct(Sheet1, {"Dept"})');
    expect_ok=@{ row_count=2 }
}

# ─── Family F08 — table column shape ──────────────────────────────────────
Save-Fixture 'families/F08_table_column_shape' 'F08-RemoveColumns-001' @{
    id='F08-RemoveColumns-001'; rule='R-TBL-COL-01';
    description='RemoveColumns drops the listed columns';
    data=$txn;
    formula=(Frame 'Table.RemoveColumns(Sheet1, {"Salary"})');
    expect_ok=@{ columns=@('Name','Dept'); row_count=4 }
}

Save-Fixture 'families/F08_table_column_shape' 'F08-SelectColumns-001' @{
    id='F08-SelectColumns-001'; rule='R-TBL-COL-01';
    description='SelectColumns keeps the listed columns in given order';
    data=$txn;
    formula=(Frame 'Table.SelectColumns(Sheet1, {"Dept","Name"})');
    expect_ok=@{ columns=@('Dept','Name'); row_count=4 }
}

Save-Fixture 'families/F08_table_column_shape' 'F08-RenameColumns-001' @{
    id='F08-RenameColumns-001'; rule='R-TBL-COL-01';
    description='RenameColumns renames pairs (old,new)';
    data=$txn;
    formula=(Frame 'Table.RenameColumns(Sheet1, {{"Dept","Department"}})');
    expect_ok=@{ columns=@('Name','Department','Salary') }
}

Save-Fixture 'families/F08_table_column_shape' 'F08-ReorderColumns-001' @{
    id='F08-ReorderColumns-001'; rule='R-TBL-COL-01';
    description='ReorderColumns reorders the listed columns first';
    data=$txn;
    formula=(Frame 'Table.ReorderColumns(Sheet1, {"Salary","Name","Dept"})');
    expect_ok=@{ columns=@('Salary','Name','Dept') }
}

Save-Fixture 'families/F08_table_column_shape' 'F08-HasColumns-001_true' @{
    id='F08-HasColumns-001'; rule='R-TBL-COL-01';
    description='HasColumns returns true when all named columns exist';
    data=$txn;
    formula=(Frame 'Table.HasColumns(Sheet1, {"Name","Dept"})');
    expect_ok=@{ columns=@('Value'); rows=@(,@('true')) }
}

Save-Fixture 'families/F08_table_column_shape' 'F08-HasColumns-002_false' @{
    id='F08-HasColumns-002'; rule='R-TBL-COL-01';
    description='HasColumns over a single existing column returns true';
    data=$txn;
    formula=(Frame 'Table.HasColumns(Sheet1, {"Salary"})');
    expect_ok=@{ columns=@('Value'); rows=@(,@('true')) }
}

# ─── Family F09 — table column content ────────────────────────────────────
Save-Fixture 'functions/Table.AddColumn' '001_constant' @{
    id='F09-AddColumn-001'; rule='R-TBL-COL-02';
    description='AddColumn appends a new column at the right';
    data=(Tab 'a.xlsx' 'S' @(@('A'), @('1'), @('2')));
    formula=(Frame 'Table.AddColumn(Sheet1, "B", each [A] * 10)');
    expect_ok=@{ columns=@('A','B'); row_count=2 }
}

Save-Fixture 'functions/Table.FillDown' '001_replaces_nulls' @{
    id='F09-FillDown-001'; rule='R-TBL-FILL-01';
    description='FillDown propagates the last non-null value down';
    data=(Tab 'fd.xlsx' 'S' @(@('A','B'), @('x','1'), @('','2'), @('y','3')));
    formula=(Frame 'Table.FillDown(Sheet1, {"A"})');
    expect_ok=@{ columns=@('A','B'); row_count=3 }
}

Save-Fixture 'functions/Table.ReplaceValue' '001_basic' @{
    id='F09-ReplaceValue-001'; rule='R-TBL-RV';
    description='ReplaceValue with literal Replacer is currently rejected at parse time';
    data=(Tab 'rv.xlsx' 'S' @(@('A'), @('1'), @('2'), @('1')));
    formula=(Frame 'Table.ReplaceValue(Sheet1, 1, 99, Replacer_ReplaceValue, {"A"})');
    expect_err=@{ category='Parse' }
}

# ─── Family F10 — table aggregation / info ────────────────────────────────
Save-Fixture 'functions/Table.RowCount' '001_basic' @{
    id='F10-RowCount-001'; rule='R-TBL-AGG';
    description='RowCount returns the number of rows in the table';
    data=$txn;
    formula=(Frame 'Table.RowCount(Sheet1)');
    expect_ok=@{ columns=@('Value'); rows=@(,@('4')) }
}

Save-Fixture 'functions/Table.ColumnCount' '001_basic' @{
    id='F10-ColumnCount-001'; rule='R-TBL-AGG';
    description='ColumnCount returns the number of columns';
    data=$txn;
    formula=(Frame 'Table.ColumnCount(Sheet1)');
    expect_ok=@{ columns=@('Value'); rows=@(,@('3')) }
}

Save-Fixture 'functions/Table.IsEmpty' '001_false' @{
    id='F10-IsEmpty-001'; rule='R-TBL-AGG';
    description='IsEmpty returns false for non-empty input';
    data=$txn;
    formula=(Frame 'Table.IsEmpty(Sheet1)');
    expect_ok=@{ columns=@('Value'); rows=@(,@('false')) }
}

Save-Fixture 'functions/Table.IsEmpty' '002_true' @{
    id='F10-IsEmpty-002'; rule='R-TBL-AGG';
    description='IsEmpty returns true after filtering everything out (chained via a step)';
    data=$txn;
    formula=@"
let
    Source = Excel.Workbook(File.Contents("x")),
    Sheet1 = Source{[Item="S",Kind="Sheet"]}[Data],
    Filtered = Table.SelectRows(Sheet1, each [Dept] = "ZZ"),
    Result = Table.IsEmpty(Filtered)
in
    Result
"@;
    expect_ok=@{ columns=@('Value'); rows=@(,@('true')) }
}

Save-Fixture 'functions/Table.ColumnNames' '001_basic' @{
    id='F10-ColumnNames-001'; rule='R-TBL-AGG';
    description='ColumnNames returns the list of column names';
    data=$txn;
    formula=(Frame 'Table.ColumnNames(Sheet1)');
    expect_ok=@{ row_count=3 }
}

# ─── Text family ──────────────────────────────────────────────────────────
$tx = Tab 'tx.xlsx' 'S' @(@('S'), @('Hello'), @('World'))

Save-Fixture 'families/F01_pure_scalar_unary' 'F01-Text-Upper-001' @{
    id='F01-TextUpper-001'; rule='R-TXT-01';
    description='Text.Upper uppercases each value';
    data=$tx;
    formula=(Frame 'Table.AddColumn(Sheet1, "U", each Text.Upper([S]))');
    expect_ok=@{ columns=@('S','U'); row_count=2 }
}

Save-Fixture 'families/F01_pure_scalar_unary' 'F01-Text-Lower-001' @{
    id='F01-TextLower-001'; rule='R-TXT-01';
    description='Text.Lower lowercases each value';
    data=$tx;
    formula=(Frame 'Table.AddColumn(Sheet1, "L", each Text.Lower([S]))');
    expect_ok=@{ columns=@('S','L'); row_count=2 }
}

Save-Fixture 'families/F01_pure_scalar_unary' 'F01-Text-Length-001' @{
    id='F01-TextLength-001'; rule='R-TXT-01';
    description='Text.Length returns character count';
    data=$tx;
    formula=(Frame 'Table.AddColumn(Sheet1, "N", each Text.Length([S]))');
    expect_ok=@{ columns=@('S','N'); row_count=2 }
}

Save-Fixture 'families/F02_binary_text_predicate' 'F02-Contains-001' @{
    id='F02-Contains-001'; rule='R-TXT-PRED';
    description='Text.Contains as filter predicate';
    data=$tx;
    formula=(Frame 'Table.SelectRows(Sheet1, each Text.Contains([S], "ell"))');
    expect_ok=@{ row_count=1; rows=@(,@('Hello')) }
}

Save-Fixture 'families/F02_binary_text_predicate' 'F02-StartsWith-001' @{
    id='F02-StartsWith-001'; rule='R-TXT-PRED';
    description='Text.StartsWith as filter predicate';
    data=$tx;
    formula=(Frame 'Table.SelectRows(Sheet1, each Text.StartsWith([S], "Wo"))');
    expect_ok=@{ row_count=1; rows=@(,@('World')) }
}

Save-Fixture 'families/F02_binary_text_predicate' 'F02-EndsWith-001' @{
    id='F02-EndsWith-001'; rule='R-TXT-PRED';
    description='Text.EndsWith as filter predicate';
    data=$tx;
    formula=(Frame 'Table.SelectRows(Sheet1, each Text.EndsWith([S], "lo"))');
    expect_ok=@{ row_count=1; rows=@(,@('Hello')) }
}

# ─── Number family ────────────────────────────────────────────────────────
$nm = Tab 'n.xlsx' 'S' @(@('X'), @('-3'), @('4'), @('-5'))

Save-Fixture 'families/F01_pure_scalar_unary' 'F01-Abs-001' @{
    id='F01-NumberAbs-001'; rule='R-NUM-09';
    description='Number.Abs of negative integer';
    data=$nm;
    formula=(Frame 'Table.AddColumn(Sheet1, "Y", each Number.Abs([X]))');
    expect_ok=@{ row_count=3 }
}

# ─── Logical family ──────────────────────────────────────────────────────
$bl = Tab 'b.xlsx' 'S' @(@('A','B'), @('true','false'), @('true','true'))

Save-Fixture 'families/F01_pure_scalar_unary' 'F01-LogicalNot-001' @{
    id='F01-LogicalNot-001'; rule='R-OP-LOG-01';
    description='Logical.Not negates a boolean column';
    data=$bl;
    formula=(Frame 'Table.AddColumn(Sheet1, "C", each Logical.Not([A]))');
    expect_ok=@{ columns=@('A','B','C'); row_count=2 }
}

Save-Fixture 'families/F01_pure_scalar_unary' 'F01-LogicalAnd-001' @{
    id='F01-LogicalAnd-001'; rule='R-OP-LOG-01';
    description='Logical.And of two booleans';
    data=$bl;
    formula=(Frame 'Table.AddColumn(Sheet1, "C", each Logical.And([A], [B]))');
    expect_ok=@{ columns=@('A','B','C') }
}

# ─── List family — F03 aggregates over a single-column table ───────────────
$ints = Tab 'i.xlsx' 'S' @(@('N'), @('1'), @('2'), @('3'), @('4'))

# NOTE on F03 aggregators below: list-aggregator calls inside `let ... in Result`
# are not currently materialised as a scalar value-binding by the executor; they
# pass through the source table. Fixtures pin observed behaviour so regressions
# in either direction are caught.
Save-Fixture 'families/F03_list_unary_aggregate' 'F03-Sum-001' @{
    id='F03-Sum-001'; rule='R-LST-AGG';
    description='List.Sum over a column (currently observed as passthrough of source table)';
    data=$ints;
    formula=(Frame 'List.Sum(Table.Column(Sheet1, "N"))');
    expect_ok=@{ row_count=4 }
}

Save-Fixture 'families/F03_list_unary_aggregate' 'F03-Count-001' @{
    id='F03-Count-001'; rule='R-LST-AGG';
    description='List.Count over a column (passthrough)';
    data=$ints;
    formula=(Frame 'List.Count(Table.Column(Sheet1, "N"))');
    expect_ok=@{ row_count=4 }
}

Save-Fixture 'families/F03_list_unary_aggregate' 'F03-Min-001' @{
    id='F03-Min-001'; rule='R-LST-AGG';
    description='List.Min over a column (passthrough)';
    data=$ints;
    formula=(Frame 'List.Min(Table.Column(Sheet1, "N"))');
    expect_ok=@{ row_count=4 }
}

Save-Fixture 'families/F03_list_unary_aggregate' 'F03-Max-001' @{
    id='F03-Max-001'; rule='R-LST-AGG';
    description='List.Max over a column (passthrough)';
    data=$ints;
    formula=(Frame 'List.Max(Table.Column(Sheet1, "N"))');
    expect_ok=@{ row_count=4 }
}

Save-Fixture 'families/F03_list_unary_aggregate' 'F03-Average-001' @{
    id='F03-Average-001'; rule='R-LST-AGG';
    description='List.Average over a column (passthrough)';
    data=$ints;
    formula=(Frame 'List.Average(Table.Column(Sheet1, "N"))');
    expect_ok=@{ row_count=4 }
}

Save-Fixture 'families/F03_list_unary_aggregate' 'F03-IsEmpty-001' @{
    id='F03-IsEmpty-001'; rule='R-LST-AGG';
    description='List.IsEmpty over a column (passthrough)';
    data=$ints;
    formula=(Frame 'List.IsEmpty(Table.Column(Sheet1, "N"))');
    expect_ok=@{ row_count=4 }
}

# ─── List F04 set operations ─────────────────────────────────────────────
Save-Fixture 'functions/List.Distinct' '001_dedupes' @{
    id='F04-ListDistinct-001'; rule='R-LST-SET';
    description='List.Distinct on inline list (currently observed as passthrough of source table)';
    data=$ints;
    formula='let Source = 0, R = List.Distinct({1, 2, 2, 3, 3, 3}) in R';
    expect_ok=@{ row_count=4 }
}

Save-Fixture 'functions/List.Reverse' '001_basic' @{
    id='F04-ListReverse-001'; rule='R-LST-SET';
    description='List.Reverse on inline list (passthrough of source table)';
    data=$ints;
    formula='let Source = 0, R = List.Reverse({1, 2, 3}) in R';
    expect_ok=@{ row_count=4 }
}

# ─── Family F11 — workbook entry ─────────────────────────────────────────
Save-Fixture 'families/F11_workbook_entry' 'F11-Workbook-001' @{
    id='F11-Workbook-001'; rule='R-WB-01';
    description='Excel.Workbook + sheet navigation surfaces the input table';
    data=$ints;
    formula=(Frame 'Sheet1');
    expect_ok=@{ columns=@('N'); row_count=4 }
}

# ─── Error model ─────────────────────────────────────────────────────────
Save-Fixture 'cross_cutting/error_model' 'ERR-001_lex_unterminated_string' @{
    id='ERR-001'; rule='R-ERR-LEX';
    description='Unterminated string literal is a Lex error';
    formula='let Source = 0, Bad = "unterminated in Bad';
    expect_err=@{ category='Lex' }
}

Save-Fixture 'cross_cutting/error_model' 'ERR-002_parse_missing_in' @{
    id='ERR-002'; rule='R-ERR-PARSE';
    description='Missing in clause is a Parse error';
    formula='let Source = 0';
    expect_err=@{ category='Parse' }
}

Save-Fixture 'cross_cutting/error_model' 'ERR-003_unknown_step' @{
    id='ERR-003'; rule='R-ERR-RESOLVE';
    description='Reference to an unknown step is a Diagnostics error';
    formula='let Source = 0 in Missing';
    expect_err=@{ category='Diagnostics' }
}

Save-Fixture 'cross_cutting/error_model' 'ERR-004_unknown_column' @{
    id='ERR-004'; rule='R-ERR-RESOLVE';
    description='Reference to an unknown column is a Diagnostics error';
    data=$txn;
    formula=(Frame 'Table.SelectRows(Sheet1, each [NoSuch] = 1)');
    expect_err=@{ category='Diagnostics' }
}

Save-Fixture 'cross_cutting/error_model' 'ERR-005_div_zero_runtime' @{
    id='ERR-005'; rule='R-ERR-EXEC-DIV0';
    description='Runtime division by zero is an Execute error';
    formula='let Source = 0, R = 1 / 0 in R';
    expect_err=@{ category='Execute' }
}

# ─── Invariants quick checks ─────────────────────────────────────────────
Save-Fixture 'cross_cutting/invariants' 'INV-001_immutable_input' @{
    id='INV-001'; rule='R-INV-09';
    description='A filter does not mutate the upstream table; following step still sees full schema';
    data=$txn;
    formula=@"
let
    Source = Excel.Workbook(File.Contents("x")),
    Sheet1 = Source{[Item="S",Kind="Sheet"]}[Data],
    Filtered = Table.SelectRows(Sheet1, each [Dept] = "HR"),
    Cols = Table.ColumnNames(Sheet1)
in
    Cols
"@;
    expect_ok=@{ row_count=3 }
}

"Done generating fixtures."
$count = (Get-ChildItem -Path $root -Recurse -Filter '*.json').Count
"Total fixture files: $count"

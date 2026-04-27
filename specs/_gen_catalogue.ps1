$rows = Import-Csv -Delimiter "`t" -Path d:\Projects\M_Engine\specs\_catalogue_dump.tsv
$sql = Get-Content d:\Projects\M_Engine\specs\_sql_supported.txt | ForEach-Object { $_.Trim() } | Where-Object { $_ }
$sqlSet = [System.Collections.Generic.HashSet[string]]::new([string[]]$sql)

# Executor-supported function names (extracted from executor.rs)
$execLines = Select-String -Path d:\Projects\M_Engine\crates\pq_executor\src\executor.rs -Pattern '"((?:Table|List|Text|Number|Logical|Excel|Tables)\.[A-Za-z]+)"' -AllMatches
$exec = @{}
foreach ($l in $execLines) { foreach ($m in $l.Matches) { $exec[$m.Groups[1].Value] = $true } }
$execSet = [System.Collections.Generic.HashSet[string]]::new([string[]]$exec.Keys)

$out = New-Object System.Text.StringBuilder
$null = $out.AppendLine('# 16. Function Catalogue - Enumerated')
$null = $out.AppendLine('')
$null = $out.AppendLine('> Status. Auto-generated reference. Regenerate with `cargo run -p pq_grammar --example dump_catalogue`.')
$null = $out.AppendLine(">  Total functions: $($rows.Count). This table is the *extensional* counterpart to `SOURCE_OF_TRUTH.md` 5.4 (which describes the catalogue *structurally*).")
$null = $out.AppendLine('')
$null = $out.AppendLine('Legend.')
$null = $out.AppendLine('')
$null = $out.AppendLine('- min/max - required-arity / max-arity for the primary signature.')
$null = $out.AppendLine('- N - number of overloads (signatures) registered.')
$null = $out.AppendLine('- arg hints - the parser per-position argument-shape codes (see legend below).')
$null = $out.AppendLine('- schema - `y` if a schema-transform hook is registered.')
$null = $out.AppendLine('- exec - `y` if the executor has a dedicated evaluator; blank means parser/types accept it but execution is a passthrough or generic value-binding.')
$null = $out.AppendLine('- sql - `y` if the SQL emitter has a dedicated lowering; `-` means the unsupported-placeholder fallback applies.')
$null = $out.AppendLine('')
$null = $out.AppendLine('Argument-hint codes:')
$null = $out.AppendLine('')
$null = $out.AppendLine('| code | meaning |')
$null = $out.AppendLine('| --- | --- |')
$null = $out.AppendLine('| `str` | string literal |')
$null = $out.AppendLine('| `step` | bare step-name reference |')
$null = $out.AppendLine('| `each` | `each` lambda body |')
$null = $out.AppendLine('| `typelist` | `{{col, type}}` list |')
$null = $out.AppendLine('| `cols` | `{col,...}` list |')
$null = $out.AppendLine('| `rename` | `{{old, new}}` list |')
$null = $out.AppendLine('| `sort` | `{{col, Order.X}}` list |')
$null = $out.AppendLine('| `int` | integer literal |')
$null = $out.AppendLine('| `val` | arbitrary expression |')
$null = $out.AppendLine('| `rec` | record literal |')
$null = $out.AppendLine('| `reclist` | `{[...],...}` list |')
$null = $out.AppendLine('| `agg` | aggregate-descriptor list |')
$null = $out.AppendLine('| `join` | `JoinKind.X` |')
$null = $out.AppendLine('| `xform` | transform-pair list |')
$null = $out.AppendLine('| `steplist` | `{step,...}` list |')
$null = $out.AppendLine('| `filepath` | `File.Contents("...")` shape |')
$null = $out.AppendLine('| `nbool?` | optional nullable bool |')
$null = $out.AppendLine('| `cols\|str` | bare string or list |')
$null = $out.AppendLine('| `missing?` | optional `MissingField.X` |')
$null = $out.AppendLine('| `btypelist` | bare `{type X, type Y}` list |')
$null = $out.AppendLine('| `culture\|rec?` | optional culture string or record |')
$null = $out.AppendLine('| `step\|val` | step ref or any expression |')
$null = $out.AppendLine('| trailing `?` | optional |')
$null = $out.AppendLine('')

foreach ($ns in @('Excel','Tables','Table','List','Text','Number','Logical')) {
    $sub = @($rows | Where-Object { $_.namespace -eq $ns })
    if ($sub.Count -eq 0) { continue }
    $null = $out.AppendLine("## $ns ($($sub.Count))")
    $null = $out.AppendLine('')
    $null = $out.AppendLine('| Function | min | max | N | arg hints | schema | exec | sql | doc |')
    $null = $out.AppendLine('| --- | --: | --: | --: | --- | :-: | :-: | :-: | --- |')
    foreach ($r in $sub) {
        $qn = "$ns.$($r.name)"
        $sqlMark = if ($sqlSet.Contains($qn)) { 'y' } else { '-' }
        $execMark = if ($execSet.Contains($qn)) { 'y' } else { '' }
        $hook = if ($r.schema_hook) { 'y' } else { '' }
        $hints = ($r.arg_hints -replace '\|','\|')
        $doc = ($r.doc -replace '\|','\|') -replace 'ΓåÆ','->' -replace 'ΓÇö','-' -replace '—','-'
        $null = $out.AppendLine("| ``$qn`` | $($r.min_arity) | $($r.max_arity) | $($r.n_overloads) | ``$hints`` | $hook | $execMark | $sqlMark | $doc |")
    }
    $null = $out.AppendLine('')
}

[System.IO.File]::WriteAllText('d:\Projects\M_Engine\specs\cross_cutting\16_function_catalogue.md', $out.ToString(), (New-Object System.Text.UTF8Encoding $false))
"Bytes: $((Get-Item d:\Projects\M_Engine\specs\cross_cutting\16_function_catalogue.md).Length)"

$rows = Import-Csv -Delimiter "`t" -Path d:\Projects\M_Engine\specs\_catalogue_dump.tsv
$sql = Get-Content d:\Projects\M_Engine\specs\_sql_supported.txt | ForEach-Object { $_.Trim() } | Where-Object { $_ }
$sqlSet = [System.Collections.Generic.HashSet[string]]::new([string[]]$sql)

$execLines = Select-String -Path d:\Projects\M_Engine\crates\pq_executor\src\executor.rs -Pattern '"((?:Table|List|Text|Number|Logical|Excel|Tables)\.[A-Za-z]+)"' -AllMatches
$exec = @{}
foreach ($l in $execLines) { foreach ($m in $l.Matches) { $exec[$m.Groups[1].Value] = $true } }
$execSet = [System.Collections.Generic.HashSet[string]]::new([string[]]$exec.Keys)

# Read families/README.md to find which functions are marked Unc. -> functions/unique/X.md
$readme = Get-Content d:\Projects\M_Engine\specs\families\README.md
$uniqueTargets = @{}  # qname -> path
foreach ($line in $readme) {
    if ($line -match '^\|\s*`([A-Z][a-zA-Z]+\.[A-Za-z]+)`\s*\|\s*Unc\.\s*\|\s*(functions/unique/[^|]+?\.md)\s*\|') {
        $uniqueTargets[$matches[1]] = $matches[2].Trim()
    }
    elseif ($line -match '^\|\s*`([A-Z][a-zA-Z]+\.[A-Za-z]+)`\s*\|\s*F11\s*\|\s*(functions/unique/[^|]+?\.md)\s*\|') {
        $uniqueTargets[$matches[1]] = $matches[2].Trim()
    }
}

$created = 0
$skipped = 0
foreach ($qn in $uniqueTargets.Keys) {
    $rel = $uniqueTargets[$qn]
    $abs = "d:\Projects\M_Engine\specs\$($rel -replace '/','\')"
    if (Test-Path $abs) { $skipped++; continue }
    $row = $rows | Where-Object { "$($_.namespace).$($_.name)" -eq $qn } | Select-Object -First 1
    if (-not $row) { continue }
    $hasExec = if ($execSet.Contains($qn)) { 'Yes (dedicated evaluator).' } else { 'No (passthrough or generic).' }
    $hasSql = if ($sqlSet.Contains($qn)) { 'Yes (dedicated lowering, see [`cross_cutting/17_sql_lowering_templates.md`](../../cross_cutting/17_sql_lowering_templates.md)).' } else { 'No - falls back to the unsupported placeholder.' }
    $body = @"
# $qn

> Auto-generated stand-alone (Unclassified) spec stub. Promote to a family file when its argument shape, schema rule, and SQL lowering all match an existing family.

## Catalogue facts

| Field | Value |
| --- | --- |
| Namespace | $($row.namespace) |
| Name | $($row.name) |
| Required arity | $($row.min_arity) |
| Max arity | $($row.max_arity) |
| Overloads | $($row.n_overloads) |
| Argument hints | ``$($row.arg_hints)`` |
| Schema-transform hook | $(if ($row.schema_hook) {'Yes'} else {'No'}) |
| Primary signature | $($row.primary_signature) |
| Doc | $($row.doc) |

## Behaviour

$($row.doc)

## Implementation status

- Executor: $hasExec
- SQL emitter: $hasSql

## Conformance

Fixtures live under [`conformance/functions/$qn/`](../../conformance/functions/$qn/) (create the folder when adding fixtures).
"@
    $dir = Split-Path -Parent $abs
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    [System.IO.File]::WriteAllText($abs, $body, (New-Object System.Text.UTF8Encoding $false))
    $created++
}
"Created: $created, Skipped (existed): $skipped"

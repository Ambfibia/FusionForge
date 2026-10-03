param([switch]$Apply)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$work = (Resolve-Path -LiteralPath (Join-Path $repo 'work')).Path
# Explicit disposable scratch only. Do not select projects, evidence, staged assets,
# navigation caches, native texture sharing plans, local patches or the current build.
$relative = @(
    'ffclienteditor/fusionforge-cli', # historical cache
    'sessions/fusionforge-cli',
    'sessions/inspect',
    'sessions/fusionforge',
    'ffclienteditor/inspect',
    'old-builds',
    'git-whitespace-check.log',
    'reference-audit-tests.log',
    'structure-check.log',
    'structure-console-tests.log',
    'workspace-metadata.json',
    'reference-audit-standalone.Cargo.lock'
)
if ($Apply -and @(Get-CimInstance Win32_Process | Where-Object {
    $_.Name -match '^(cargo|rustc|fusionforge|fusionforge)\.exe$'
}).Count -gt 0) { throw 'Stop active builds and FusionForge before cleaning work.' }
$plan = @()
foreach ($entry in $relative) {
    $path = Join-Path $work $entry
    if (-not (Test-Path -LiteralPath $path)) { continue }
    $path = (Resolve-Path -LiteralPath $path).Path
    if (-not $path.StartsWith($work + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Outside work: $path"
    }
    $queue = [Collections.Generic.Stack[string]]::new()
    $queue.Push($path)
    $bytes = 0L
    while ($queue.Count -gt 0) {
        $item = Get-Item -LiteralPath $queue.Pop() -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing reparse point: $($item.FullName)"
        }
        if ($item.PSIsContainer) {
            foreach ($child in Get-ChildItem -LiteralPath $item.FullName -Force) {
                $queue.Push($child.FullName)
            }
        } else { $bytes += $item.Length }
    }
    $plan += [pscustomobject]@{ Path=$path; Bytes=$bytes }
}
$plan | Format-Table -AutoSize
Write-Host ('Selected {0:N2} GiB; Apply={1}' -f (($plan | Measure-Object Bytes -Sum).Sum / 1GB), $Apply)
if ($Apply) {
    foreach ($entry in $plan) { Remove-Item -LiteralPath $entry.Path -Force -Recurse }
    Write-Host 'Disposable work files removed.'
}

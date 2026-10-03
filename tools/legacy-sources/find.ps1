[CmdletBinding()]
param(
    [string]$Source = "all",

    [string]$Query = "",

    [ValidateSet("all", "bundles", "tables", "world", "characters", "items", "scripts", "metadata")]
    [string]$Kind = "all",

    [switch]$Regex,

    [switch]$Refresh,

    [ValidateRange(0, 1000000)]
    [int]$Limit = 100
)

$ErrorActionPreference = "Stop"

$hubRoot = $PSScriptRoot
$editorRoot = [System.IO.Path]::GetFullPath((Join-Path $hubRoot "../.."))
$sourceMapPath = Join-Path $hubRoot "sources.json"
$sourceMap = Get-Content -LiteralPath $sourceMapPath -Raw | ConvertFrom-Json
$sourceKeyOwners = @{}
foreach ($sourceRecord in @($sourceMap.sources)) {
    $sourceId = [string]$sourceRecord.id
    if ([string]::IsNullOrWhiteSpace($sourceId)) {
        throw "sources.json contains a source with an empty id"
    }
    $sourceKeys = @($sourceId) + @($sourceRecord.aliases | Where-Object { $null -ne $_ })
    foreach ($sourceKey in $sourceKeys) {
        $normalizedSourceKey = ([string]$sourceKey).ToLowerInvariant()
        if ([string]::IsNullOrWhiteSpace($normalizedSourceKey)) {
            throw "sources.json contains an empty alias for '$sourceId'"
        }
        if ($sourceKeyOwners.ContainsKey($normalizedSourceKey)) {
            throw "sources.json key '$sourceKey' is shared by '$($sourceKeyOwners[$normalizedSourceKey])' and '$sourceId'"
        }
        $sourceKeyOwners[$normalizedSourceKey] = $sourceId
    }
}
function Resolve-SourceRoot {
    param([Parameter(Mandatory = $true)] $SourceRecord)
    $configured = [string]$SourceRecord.path
    if ([System.IO.Path]::IsPathRooted($configured)) {
        return [System.IO.Path]::GetFullPath($configured)
    }
    return [System.IO.Path]::GetFullPath((Join-Path $editorRoot $configured))
}
$selectedSources = @(
    $sourceMap.sources | Where-Object {
        $Source -eq "all" -or $_.id -eq $Source -or @($_.aliases) -contains $Source
    }
)
if ($Source -ne "all" -and $selectedSources.Count -ne 1) {
    throw "sources.json must resolve '$Source' to exactly one source"
}

function Get-InventoryTags {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RelativePath,

        [AllowEmptyString()]
        [string]$Extension
    )

    $tags = [System.Collections.Generic.List[string]]::new()
    $normalized = $RelativePath.Replace("\", "/")

    if ($Extension -in @(".resourcefile", ".unity3d")) {
        $tags.Add("bundles")
    }
    if ($normalized -match "(?i)(^|/)(tabledata|tdata)(/|$)|xdt") {
        $tags.Add("tables")
    }
    if ($normalized -match "(?i)dongresources|worldshared|terrain|(^|/)maps?(/|$)|map_\d\d_\d\d") {
        $tags.Add("world")
    }
    if ($normalized -match "(?i)character|npc|hnpc|mob|nano|fusion|player") {
        $tags.Add("characters")
    }
    if ($normalized -match "(?i)item|weapon|wear|equipment|icons?") {
        $tags.Add("items")
    }
    if (
        $Extension -in @(".cs", ".dll") -or
        $normalized -match "(?i)assembly|managed|script|monobehaviour"
    ) {
        $tags.Add("scripts")
    }
    if ($Extension -in @(".json", ".xml", ".txt", ".log", ".md")) {
        $tags.Add("metadata")
    }
    if ($tags.Count -eq 0) {
        $tags.Add("other")
    }

    return ($tags -join ",")
}

function Update-Inventory {
    param(
        [Parameter(Mandatory = $true)]
        $SourceRecord
    )

    $sourceRoot = Resolve-SourceRoot $SourceRecord
    if (-not (Test-Path -LiteralPath $sourceRoot -PathType Container)) {
        throw "Legacy source '$($SourceRecord.id)' does not exist: $sourceRoot"
    }

    $inventoryPath = Join-Path $hubRoot ([string]$SourceRecord.inventory)
    $inventoryDirectory = Split-Path -Parent $inventoryPath
    [System.IO.Directory]::CreateDirectory($inventoryDirectory) | Out-Null

    $prefix = $sourceRoot.TrimEnd("\") + "\"
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("relative_path`tbytes`textension`ttags")

    Get-ChildItem -LiteralPath $sourceRoot -Recurse -File |
        Where-Object {
            # Old raw folders can contain historical extraction/project residue.
            # Keep that residue out of raw-build inventories, but preserve the
            # cache itself when the registered source is explicitly cache-only.
            $rel = $_.FullName.Substring($prefix.Length)
            $SourceRecord.id -eq "reborn-cache" -or $rel -notmatch '^(cache|translations|work|\.ffclienteditor)[\\/]'
        } |
        Sort-Object FullName |
        ForEach-Object {
            $relativePath = $_.FullName.Substring($prefix.Length)
            $extension = $_.Extension.ToLowerInvariant()
            $tags = Get-InventoryTags -RelativePath $relativePath -Extension $extension
            $lines.Add("$relativePath`t$($_.Length)`t$extension`t$tags")
        }

    $utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllLines($inventoryPath, $lines, $utf8WithoutBom)
    Write-Host "Refreshed $($SourceRecord.id): $($lines.Count - 1) files -> $inventoryPath"
}

if ($Refresh) {
    foreach ($sourceRecord in $selectedSources) {
        Update-Inventory -SourceRecord $sourceRecord
    }
    if ($Query.Length -eq 0 -and $Kind -eq "all") {
        return
    }
}

$results = foreach ($sourceRecord in $selectedSources) {
    $inventoryPath = Join-Path $hubRoot ([string]$sourceRecord.inventory)
    if (-not (Test-Path -LiteralPath $inventoryPath -PathType Leaf)) {
        throw "Missing inventory for '$($sourceRecord.id)'. Run this command with -Refresh."
    }

    $sourceRoot = Resolve-SourceRoot $sourceRecord
    Import-Csv -LiteralPath $inventoryPath -Delimiter "`t" | ForEach-Object {
        $kindMatches = $Kind -eq "all" -or ([string]$_.tags).Split(",") -contains $Kind
        if (-not $kindMatches) {
            return
        }

        $queryMatches = $true
        if ($Query.Length -gt 0) {
            if ($Regex) {
                $queryMatches = [string]$_.relative_path -match $Query
            } else {
                $queryMatches = [string]$_.relative_path -like "*$([WildcardPattern]::Escape($Query))*"
            }
        }
        if (-not $queryMatches) {
            return
        }

        $displaySource = [string]$sourceRecord.id
        if ($Source -ne "all" -and $displaySource -ne $Source) {
            $displaySource = $Source
        }
        [pscustomobject]@{
            Source       = $displaySource
            CanonicalSource = [string]$sourceRecord.id
            Tags         = [string]$_.tags
            Bytes        = [int64]$_.bytes
            RelativePath = [string]$_.relative_path
            FullPath     = Join-Path $sourceRoot ([string]$_.relative_path)
        }
    }
}

if ($Limit -gt 0) {
    $results | Select-Object -First $Limit
} else {
    $results
}

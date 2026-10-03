[CmdletBinding()]
param(
    [string]$Source = "primary",

    [string]$Query = "",

    [string]$Type = "",

    [ValidateRange(0, 1000000)]
    [int]$Limit = 100
)

$ErrorActionPreference = "Stop"

$editorRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))

if ($Query.Length -eq 0 -and $Type.Length -eq 0) {
    throw "Specify -Query, -Type, or both. Example: find-objects.cmd -Type MonoBehaviour"
}

$sourceMap = Get-Content -LiteralPath (Join-Path $PSScriptRoot "sources.json") -Raw |
    ConvertFrom-Json
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
$selectedSources = @(
    $sourceMap.sources | Where-Object {
        $_.id -eq $Source -or @($_.aliases) -contains $Source
    }
)
if ($selectedSources.Count -ne 1) {
    throw "sources.json must resolve '$Source' to exactly one source"
}
$selectedSource = $selectedSources[0]

# Prefer the standalone CLI cache; old indexes remain read-only fallbacks.
$cacheRoots = @([string]$selectedSource.navigationWorkDir, [string]$selectedSource.navigationProject, [string]$selectedSource.path)
$bundleIndexPath = $null
foreach ($configuredRoot in $cacheRoots) {
    if ([string]::IsNullOrWhiteSpace($configuredRoot)) { continue }
    $root = if ([System.IO.Path]::IsPathRooted($configuredRoot)) {
        [System.IO.Path]::GetFullPath($configuredRoot)
    } else {
        [System.IO.Path]::GetFullPath((Join-Path $editorRoot $configuredRoot))
    }
    $candidate = Join-Path $root "cache/bundle-index.json"
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
        $bundleIndexPath = $candidate
        break
    }
}
if ($null -eq $bundleIndexPath) {
    throw "Missing bundle index. From FusionForge run: .\fusionforge.cmd index-client `"$($selectedSource.path)`" `"$($selectedSource.navigationWorkDir)`""
}

$bundleIndex = Get-Content -LiteralPath $bundleIndexPath -Raw | ConvertFrom-Json
$matches = foreach ($bundle in $bundleIndex.bundles) {
    foreach ($asset in $bundle.assets) {
        $typeProperty = $null
        if ($Type.Length -gt 0) {
            $typeProperty = $asset.typeCounts.PSObject.Properties |
                Where-Object { $_.Name -ieq $Type } |
                Select-Object -First 1
            if ($null -eq $typeProperty -or [int64]$typeProperty.Value -le 0) {
                continue
            }
        }

        $matchingPaths = @()
        if ($Query.Length -gt 0) {
            $queryMatchesBundle = (
                [string]$bundle.name -like "*$([WildcardPattern]::Escape($Query))*" -or
                [string]$bundle.path -like "*$([WildcardPattern]::Escape($Query))*" -or
                [string]$asset.name -like "*$([WildcardPattern]::Escape($Query))*"
            )
            $matchingPaths = @(
                $asset.containerPaths |
                    Where-Object { [string]$_ -like "*$([WildcardPattern]::Escape($Query))*" }
            )
            if (-not $queryMatchesBundle -and $matchingPaths.Count -eq 0) {
                continue
            }
        }

        $cacheAssetPath = ""
        $cacheDirectory = [string]$bundle.cacheDir
        if (-not [string]::IsNullOrWhiteSpace($cacheDirectory)) {
            $cacheAssetPath = [System.IO.Path]::Combine($cacheDirectory, ([string]$asset.name))
        }

        [pscustomobject]@{
            Source           = $Source
            CanonicalSource  = [string]$selectedSource.id
            Bundle           = [string]$bundle.name
            SerializedAsset  = [string]$asset.name
            ObjectCount      = [int64]$asset.objectCount
            RequestedType    = $Type
            RequestedCount   = if ($null -eq $typeProperty) { $null } else { [int64]$typeProperty.Value }
            MatchingRoutes   = ($matchingPaths | Select-Object -First 12) -join "; "
            RawBundlePath    = [string]$bundle.path
            CachedAssetPath  = $cacheAssetPath
        }
    }
}

if ($Limit -gt 0) {
    $matches | Select-Object -First $Limit
} else {
    $matches
}

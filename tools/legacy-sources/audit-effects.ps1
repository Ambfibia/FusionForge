param(
    [Parameter(Mandatory = $true)]
    [string]$NativeTargetRoot,

    [Parameter(Mandatory = $true)]
    [string]$ObjectDumpPath,

    [string]$ReportPath = (Join-Path $PSScriptRoot "../../work/legacy-sources/effects-source-retrobution-v2/coverage.json")
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$editorRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))

function Assert-Equal {
    param($Actual, $Expected, [string]$Label)
    if ($Actual -ne $Expected) {
        throw "$Label mismatch: expected $Expected, got $Actual"
    }
}

function Get-FileEvidence {
    param([string]$Path)
    $file = Get-Item -LiteralPath $Path
    [ordered]@{
        bytes = [uint64]$file.Length
        sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
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

$NativeTargetRoot = (Resolve-Path -LiteralPath $NativeTargetRoot).Path
$ObjectDumpPath = (Resolve-Path -LiteralPath $ObjectDumpPath).Path
$sourceMapPath = Join-Path $PSScriptRoot "sources.json"
$sourceMap = Get-Content -Raw -LiteralPath $sourceMapPath | ConvertFrom-Json
$primary = $sourceMap.sources | Where-Object id -eq "primary"
$patched = $sourceMap.sources | Where-Object id -eq "patched"
if (@($primary).Count -ne 1 -or @($patched).Count -ne 1) {
    throw "legacy source map must contain exactly one primary and one patched source"
}
Assert-Equal $primary.role "parity_authority" "primary source role"
Assert-Equal $patched.role "intentional_overlay" "patched source role"

$primaryRoot = Resolve-SourceRoot $primary
$rawContainerPath = Join-Path $primaryRoot "Effects.resourceFile"
$navigationProjectPath = [System.IO.Path]::GetFullPath((Join-Path $editorRoot ([string]$primary.navigationProject)))
$bundleIndexPath = Join-Path $navigationProjectPath "cache/bundle-index.json"
$effectCatalogPath = Join-Path $NativeTargetRoot "map/shared/effects/catalog.json"
$projectileCatalogPath = Join-Path $NativeTargetRoot "map/shared/projectiles/catalog.json"
$editorPrefix = $editorRoot.TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
$objectDumpRelative = if ($ObjectDumpPath.StartsWith($editorPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    $ObjectDumpPath.Substring($editorPrefix.Length).Replace("\", "/")
} else {
    "external/" + [System.IO.Path]::GetFileName($ObjectDumpPath)
}

$rawEvidence = Get-FileEvidence $rawContainerPath
Assert-Equal $rawEvidence.bytes 8130528 "primary Effects.resourceFile byte count"
Assert-Equal $rawEvidence.sha256 "2aa08e85b7aefa8f7e22705ee2f1581befa83cc5708b32116ce8a46a273595e9" "primary Effects.resourceFile SHA-256"

$indexEvidence = Get-FileEvidence $bundleIndexPath
$index = Get-Content -Raw -LiteralPath $bundleIndexPath | ConvertFrom-Json
$bundle = $index.bundles | Where-Object name -eq "Effects.resourceFile"
if (@($bundle).Count -ne 1) {
    throw "bundle index must contain exactly one Effects.resourceFile"
}
$serializedObject = $bundle.assets | Where-Object name -eq "CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918"
if (@($serializedObject).Count -ne 1) {
    throw "Effects.resourceFile must contain the expected serialized asset"
}
$serializedExtracted = $bundle.extractedFiles | Where-Object name -eq $serializedObject.name
if (@($serializedExtracted).Count -ne 1) {
    throw "bundle cache must contain the expected extracted serialized asset"
}
$serializedAssetPath = [System.IO.Path]::GetFullPath((Join-Path $editorRoot ([string]$serializedExtracted.path)))
$serializedAssetRelative = "cache/extracted-bundles/$((Split-Path $bundle.cacheDir -Leaf))/$($serializedExtracted.name)"
Assert-Equal ([uint64]$bundle.size) $rawEvidence.bytes "indexed Effects.resourceFile byte count"
Assert-Equal ([uint64]$serializedObject.objectCount) 19592 "serialized object count"

$effectRoutePattern = '^prefabs/particle/effectscripts/es\[(\d+)\]\.prefab$'
$sourceIds = @(
    foreach ($route in $serializedObject.containerPaths) {
        if ($route -match $effectRoutePattern) {
            [int]$Matches[1]
        }
    }
) | Sort-Object -Unique
$expectedIds = @(0..871)
if (@(Compare-Object $expectedIds $sourceIds).Count -ne 0) {
    throw "primary effect routes are not the expected continuous ES[0]..ES[846] range"
}

$effectCatalogEvidence = Get-FileEvidence $effectCatalogPath
$projectileCatalogEvidence = Get-FileEvidence $projectileCatalogPath
$effectCatalog = Get-Content -Raw -LiteralPath $effectCatalogPath | ConvertFrom-Json
$projectileCatalog = Get-Content -Raw -LiteralPath $projectileCatalogPath | ConvertFrom-Json
Assert-Equal $effectCatalog.sourceBuild "retrobution-20260821" "effect catalog source build"
Assert-Equal $projectileCatalog.sourceBuild "retrobution-20260821" "projectile catalog source build"
Assert-Equal $effectCatalog.sourceBundle.blake3 $projectileCatalog.sourceBundle.blake3 "catalog source-bundle proof"
Assert-Equal $effectCatalog.sourceDump.blake3 $projectileCatalog.sourceDump.blake3 "catalog source-dump proof"

$generalIds = @($effectCatalog.effects.effectId | ForEach-Object { [int]$_ } | Sort-Object -Unique)
$projectileIds = @($projectileCatalog.particleEffects.effectId | ForEach-Object { [int]$_ } | Sort-Object -Unique)
$publishedIds = @($generalIds + $projectileIds | Sort-Object -Unique)
$missingIds = @($sourceIds | Where-Object { $_ -notin $publishedIds })
$unexpectedIds = @($publishedIds | Where-Object { $_ -notin $sourceIds })
if ($unexpectedIds.Count -ne 0) {
    throw "published catalogs contain effect IDs absent from primary: $($unexpectedIds -join ', ')"
}

foreach ($entry in @($effectCatalog.effects) + @($projectileCatalog.particleEffects)) {
    $closure = Join-Path $NativeTargetRoot ([string]$entry.closurePath)
    $file = Get-Item -LiteralPath $closure
    Assert-Equal ([uint64]$file.Length) ([uint64]$entry.closureBytes) "closure byte count for ES[$($entry.effectId)]"
    if ($entry.closureBlake3 -notmatch '^[0-9a-f]{64}$') {
        throw "closure BLAKE3 proof is malformed for ES[$($entry.effectId)]"
    }
}

$serializedEvidence = Get-FileEvidence $serializedAssetPath
$dumpEvidence = Get-FileEvidence $objectDumpPath
Assert-Equal $serializedEvidence.bytes ([uint64]$effectCatalog.sourceBundle.bytes) "serialized asset byte count"
Assert-Equal $dumpEvidence.bytes ([uint64]$effectCatalog.sourceDump.bytes) "object dump byte count"

$report = [ordered]@{
    schema = "ffone.retrobution-effect-coverage.v1"
    generatedBy = "tools/legacy-sources/audit-effects.ps1@2"
    status = if ($missingIds.Count -eq 0) { "complete_runtime_publication" } else { "incomplete_runtime_publication" }
    sourceAlias = "primary"
    sourceBuild = "retrobution-20260821"
    source = [ordered]@{
        relativeContainerPath = "Effects.resourceFile"
        bytes = $rawEvidence.bytes
        sha256 = $rawEvidence.sha256
        serializedAsset = "CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918"
        objectCount = [uint64]$serializedObject.objectCount
        typeCounts = $serializedObject.typeCounts
        effectContainerRoutePattern = "prefabs/particle/effectscripts/es[<id>].prefab"
    }
    navigationEvidence = [ordered]@{
        sourceAlias = "primary"
        relativePath = "cache/bundle-index.json"
        bytes = $indexEvidence.bytes
        sha256 = $indexEvidence.sha256
        role = "primary_navigation_index_verified_against_primary_container"
    }
    extractionEvidence = [ordered]@{
        serializedAsset = [ordered]@{
            sourceAlias = "primary"
            relativePath = $serializedAssetRelative
            bytes = $serializedEvidence.bytes
            sha256 = $serializedEvidence.sha256
            blake3 = $effectCatalog.sourceBundle.blake3
        }
        objectDump = [ordered]@{
            sourceAlias = "project_generated"
            path = $objectDumpRelative
            bytes = $dumpEvidence.bytes
            sha256 = $dumpEvidence.sha256
            blake3 = $effectCatalog.sourceDump.blake3
        }
    }
    publication = [ordered]@{
        effectCatalog = [ordered]@{
            path = "map/shared/effects/catalog.json"
            bytes = $effectCatalogEvidence.bytes
            sha256 = $effectCatalogEvidence.sha256
            effects = [uint64]$generalIds.Count
            rendererStatus = $effectCatalog.rendererStatus
        }
        projectileCatalog = [ordered]@{
            path = "map/shared/projectiles/catalog.json"
            bytes = $projectileCatalogEvidence.bytes
            sha256 = $projectileCatalogEvidence.sha256
            effects = [uint64]$projectileIds.Count
            rendererStatus = $projectileCatalog.rendererStatus
        }
    }
    coverage = [ordered]@{
        sourceEffects = [uint64]$sourceIds.Count
        sourceRange = [ordered]@{ minimum = $sourceIds[0]; maximum = $sourceIds[-1]; continuous = $true }
        publishedGeneral = [uint64]$generalIds.Count
        publishedProjectile = [uint64]$projectileIds.Count
        publishedUnique = [uint64]$publishedIds.Count
        missing = [uint64]$missingIds.Count
        sourceIds = $sourceIds
        publishedIds = $publishedIds
        missingIds = $missingIds
    }
    conclusion = "The raw primary serialized asset and the full object dump cover every ES[0]..ES[846] source route. Runtime closure publication is intentionally selective and is not complete. Native Unity particle rendering remains a separate required implementation gate."
    intentionalDivergence = "Only effects selected by the tutorial, actor, world-EP and projectile installers are currently published; all other primary IDs remain offline extraction evidence."
    reproduction = @(
        "tools/legacy-sources/find.cmd -Source primary -Query effect -Limit 0",
        "tools/legacy-sources/find-objects.cmd -Query effect -Limit 0",
        "powershell -NoProfile -ExecutionPolicy Bypass -File tools/legacy-sources/audit-effects.ps1 -NativeTargetRoot <assets/game> -ObjectDumpPath <dump.json>"
    )
}

$absoluteReportPath = if ([IO.Path]::IsPathRooted($ReportPath)) {
    $ReportPath
} else {
    [System.IO.Path]::GetFullPath((Join-Path $editorRoot $ReportPath))
}
$reportDirectory = Split-Path -Parent $absoluteReportPath
[IO.Directory]::CreateDirectory($reportDirectory) | Out-Null
$json = $report | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($absoluteReportPath, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))

[pscustomobject]@{
    SourceEffects = $sourceIds.Count
    PublishedUnique = $publishedIds.Count
    Missing = $missingIds.Count
    Status = $report.status
    Report = $absoluteReportPath
}

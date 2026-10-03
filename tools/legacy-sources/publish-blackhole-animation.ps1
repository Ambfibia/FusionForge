<#
.SYNOPSIS
Publishes the exact clean-primary material clip for effect/blackhole.nif.

.DESCRIPTION
Input is the focused `fusionforge export-logical-model-source` evidence named
below. The script accepts only the checked Retrobution container and extraction
hashes, exact route/object identities, 40 source curves, and seven known target
paths. It never mutates a legacy build or makes it a runtime dependency.
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$SourcePath,

    [Parameter(Mandatory = $true)]
    [string]$PrimaryContainerPath,

    [Parameter(Mandatory = $true)]
    [string]$NativeTargetRoot,

    [string]$OutputRelativePath = "map/shared/effects/blackhole.nif-animation.json"
)

$ErrorActionPreference = "Stop"

$editorRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../.."))
$SourcePath = (Resolve-Path -LiteralPath $SourcePath).Path
$PrimaryContainerPath = (Resolve-Path -LiteralPath $PrimaryContainerPath).Path
$NativeTargetRoot = (Resolve-Path -LiteralPath $NativeTargetRoot).Path
if ([System.IO.Path]::IsPathRooted($OutputRelativePath) -or
    @($OutputRelativePath.Replace("\", "/").Split("/") | Where-Object { $_ -eq ".." }).Count -ne 0) {
    throw "OutputRelativePath must stay below NativeTargetRoot"
}
$OutputPath = [System.IO.Path]::GetFullPath((Join-Path $NativeTargetRoot $OutputRelativePath))
$nativePrefix = $NativeTargetRoot.TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
if (-not $OutputPath.StartsWith($nativePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "OutputRelativePath escapes NativeTargetRoot"
}
$editorPrefix = $editorRoot.TrimEnd("\", "/") + [System.IO.Path]::DirectorySeparatorChar
$sourceRelative = if ($SourcePath.StartsWith($editorPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    $SourcePath.Substring($editorPrefix.Length).Replace("\", "/")
} else {
    "external/" + [System.IO.Path]::GetFileName($SourcePath)
}

$source = Get-Content -LiteralPath $SourcePath -Raw | ConvertFrom-Json
if ($source.schema -ne "ffone.logical-model-source.v1") {
    throw "Unexpected logical-model source schema: $($source.schema)"
}
if ($source.exactContainerRoute -ne "effect/blackhole.nif") {
    throw "The source does not own effect/blackhole.nif"
}
if ($source.exactContainerTargets.assetName -ne "CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918" -or
    $source.exactContainerTargets.pathId -ne 9377) {
    throw "The exact blackhole root identity changed"
}

$animations = @($source.animations | Where-Object { $_.name -eq "nif-default" })
if ($animations.Count -ne 1) {
    throw "Expected exactly one nif-default clip, got $($animations.Count)"
}
$animation = $animations[0]
$sourceCurves = @($animation.animationData.floatCurves)
if (-not $animation.loop -or $sourceCurves.Count -ne 40) {
    throw "The clean blackhole clip must be looped and own exactly 40 material curves"
}

$expectedBindings = [ordered]@{
    "Cylinder01" = 0
    "Cylinder02" = 1
    "Cylinder03" = 2
    "Cylinder04" = 3
    "Object01/Editable Mesh2" = 4
    "Plane26_BillboardCamera/Editable Mesh" = 5
    "Plane27_BillboardCamera/Editable Mesh1" = 6
}
$actualPaths = @($sourceCurves | ForEach-Object { $_.path } | Sort-Object -Unique)
if (($actualPaths -join "|") -ne (($expectedBindings.Keys | Sort-Object) -join "|")) {
    throw "The clean blackhole material target set changed"
}

$floatCurves = @(
    for ($sourceIndex = 0; $sourceIndex -lt $sourceCurves.Count; $sourceIndex++) {
        $curve = $sourceCurves[$sourceIndex]
        $keys = @($curve.keys)
        [ordered]@{
            targetPath = $curve.path
            classId = [int64]$curve.classId
            attribute = $curve.property
            preInfinity = [int64]$curve.preInfinity
            postInfinity = [int64]$curve.postInfinity
            times = @($keys | ForEach-Object { [double]$_.time })
            values = @($keys | ForEach-Object { [double]$_.value })
            inTangents = @($keys | ForEach-Object { [double]$_.inTangent })
            outTangents = @($keys | ForEach-Object { [double]$_.outTangent })
        }
    }
)

$container = Get-Item -LiteralPath $PrimaryContainerPath
$containerSha256 = (Get-FileHash -LiteralPath $PrimaryContainerPath -Algorithm SHA256).Hash
$sourceFile = Get-Item -LiteralPath $SourcePath
$sourceSha256 = (Get-FileHash -LiteralPath $SourcePath -Algorithm SHA256).Hash
if ($container.Length -ne 3834593 -or
    $containerSha256 -ne "1AF8A0F359C5D7231C7C503E282861C6FB8846E4A350ACE0EB37B641613B5605") {
    throw "The clean-primary DongResources_07_08.resourceFile acceptance bytes changed"
}
if ($sourceFile.Length -ne 966250 -or
    $sourceSha256 -ne "AD72B063F52FC516EB0D46F4CC0442175BD9A66F1343B814655F7E995286BC84") {
    throw "The focused clean-primary blackhole logical-model extraction changed"
}

$contract = [ordered]@{
    schema = "ffone.world-effect-nif-animation.v1"
    id = "effect/blackhole.nif#nif-default"
    effectName = "blackhole"
    effectRoute = "effect/blackhole.nif"
    source = [ordered]@{
        alias = "primary"
        build = "retrobution-20260613"
        relativeContainerPath = "DongResources_07_08.resourceFile"
        containerBytes = [int64]$container.Length
        containerSha256 = $containerSha256
        containerAsset = "CustomAssetBundle-f084c5842e2eb4f738aa4218bbc4514c"
        route = "effect/blackhole.nif"
        rootAsset = "CustomAssetBundle-fa9dbcf4604f64024b06ff1d5e375918"
        rootPathId = 9377
        animationAsset = $animation.asset
        animationPathId = [int64]$animation.pathId
        logicalModelSource = $sourceRelative
        logicalModelSourceBytes = [int64]$sourceFile.Length
        logicalModelSourceSha256 = $sourceSha256
        extractionCommand = "fusionforge fusionforge export-logical-model-source <primary-container> effect/blackhole.nif <editor-work-source.json> <project.ffclient>"
        converter = "tools/legacy-sources/publish-blackhole-animation.ps1"
        converterVersion = "ffone.publish-blackhole-animation.v2"
        intentionalDivergence = "Material-only scope: all 40 source material curves are exact; the source clip's one compressed Object01 rotation channel is outside this texture-animation contract. _SpecColor curves remain retained even though the current exact native fixed-function replacement has no specular shader term."
    }
    modelBindings = @(
        foreach ($entry in $expectedBindings.GetEnumerator()) {
            [ordered]@{
                targetPath = $entry.Key
                modelIndex = [int64]$entry.Value
            }
        }
    )
    clip = [ordered]@{
        id = "effect/blackhole.nif#2108:nif-default"
        name = "nif-default"
        duration = [double]$animation.duration
        sampleRate = [double]$animation.sampleRate
        looped = [bool]$animation.loop
        channels = @()
        floatCurves = $floatCurves
        events = @()
    }
}

$parent = Split-Path -Parent $OutputPath
if ($parent) {
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
}
$json = $contract | ConvertTo-Json -Depth 100 -Compress
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($OutputPath, $json + [Environment]::NewLine, $utf8NoBom)
Write-Output "Published $OutputPath ($(([System.IO.FileInfo]$OutputPath).Length) bytes)"

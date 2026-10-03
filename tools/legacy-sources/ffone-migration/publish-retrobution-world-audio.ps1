param(
    [Parameter(Mandatory = $true)]
    [string]$NativeTargetRoot,

    [Parameter(Mandatory = $true)]
    [string]$EnvironmentEvidencePath,

    [Parameter(Mandatory = $true)]
    [string]$MusicEvidencePath,

    [Parameter(Mandatory = $true)]
    [string]$RetroBundleEvidencePath,

    [string]$OutputRelativePath = 'data/audio/retrobution-world.json'
)

$ErrorActionPreference = 'Stop'
$editorRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$sourceMapPath = Join-Path $PSScriptRoot '../sources.json'
$sourceMap = Get-Content -LiteralPath $sourceMapPath -Raw | ConvertFrom-Json
$primary = @($sourceMap.sources | Where-Object id -eq 'primary')
$patched = @($sourceMap.sources | Where-Object id -eq 'patched')
if ($primary.Count -ne 1 -or $patched.Count -ne 1) {
    throw 'sources.json must define exactly one primary and one patched source'
}
if ($primary[0].role -ne 'parity_authority' -or $patched[0].role -ne 'intentional_overlay') {
    throw 'sources.json source roles do not match the legacy-source boundary'
}
function Resolve-SourceRoot($SourceRecord) {
    $configured = [string]$SourceRecord.path
    if ([IO.Path]::IsPathRooted($configured)) { return [IO.Path]::GetFullPath($configured) }
    return [IO.Path]::GetFullPath((Join-Path $editorRoot $configured))
}
$PrimaryRoot = Resolve-SourceRoot $primary[0]
$PatchedCacheRoot = Join-Path (Resolve-SourceRoot $patched[0]) 'cache'
$NativeTargetRoot = (Resolve-Path -LiteralPath $NativeTargetRoot).Path
$EnvironmentEvidencePath = (Resolve-Path -LiteralPath $EnvironmentEvidencePath).Path
$MusicEvidencePath = (Resolve-Path -LiteralPath $MusicEvidencePath).Path
$RetroBundleEvidencePath = (Resolve-Path -LiteralPath $RetroBundleEvidencePath).Path
if ([IO.Path]::IsPathRooted($OutputRelativePath) -or
    @($OutputRelativePath.Replace('\', '/').Split('/') | Where-Object { $_ -eq '..' }).Count -ne 0) {
    throw 'OutputRelativePath must stay below NativeTargetRoot'
}
$outputPath = [IO.Path]::GetFullPath((Join-Path $NativeTargetRoot $OutputRelativePath))
$nativePrefix = $NativeTargetRoot.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
if (-not $outputPath.StartsWith($nativePrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'OutputRelativePath escapes NativeTargetRoot'
}
$audioCatalogPath = Join-Path $NativeTargetRoot '_runtime/audio.json'
$bundleIndexPath = Join-Path $PatchedCacheRoot 'bundle-index.json'

$environmentEvidence = Get-Content -LiteralPath $environmentEvidencePath -Raw | ConvertFrom-Json
$musicEvidence = Get-Content -LiteralPath $musicEvidencePath -Raw | ConvertFrom-Json
$retroBundleEvidence = Get-Content -LiteralPath $retroBundleEvidencePath -Raw | ConvertFrom-Json
$audioCatalog = Get-Content -LiteralPath $audioCatalogPath -Raw | ConvertFrom-Json
$bundleIndex = Get-Content -LiteralPath $bundleIndexPath -Raw | ConvertFrom-Json

if ($environmentEvidence.schema -ne 'ffone.recovery.retrobution-environment-sounds.v1') {
    throw "Unexpected environment evidence schema: $($environmentEvidence.schema)"
}
if ($audioCatalog.schema -ne 'ffone.semantic-audio-catalog.v5') {
    throw "World-audio publication requires the editable v5 audio catalog"
}

$audioByTrueName = @{}
$audioByLogicalKey = @{}
foreach ($asset in $audioCatalog.assets) {
    if (!$audioByTrueName.ContainsKey($asset.trueName)) {
        $audioByTrueName[$asset.trueName] = @()
    }
    $audioByTrueName[$asset.trueName] += $asset
    if ($audioByLogicalKey.ContainsKey($asset.logicalKey)) {
        throw "Duplicate audio logical key: $($asset.logicalKey)"
    }
    $audioByLogicalKey[$asset.logicalKey] = $asset
}

function Resolve-AudioLogicalKey {
    param(
        [Parameter(Mandatory = $true)][string]$TrueName,
        [Parameter(Mandatory = $true)][ValidateSet('music', 'ambient', 'environment')][string]$Usage
    )
    if (!$audioByTrueName.ContainsKey($TrueName)) {
        return $null
    }
    $candidates = @($audioByTrueName[$TrueName])
    if ($Usage -eq 'environment') {
        $preferred = @($candidates | Where-Object {
            $_.category -eq 'sfx' -and $_.logicalKey -notlike 'sfx/character_creation/*'
        })
    } elseif ($Usage -eq 'music') {
        $preferred = @($candidates | Where-Object { $_.category -eq 'music' })
    } else {
        $preferred = @($candidates | Where-Object { $_.category -eq 'ambient' })
    }
    if ($preferred.Count -eq 0) {
        $preferred = $candidates
    }
    if ($preferred.Count -ne 1) {
        throw "$Usage audio $TrueName resolves to $($preferred.Count) preferred native assets"
    }
    return [string]$preferred[0].logicalKey
}

$retroClipNames = @{}
foreach ($clip in $bundleIndex.audioClips | Where-Object { $_.container -eq 'RetroMusic.resourceFile' }) {
    $retroClipNames[[int64]$clip.pathId] = [string]$clip.name
}
$retroRoutes = @{}
foreach ($entry in $retroBundleEvidence.value.m_Container) {
    $pathId = [int64]$entry[1].asset.pathId
    if (!$retroClipNames.ContainsKey($pathId)) {
        throw "RetroMusic route $($entry[0]) points to unknown AudioClip pathId $pathId"
    }
    $retroRoutes[([string]$entry[0]).ToLowerInvariant()] = $retroClipNames[$pathId]
}

function Convert-MusicRows {
    param(
        [Parameter(Mandatory = $true)][array]$Rows,
        [Parameter(Mandatory = $true)][ValidateSet('music', 'ambient')][string]$Channel
    )
    $converted = @()
    foreach ($row in $Rows) {
        $sourceFileName = [string]$row.fileName
        $silent = $sourceFileName.Equals('none.ogg', [StringComparison]::OrdinalIgnoreCase)
        $trueName = $null
        $logicalKey = $null
        $resolution = if ($silent) { 'silence' } else { 'resolved' }
        if (!$silent) {
            $route = ('bgm/' + $sourceFileName).ToLowerInvariant()
            $trueName = if ($retroRoutes.ContainsKey($route)) {
                [string]$retroRoutes[$route]
            } else {
                [IO.Path]::GetFileNameWithoutExtension($sourceFileName)
            }
            $logicalKey = Resolve-AudioLogicalKey -TrueName $trueName -Usage $Channel
            if ($null -eq $logicalKey) {
                $resolution = 'legacySourceMissing'
            }
        }
        $converted += [ordered]@{
            index = [int]$row.index
            name = [string]$row.name
            instance = ([int]$row.bInstanceFlag -ne 0)
            delaySeconds = [int]$row.delay
            sourceFileName = $sourceFileName
            sourceTrueName = $trueName
            logicalKey = $logicalKey
            resolution = $resolution
            # Unary comma keeps each X/Z pair as one JSON array; without it,
            # PowerShell flattens the polygon into a single number list.
            points = @($row.points | ForEach-Object { ,@([double]$_.x, [double]$_.y) })
        }
    }
    return $converted
}

$hashCache = @{}
function Get-SourceAcceptance {
    param([Parameter(Mandatory = $true)][string]$RelativePath)
    if ($hashCache.ContainsKey($RelativePath)) {
        return $hashCache[$RelativePath]
    }
    $path = Join-Path $PrimaryRoot $RelativePath
    $file = Get-Item -LiteralPath $path
    $acceptance = [ordered]@{
        alias = 'primary'
        relativePath = $RelativePath.Replace('\', '/')
        bytes = [int64]$file.Length
        sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    $hashCache[$RelativePath] = $acceptance
    return $acceptance
}

$environmentTiles = @()
foreach ($group in $environmentEvidence.tiles | Group-Object objectName | Sort-Object Name) {
    $coordinates = $group.Name -replace '^sound_', ''
    $expectedContainer = "DongResources_$coordinates.resourceFile"
    $source = $group.Group | Where-Object { $_.containerRelativePath -eq $expectedContainer } | Select-Object -First 1
    if ($null -eq $source) {
        $source = $group.Group | Where-Object { $_.containerRelativePath -eq 'Tutorial.resourceFile' } | Select-Object -First 1
    }
    if ($null -eq $source) {
        $source = $group.Group | Select-Object -First 1
    }
    $emitters = @()
    foreach ($sound in $source.sounds) {
        $sourceFileName = [string]$sound.FileName
        if ([string]::IsNullOrWhiteSpace($sourceFileName)) {
            continue
        }
        $trueName = [IO.Path]::GetFileNameWithoutExtension($sourceFileName)
        $logicalKey = Resolve-AudioLogicalKey -TrueName $trueName -Usage environment
        if ($null -eq $logicalKey) {
            throw "Environment sound $sourceFileName from $($group.Name) has no native audio route"
        }
        $emitters += [ordered]@{
            sourceFileName = $sourceFileName
            sourceTrueName = $trueName
            logicalKey = $logicalKey
            position = @(-[double]$sound.Position.x, [double]$sound.Position.y, [double]$sound.Position.z)
        }
    }
    if ($emitters.Count -eq 0) {
        continue
    }
    $parts = $coordinates.Split('_')
    $environmentTiles += [ordered]@{
        tile = @([int]$parts[0], [int]$parts[1])
        scope = if ($source.containerRelativePath -eq 'Tutorial.resourceFile') { 'tutorial' } else { 'worldMap' }
        source = Get-SourceAcceptance -RelativePath ([string]$source.containerRelativePath)
        object = [ordered]@{
            extractedAsset = [string]$source.extractedAsset
            name = [string]$source.objectName
            pathId = [int64]$source.pathId
            scriptFileId = [int64]$source.scriptFileId
            scriptPathId = [int64]$source.scriptPathId
        }
        emitters = $emitters
    }
}

$musicZones = Convert-MusicRows -Rows @($musicEvidence.value.musicDatas) -Channel music
$ambientZones = Convert-MusicRows -Rows @($musicEvidence.value.ambientDatas) -Channel ambient
$mainAcceptance = Get-SourceAcceptance -RelativePath 'main.unity3d'
$retroMusicAcceptance = Get-SourceAcceptance -RelativePath 'RetroMusic.resourceFile'
$document = [ordered]@{
    schema = 'ffone.retrobution-world-audio.v1'
    coordinateSpace = 'native world; H=diag(-1,1,1), one Unity unit equals one Bevy unit; music polygons retain Unity X/Z and runtime reflects native X before testing'
    source = [ordered]@{
        musicDataStorage = [ordered]@{
            container = $mainAcceptance
            extractedAsset = 'sharedassets0.assets'
            objectName = 'MusicDataStorage'
            pathId = 1391
        }
        retroMusicRoutes = [ordered]@{
            container = $retroMusicAcceptance
            extractedAsset = 'CustomAssetBundle-RetroMusic'
            objectType = 'AssetBundle'
            pathId = 1
        }
    }
    conversion = [ordered]@{
        version = 'ffone.retrobution-world-audio.v1'
        command = 'powershell -NoProfile -File tools/legacy-sources/ffone-migration/publish-retrobution-world-audio.ps1 -NativeTargetRoot <assets/game> -EnvironmentEvidencePath <environment.json> -MusicEvidencePath <music.json> -RetroBundleEvidencePath <routes.json>'
        intentionalDivergence = 'Legacy filenames are converted to editable semantic logical keys; four clean-primary missing BGM loads remain explicit legacySourceMissing rows and are not substituted.'
    }
    counts = [ordered]@{
        musicZones = $musicZones.Count
        ambientZones = $ambientZones.Count
        environmentTiles = $environmentTiles.Count
        environmentEmitters = [int](($environmentTiles | ForEach-Object { $_.emitters.Count } | Measure-Object -Sum).Sum)
    }
    musicZones = $musicZones
    ambientZones = $ambientZones
    environmentTiles = $environmentTiles
}

$parent = Split-Path -Parent $outputPath
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$utf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($outputPath, (($document | ConvertTo-Json -Depth 20) + "`n"), $utf8)
Write-Output "Published $outputPath ($($document.counts.musicZones) music zones, $($document.counts.ambientZones) ambient zones, $($document.counts.environmentEmitters) environment emitters)."

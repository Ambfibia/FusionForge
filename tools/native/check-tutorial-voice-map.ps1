param(
    [Parameter(Mandatory = $true)][string]$NativeSource,
    [Parameter(Mandatory = $true)][string]$ManagedSource
)
$ErrorActionPreference = 'Stop'
$native = Get-Content -LiteralPath $NativeSource -Raw
$source = Get-Content -LiteralPath $ManagedSource -Raw
$mapped = @{}
foreach ($match in [regex]::Matches($source, 'VoiceOut\("([^"]+)", TextManager.GetSceneText\(2, (\d+)\), ([\d.]+)f\);')) {
    $key = $match.Groups[1].Value.ToLowerInvariant()
    $value = "$($match.Groups[2].Value):$($match.Groups[3].Value)"
    if ($mapped.ContainsKey($key) -and $mapped[$key] -ne $value) {
        throw "Conflicting source cue: $key"
    }
    $mapped[$key] = $value
}
$nativeCount = 0
foreach ($match in [regex]::Matches($native, '(?:override_)?spec\("([^"]+)", (\d+), ([\d.]+)(?:,|\))')) {
    $key = $match.Groups[1].Value.ToLowerInvariant()
    $nativeCount++
    if (!$mapped.ContainsKey($key)) { throw "Missing source cue: $key" }
    $actual = $mapped[$key].Split(':')
    $culture = [Globalization.CultureInfo]::InvariantCulture
    if ([int]$actual[0] -ne [int]$match.Groups[2].Value -or
        [double]::Parse($actual[1], $culture) -ne [double]::Parse($match.Groups[3].Value, $culture)) {
        throw "Line or duration mismatch: $key"
    }
}
if ($nativeCount -ne 87 -or $mapped.Count -ne $nativeCount) {
    throw "Incomplete cue map: native=$nativeCount, source=$($mapped.Count)"
}
Write-Output "Verified $nativeCount native tutorial voice mappings against managed source."

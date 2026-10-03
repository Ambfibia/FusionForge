param(
    [string]$ClientRoot = "$PSScriptRoot/../../../FFOneClient",
    [switch]$Gpu,
    [switch]$Server,
    [string]$Make = 'make',
    [string]$Cxx = 'g++',
    [string]$Cc = 'gcc'
)
$ErrorActionPreference = 'Stop'
$creationRoot = (Resolve-Path -LiteralPath $ClientRoot).Path
Push-Location -LiteralPath $creationRoot
try {
    $checks = @(
        @('test', '-p', 'ffone-client', '--bin', 'ffone-client', 'app::tests::character'),
        @('test', '-p', 'ffone-client', '--lib', 'character_creation'),
        @('test', '-p', 'ffone-client', '--test', 'character_ui_localization_standalone'),
        @('test', '-p', 'ffone-client-network', 'character_creation_worker'),
        @('test', '-p', 'ffone-net', 'character_creation')
    )
    foreach ($check in $checks) {
        & cargo @check
        if ($LASTEXITCODE -ne 0) { throw "Character creation check failed: cargo $check" }
    }
    if ($Server) {
        & "$creationRoot/../OpenFusion/tests/run-character-creation.ps1" -Make $Make -Cxx $Cxx -Cc $Cc
        if ($LASTEXITCODE -ne 0) { throw 'Server creation regression failed' }
    }
    if ($Gpu) {
        $creationOutput = Join-Path $creationRoot "target/performance/character-creation/run-$(Get-Date -Format 'yyyyMMdd-HHmmss-fff')"
        $previousOutput = $env:FFONE_CREATION_PROBE_OUTPUT
        try {
            $env:FFONE_CREATION_PROBE_OUTPUT = $creationOutput
            & cargo run -p ffone-client --bin ffone-client
            if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath "$creationOutput/passed.txt")) {
                throw "Production GPU creation flow failed; inspect $creationOutput/status.txt"
            }
            Write-Host "Creation GPU evidence: $creationOutput"
        } finally {
            $env:FFONE_CREATION_PROBE_OUTPUT = $previousOutput
        }
    }
} finally {
    Pop-Location
}

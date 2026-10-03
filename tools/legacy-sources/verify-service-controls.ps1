param([string]$NativeRoot='../FFOneClient', [string]$OutputRoot='work/legacy-sources/whole-client-audit-20260905/service-controls-final', [ValidateSet('bank','vendor')][string]$Group='bank')
$ErrorActionPreference='Stop'
$controlNative=(Resolve-Path -LiteralPath $NativeRoot).Path
$controlOutput=[IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputRoot))
New-Item -ItemType Directory -Force -Path $controlOutput | Out-Null
$controlModes=@('scroll-up','scroll-hold','scroll-thumb','scroll-track','inventory-scroll')
if ($Group -eq 'bank') { $controlModes+=@('drag-hold','drag-drop','drag-cancel','drag-modal') }
Push-Location $controlNative
try {
    foreach ($controlMode in $controlModes) {
        foreach ($controlLocale in @('en','ru')) {
            $env:FFONE_SERVICE_CONTROL=$controlMode
            $controlStem=Join-Path $controlOutput "$Group-$controlMode-$controlLocale"
            $controlExe="./target/debug/examples/${Group}_ui_gpu_preview.exe"
            & $controlExe $controlLocale "$controlStem.png" *> "$controlStem.log"
            if ($LASTEXITCODE -ne 0) { throw "$Group/$controlMode/$controlLocale failed: $controlStem.log" }
            $controlLog=Get-Content -LiteralPath "$controlStem.log" -Raw
            if ($controlLog -notmatch "PASS $Group") { throw "Missing pointer assertions: $controlStem.log" }
            Write-Output "PASS $Group/$controlMode/$controlLocale"
        }
    }
} finally { Remove-Item Env:FFONE_SERVICE_CONTROL -ErrorAction SilentlyContinue; Pop-Location }

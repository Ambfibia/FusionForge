param(
    [string]$NativeRoot = '../FFOneClient',
    [string]$OutputRoot = 'work/legacy-sources/whole-client-audit-20260905/card-final-20260907',
    [ValidateSet('vendor','bank','inventory')][string]$Group = 'vendor'
)
$ErrorActionPreference = 'Stop'
$cardNativeRoot = (Resolve-Path -LiteralPath $NativeRoot).Path
$cardOutputRoot = [IO.Path]::GetFullPath((Join-Path (Get-Location) $OutputRoot))
New-Item -ItemType Directory -Path $cardOutputRoot -Force | Out-Null
Push-Location $cardNativeRoot
try {
    if ($Group -eq 'vendor') {
        foreach ($cardMode in @('long-name','rental','expires','vehicle','combined','quantity','buyback','sell','chest-open','try-on-close','localized-general')) {
            foreach ($cardLocale in @('en','ru')) {
                $env:FFONE_VENDOR_POPUP = $cardMode
                $cardStem = Join-Path $cardOutputRoot "vendor-$cardMode-$cardLocale"
                & ./target/debug/examples/vendor_ui_gpu_preview.exe $cardLocale "$cardStem.png" *> "$cardStem.log"
                if ($LASTEXITCODE -ne 0) { throw "Vendor $cardMode/$cardLocale failed; see $cardStem.log" }
                Write-Output "PASS vendor/$cardMode/$cardLocale"
            }
        }
    } elseif ($Group -eq 'bank') {
        foreach ($cardMode in @('long-name','rental','expires','combined','general','chest','transfer','delete-accept','delete-cancel')) {
            foreach ($cardLocale in @('en','ru')) {
                $env:FFONE_BANK_POINTER = "popup-$cardMode"
                $cardStem = Join-Path $cardOutputRoot "bank-$cardMode-$cardLocale"
                & ./target/debug/examples/bank_ui_gpu_preview.exe $cardLocale "$cardStem.png" *> "$cardStem.log"
                if ($LASTEXITCODE -ne 0) { throw "Bank $cardMode/$cardLocale failed; see $cardStem.log" }
                Write-Output "PASS bank/$cardMode/$cardLocale"
            }
        }
    } else {
        foreach ($cardMode in @('long-name','rental','expires','combined','general','chest','equip-action')) {
            foreach ($cardLocale in @('en','ru')) {
                $env:FFONE_USER_EQUIP_LOCALE = $cardLocale
                $env:FFONE_USER_EQUIP_ITEM_POPUP = '1'
                $env:FFONE_USER_EQUIP_POPUP_SLOT = '0'
                Remove-Item Env:FFONE_USER_EQUIP_CARD_ACTION -ErrorAction SilentlyContinue
                if ($cardMode -in @('general','chest')) {
                    Remove-Item Env:FFONE_USER_EQUIP_CARD -ErrorAction SilentlyContinue
                    $env:FFONE_USER_EQUIP_POPUP_SLOT = if ($cardMode -eq 'general') {'1'} else {'2'}
                } else {
                    $env:FFONE_USER_EQUIP_CARD = $cardMode
                    if ($cardMode -eq 'equip-action') { $env:FFONE_USER_EQUIP_CARD_ACTION = '1' }
                }
                $cardStem = Join-Path $cardOutputRoot "inventory-$cardMode-$cardLocale"
                & ./target/debug/examples/user_equip_ui_gpu_preview.exe "$cardStem.png" *> "$cardStem.log"
                if ($LASTEXITCODE -ne 0) { throw "Inventory $cardMode/$cardLocale failed; see $cardStem.log" }
                Write-Output "PASS inventory/$cardMode/$cardLocale"
            }
        }
    }
} finally { Pop-Location }

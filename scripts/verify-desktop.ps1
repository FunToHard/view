[CmdletBinding()]
param([switch]$SelectedDesktop,[ValidateSet('en-US','ja-JP')][string]$Scenario='en-US')
$ErrorActionPreference='Stop'
if (-not $SelectedDesktop) { throw 'Select an unlocked desktop explicitly with -SelectedDesktop. F12 stops input.' }
if (-not $IsWindows) { throw 'Windows desktop driver is required; unsupported is not a passed test.' }
Push-Location (Split-Path $PSScriptRoot -Parent)
$priorIncremental=$env:CARGO_INCREMENTAL
try {
    $env:CARGO_INCREMENTAL='0'
    $mode=if ($Scenario -eq 'ja-JP') { '--desktop-ime' } else { '--desktop-test' }
    & cargo run -p view-application-proofs --features desktop-test --locked -- $mode --selected-desktop
    if ($LASTEXITCODE -ne 0) { throw "Desktop scenario $Scenario failed or is uncovered ($LASTEXITCODE). See target/verification/m5-*.txt." }
} finally { $env:CARGO_INCREMENTAL=$priorIncremental; Pop-Location }

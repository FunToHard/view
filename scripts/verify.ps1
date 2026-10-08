[CmdletBinding()]
param([switch]$Gpu, [ValidateSet('dx12','vulkan','metal')][string]$Backend = 'dx12')
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
$exitStatus = 0
$priorIncremental = $env:CARGO_INCREMENTAL
$env:CARGO_INCREMENTAL = '0'
try {
    New-Item -ItemType Directory -Force target/verification | Out-Null
    Start-Transcript -Path target/verification/verify.log -Force | Out-Null
    function Invoke-Cargo {
        param([string[]]$Arguments)
        & cargo @Arguments
        if ($LASTEXITCODE -ne 0) { throw "cargo $Arguments failed ($LASTEXITCODE)" }
    }
    $compiler = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw 'rustc unavailable' }
    $manifest = [ordered]@{
        timestampUtc = [DateTime]::UtcNow.ToString('o')
        os = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
        architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        compiler = $compiler
        runnerOS = $env:RUNNER_OS
        imageOS = $env:ImageOS
        imageVersion = $env:ImageVersion
        gpuRequested = [bool]$Gpu
        nativeInputRequested = $false
    }
    $manifest | ConvertTo-Json -Depth 4 | Set-Content target/verification/environment.json
    & "$PSScriptRoot/check-fixtures.ps1"
    if ($LASTEXITCODE -ne 0) { throw 'Fixture verification failed' }
    Invoke-Cargo @('fmt','--all','--','--check')
    Invoke-Cargo @('check','--workspace','--all-targets','--locked')
    Invoke-Cargo @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
    Invoke-Cargo @('test','--workspace','--locked')
    Invoke-Cargo @('doc','--workspace','--no-deps','--locked')
    Invoke-Cargo @('test','-p','view-core','--no-default-features','--locked')
    Invoke-Cargo @('test','-p','view-testing','--no-default-features','--locked')
    Invoke-Cargo @('test','-p','view-platform','--locked')
    Invoke-Cargo @('run','-p','view-testing','--example','headless','--locked')
    # A hidden window still requires a native desktop/display server.
    if ($IsWindows) { Invoke-Cargo @('run','-p','view-platform','--example','inspectable_app','--locked','--','--run-hidden') }
    if ($IsWindows) {
        foreach ($stage in @('ready','idle','event')) {
            Invoke-Cargo @('run','-p','view-platform','--example','callback_failure','--locked','--','--run-hidden',$stage)
        }
    }
    Invoke-Cargo @('check','-p','view','--no-default-features','--locked')
    Invoke-Cargo @('check','-p','view','--all-features','--locked')
    # Independent workspace/lockfile, unaffected by root feature unification.
    Invoke-Cargo @('check','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('run','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','harness','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('fmt','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--','--check')
    $hostTriple = ($compiler | Where-Object { $_ -like 'host: *' }) -replace '^host: ', ''
    & "$PSScriptRoot/dependency-inventory.ps1" -Target $hostTriple
    if ($IsWindows) { Invoke-Cargo @('run','-p','platform-probe','--locked') }
    if ($Gpu) { Invoke-Cargo @('run','-p','dependency-probe','--bin','gpu','--locked','--',$Backend) }
} catch {
    Write-Host $_ -ForegroundColor Red
    $exitStatus = 1
} finally {
    try { Stop-Transcript | Out-Null } catch { }
    Pop-Location
    $env:CARGO_INCREMENTAL = $priorIncremental
}
exit $exitStatus

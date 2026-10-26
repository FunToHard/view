[CmdletBinding()]
param([switch]$Gpu, [ValidateSet('dx12','vulkan','metal')][string]$Backend = 'dx12', [switch]$NativeClipboard, [switch]$NativeWebview)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
$exitStatus = 0
$priorIncremental = $env:CARGO_INCREMENTAL
$env:CARGO_INCREMENTAL = '0'
try {
    if ($NativeClipboard -and -not $IsWindows) { throw 'Native clipboard verification currently requires Windows.' }
    if ($NativeWebview -and -not $IsWindows) { throw 'Native WebView2 verification requires Windows.' }
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
        nativeClipboardRequested = [bool]$NativeClipboard
        nativeWebviewRequested = [bool]$NativeWebview
    }
    $manifest | ConvertTo-Json -Depth 4 | Set-Content target/verification/environment.json
    & "$PSScriptRoot/check-fixtures.ps1"
    if ($LASTEXITCODE -ne 0) { throw 'Fixture verification failed' }
    Invoke-Cargo @('fmt','--all','--','--check')
    Invoke-Cargo @('check','--workspace','--all-targets','--locked')
    Invoke-Cargo @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
    Invoke-Cargo @('test','--workspace','--locked')
    Invoke-Cargo @('doc','--workspace','--no-deps','--locked')
    Invoke-Cargo @('clippy','-p','view-diagnostics','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view-diagnostics','--all-features','--locked')
    Invoke-Cargo @('doc','-p','view-diagnostics','--all-features','--no-deps','--locked')
    Invoke-Cargo @('run','-p','view-addons','--bin','review-addon','--locked','--','--proof')
    Invoke-Cargo @('clippy','-p','view-addons','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view-addons','--all-features','--locked')
    Invoke-Cargo @('doc','-p','view-addons','--all-features','--no-deps','--locked')
    Invoke-Cargo @('clippy','-p','view-webview','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view-webview','--all-features','--locked')
    Invoke-Cargo @('doc','-p','view-webview','--all-features','--no-deps','--locked')
    if ($NativeWebview) { Invoke-Cargo @('run','-p','view-webview','--features','native','--example','panel','--locked','--','--native-check') }
    Invoke-Cargo @('clippy','-p','view-automation','-p','view-tooling','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view-automation','-p','view-tooling','--all-features','--locked')
    Invoke-Cargo @('doc','-p','view-automation','-p','view-tooling','--all-features','--no-deps','--locked')
    Invoke-Cargo @('clippy','-p','view','-p','view-resource','-p','view-markdown','-p','view-application-proofs','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view','-p','view-resource','-p','view-markdown','-p','view-application-proofs','--all-targets','--all-features','--locked')
    Invoke-Cargo @('doc','-p','view','-p','view-resource','-p','view-markdown','-p','view-application-proofs','--all-features','--no-deps','--locked')
    Invoke-Cargo @('run','-p','view-application-proofs','--locked')
    foreach ($example in @('minimal','form','keyed_list','async_branch','two_windows','immediate','custom_control','viewport')) {
        Invoke-Cargo @('run','-p','view','--features','controls,resources','--example',$example,'--locked')
    }
    Invoke-Cargo @('test','-p','view-core','--no-default-features','--locked')
    Invoke-Cargo @('test','-p','view-testing','--no-default-features','--locked')
    Invoke-Cargo @('test','-p','view-text','--no-default-features','--locked')
    Invoke-Cargo @('test','-p','view-wgpu','--locked')
    Invoke-Cargo @('clippy','-p','view-wgpu','--all-targets','--features','scene','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view-wgpu','--features','scene','--locked')
    Invoke-Cargo @('doc','-p','view-wgpu','--features','scene','--no-deps','--locked')
    Invoke-Cargo @('test','-p','view-controls','--no-default-features','--locked')
    Invoke-Cargo @('clippy','-p','view-controls','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view-controls','--all-features','--locked')
    Invoke-Cargo @('test','-p','view-controls','--features','gallery','--example','gallery','--locked')
    Invoke-Cargo @('doc','-p','view-controls','--all-features','--no-deps','--locked')
    Invoke-Cargo @('run','-p','view-controls','--features','gallery','--example','gallery','--locked','--','--headless')
    Invoke-Cargo @('clippy','-p','view-text','-p','view-platform','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-Cargo @('test','-p','view-text','-p','view-platform','--all-features','--locked')
    Invoke-Cargo @('doc','-p','view-text','-p','view-platform','--all-features','--no-deps','--locked')
    Invoke-Cargo @('test','-p','view-platform','--locked')
    Invoke-Cargo @('run','-p','view-testing','--example','headless','--locked')
    # A hidden window still requires a native desktop/display server.
    if ($IsWindows) { Invoke-Cargo @('run','-p','view-platform','--example','inspectable_app','--locked','--','--run-hidden') }
    if ($IsWindows) {
        if ($NativeClipboard) { Invoke-Cargo @('run','-p','view-platform','--features','clipboard-test','--example','clipboard_isolated','--locked','--','--isolated') }
        foreach ($stage in @('ready','idle','event')) {
            Invoke-Cargo @('run','-p','view-platform','--example','callback_failure','--locked','--','--run-hidden',$stage)
        }
    }
    Invoke-Cargo @('check','-p','view','--no-default-features','--locked')
    Invoke-Cargo @('check','-p','view','--all-features','--locked')
    # Independent workspace/lockfile, unaffected by root feature unification.
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','addons','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','webview','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('check','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','webview-native','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','automation','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('check','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('run','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','harness','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','text','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','text-shaping','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','wgpu','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','scene','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','markdown,resources','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','controls','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','accessibility','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--features','controls-render','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('test','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--all-features','--locked','--target-dir','target/consumer')
    Invoke-Cargo @('fmt','--manifest-path','tests/compatibility/facade-consumer/Cargo.toml','--','--check')
    $hostTriple = ($compiler | Where-Object { $_ -like 'host: *' }) -replace '^host: ', ''
    & "$PSScriptRoot/dependency-inventory.ps1" -Target $hostTriple
    if ($IsWindows) { Invoke-Cargo @('run','-p','platform-probe','--locked') }
    if ($Gpu) {
        Invoke-Cargo @('run','-p','view-addons','--features','render','--bin','review-addon','--locked','--','--gpu',$Backend)
        Invoke-Cargo @('run','-p','view-application-proofs','--features','automation','--locked','--','--gpu',$Backend)
        if ($IsWindows) { Invoke-Cargo @('run','-p','view-application-proofs','--features','native','--locked','--','--run-hidden') }
        Invoke-Cargo @('run','-p','dependency-probe','--bin','gpu','--locked','--',$Backend)
        Invoke-Cargo @('run','-p','view-wgpu','--example','offscreen','--locked','--',$Backend)
        Invoke-Cargo @('run','-p','view-wgpu','--features','scene','--example','scene','--locked','--',$Backend)
        Invoke-Cargo @('run','-p','view-wgpu','--features','scene','--example','embedded','--locked','--',$Backend)
        Invoke-Cargo @('run','-p','view-controls','--features','gallery','--example','gallery','--locked','--','--gpu',$Backend)
        if ($IsWindows) { Invoke-Cargo @('run','-p','view-controls','--features','gallery','--example','gallery','--locked','--','--run-hidden',$Backend) }
        if ($IsWindows) { Invoke-Cargo @('run','-p','view-wgpu','--example','surface','--locked','--','--run-hidden',$Backend) }
    }
} catch {
    Write-Host $_ -ForegroundColor Red
    $exitStatus = 1
} finally {
    try { Stop-Transcript | Out-Null } catch { }
    Pop-Location
    $env:CARGO_INCREMENTAL = $priorIncremental
}
exit $exitStatus

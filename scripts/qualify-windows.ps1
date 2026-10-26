[CmdletBinding()]
param(
    [switch]$Cpu,
    [switch]$Gpu,
    [switch]$NativeWebview,
    [switch]$SelectedDesktop
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This qualification profile requires Windows.' }
$repoRoot = Split-Path $PSScriptRoot -Parent
$output = Join-Path $repoRoot 'target/verification/windows-matrix'
New-Item -ItemType Directory -Force $output | Out-Null
$results = [Collections.Generic.List[object]]::new()
function Invoke-Case {
    param([string]$Name, [string]$Executable, [string[]]$Arguments, [int]$TimeoutSeconds = 300)
    $stdout = Join-Path $output "$Name.stdout.log"
    $stderr = Join-Path $output "$Name.stderr.log"
    $started = [DateTime]::UtcNow
    $status = 'failed'
    $detail = $null
    $process = $null
    try {
        # All argument lists below are repository-owned tokens without spaces.
        $process = Start-Process -FilePath $Executable -ArgumentList $Arguments -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw "Exceeded $TimeoutSeconds seconds; owned process tree terminated."
        }
        if ($process.ExitCode -ne 0) { throw "Exit code $($process.ExitCode); inspect retained logs." }
        $status = 'passed'
    } catch { $detail = $_.Exception.Message }
    finally {
        if ($process -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $results.Add([ordered]@{
            name = $Name; status = $status; detail = $detail
            startedUtc = $started.ToString('o'); endedUtc = [DateTime]::UtcNow.ToString('o')
            command = @($Executable) + $Arguments; stdout = $stdout; stderr = $stderr
        })
        $results | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $output 'cases.json')
        Write-Host "$Name`: $status"
    }
}
function Uncovered {
    param([string]$Name, [string]$Reason)
    $results.Add([ordered]@{name = $Name; status = 'uncovered'; detail = $Reason})
}
Push-Location $repoRoot
try {
    [ordered]@{
        timestampUtc = [DateTime]::UtcNow.ToString('o')
        revision = (& git rev-parse HEAD)
        workingTree = @(& git status --porcelain)
        os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
        compiler = @(& rustc -vV)
        selectedDesktop = [bool]$SelectedDesktop
        inputLanguages = @(Get-WinUserLanguageList | Select-Object LanguageTag,InputMethodTips)
        coverageMeaning = 'Exit-success is evidence only for the named fixture and its documented assertions.'
    } | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $output 'environment.json')
    if ($Cpu) {
        Invoke-Case 'cpu' 'pwsh' @('-NoProfile','-File','scripts/verify.ps1') 2400
        Invoke-Case 'cpu-workload' 'cargo' @('run','-p','view-application-proofs','--locked','--','--stress')
    }
    else { Uncovered 'cpu' 'Not requested; use -Cpu for the complete local profile matrix.' }
    Invoke-Case 'diagnostics-supervisor' 'cargo' @('test','-p','view-diagnostics','--all-features','--test','contracts','--locked','independent_supervisor_retains_failure_policy_and_recovers_on_fresh_launch','--','--exact','--nocapture')
    if ($Gpu) {
        Invoke-Case 'native-window-lifecycle' 'cargo' @('run','-p','view-platform','--example','window_lifecycle','--locked')
        Invoke-Case 'native-docking' 'cargo' @('run','-p','view-controls','--features','docking','--example','docking','--locked','--','--verify-hidden')
        Invoke-Case 'gpu-2d' 'cargo' @('run','-p','view-wgpu','--example','offscreen','--locked','--','dx12')
        Invoke-Case 'gpu-scene' 'cargo' @('run','-p','view-wgpu','--features','scene','--example','scene','--locked','--','dx12')
        Invoke-Case 'gpu-embedding' 'cargo' @('run','-p','view-wgpu','--features','scene','--example','embedded','--locked','--','dx12')
        Invoke-Case 'native-surface' 'cargo' @('run','-p','view-wgpu','--example','surface','--locked','--','--run-hidden','dx12')
        Invoke-Case 'gallery-gpu' 'cargo' @('run','-p','view-controls','--features','gallery','--example','gallery','--locked','--','--gpu','dx12')
        Invoke-Case 'gallery-native' 'cargo' @('run','-p','view-controls','--features','gallery','--example','gallery','--locked','--','--run-hidden','dx12')
        Invoke-Case 'application-gpu' 'cargo' @('run','-p','view-application-proofs','--features','automation','--locked','--','--gpu','dx12')
        Invoke-Case 'addon-gpu' 'cargo' @('run','-p','view-addons','--features','render','--bin','review-addon','--locked','--','--gpu','dx12')
    } else { Uncovered 'hardware-gpu' 'Not requested; use -Gpu on a hardware DX12 desktop.' }
    if ($NativeWebview) {
        Invoke-Case 'webview-native' 'cargo' @('run','-p','view-webview','--features','native','--example','panel','--locked','--','--native-check')
    } else { Uncovered 'webview-native' 'Not requested; requires installed WebView2 runtime.' }
    if ($SelectedDesktop) {
        Invoke-Case 'desktop-en-US' 'pwsh' @('-NoProfile','-File','scripts/verify-desktop.ps1','-SelectedDesktop','-Scenario','en-US') 600
    } else { Uncovered 'desktop-en-US' 'No explicitly selected unlocked desktop for this invocation.' }
    Uncovered 'Japanese-IME' 'Requires separately installed/selected Japanese input method and native composition scenario.'
    Uncovered 'mixed-monitor-text-scale-native-AT' 'Requires named hardware/display configurations and actual assistive-technology observations.'
    Uncovered 'sleep-resume-display-removal' 'Requires an explicitly managed disruptive hardware lifecycle session.'
    Uncovered 'Linux-macOS-GPU' 'This is a Windows-only local qualification run.'
} finally {
    $results | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $output 'cases.json')
    Pop-Location
}
if (@($results | Where-Object status -EQ 'failed').Count -gt 0) { exit 1 }
Write-Host 'Requested cases passed; uncovered entries remain open acceptance gates.'

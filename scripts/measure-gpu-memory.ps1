[CmdletBinding()]
param(
    [ValidateRange(1,10)][int]$Runs = 3,
    [ValidateSet('default','usage','manual16')][string]$Policy = 'default',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'This probe currently requires Windows/DX12.' }
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    if (-not $SkipBuild) {
        & cargo build --release -p view-controls --features gallery --example memory_probe --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release probe build failed.' }
    }
    $folder = Join-Path $repoRoot "target/verification/gpu-memory/$Policy"
    New-Item -ItemType Directory -Force $folder | Out-Null
    $summary = @()
    for ($run = 1; $run -le $Runs; $run++) {
        $log = Join-Path $folder "run-$run.log"
        $errorLog = Join-Path $folder "run-$run-error.log"
        $process = Start-Process -FilePath (Join-Path $repoRoot 'target/release/examples/memory_probe.exe') -ArgumentList @('--run-hidden', $Policy) -WindowStyle Hidden -PassThru -RedirectStandardOutput $log -RedirectStandardError $errorLog
        $samples = [System.Collections.Generic.List[object]]::new()
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        try {
            while (-not $process.HasExited) {
                if ($watch.Elapsed.TotalSeconds -gt 240) { throw 'Owned memory probe timed out.' }
                $stageLine = Get-Content -LiteralPath $log | Where-Object { $_ -like 'STAGE *' } | Select-Object -Last 1
                $stage = if ($stageLine) { $stageLine.Substring(6) } else { 'startup' }
                $process.Refresh()
                if ($process.HasExited) { break }
                $samples.Add([pscustomobject]@{
                    Run = $run; Stage = $stage; Seconds = [math]::Round($watch.Elapsed.TotalSeconds,3)
                    PrivateMiB = [math]::Round($process.PrivateMemorySize64 / 1MB,3)
                    WorkingSetMiB = [math]::Round($process.WorkingSet64 / 1MB,3)
                    PeakWorkingSetMiB = [math]::Round($process.PeakWorkingSet64 / 1MB,3)
                })
                Start-Sleep -Milliseconds 200
            }
            $process.WaitForExit()
        } finally {
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
            $samples | Export-Csv (Join-Path $folder "run-$run.csv") -NoTypeInformation
        }
        if ($process.ExitCode -ne 0) { Get-Content $errorLog; throw "Probe failed: $($process.ExitCode)" }
        foreach ($group in ($samples | Group-Object Stage | Where-Object Name -ne 'transition')) {
            $tail = @($group.Group | Select-Object -Last 5)
            $private = @($tail.PrivateMiB | Sort-Object)
            $working = @($tail.WorkingSetMiB | Sort-Object)
            $summary += [pscustomobject]@{
                Run = $run; Stage = $group.Name
                PrivateMiB = $private[[int][math]::Floor($private.Count / 2)]
                WorkingSetMiB = $working[[int][math]::Floor($working.Count / 2)]
                SampledPeakPrivateMiB = ($group.Group | Measure-Object PrivateMiB -Maximum).Maximum
                SampledPeakWorkingSetMiB = ($group.Group | Measure-Object WorkingSetMiB -Maximum).Maximum
            }
        }
        Write-Host "Completed hardware GPU memory run $run/$Runs"
    }
    $summary | Export-Csv (Join-Path $folder 'summary.csv') -NoTypeInformation
    $summary | Format-Table -AutoSize
    [ordered]@{
        os = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
        timestampUtc = [DateTime]::UtcNow.ToString('o')
        runs = $Runs; profile = 'release'; backend = 'dx12'; hidden = $true; memoryPolicy = $Policy
        binarySha256 = (Get-FileHash target/release/examples/memory_probe.exe -Algorithm SHA256).Hash
        sampleIntervalMs = 200; quietStageSeconds = 3; tailMedianSamples = 5
    } | ConvertTo-Json | Set-Content (Join-Path $folder 'environment.json')
} finally { Pop-Location }

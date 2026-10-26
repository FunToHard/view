[CmdletBinding()]
param(
    [ValidateRange(1,10)][int]$Runs = 3,
    [ValidateSet('default','usage','manual16')][string]$Policy = 'default',
    [switch]$SkipBuild,
    [string]$Baseline,
    [ValidateRange(1.0,10.0)][double]$RegressionFactor = 1.5
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
    if ($Baseline -and ([IO.Path]::GetFullPath($Baseline).TrimEnd('\','/') -eq [IO.Path]::GetFullPath($folder).TrimEnd('\','/'))) {
        throw 'Baseline must be stored separately from the current output directory.'
    }
    New-Item -ItemType Directory -Force $folder | Out-Null
    $summary = @()
    $timings = @()
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
        $runTimings = @(Get-Content -LiteralPath $log | Where-Object { $_ -like 'TIMING *' } | ForEach-Object {
            $timing = $_.Substring(7) | ConvertFrom-Json
            $timing | Add-Member -NotePropertyName run -NotePropertyValue $run -PassThru
        })
        if ($runTimings.Count -ne 12) { throw 'Probe did not report all twelve cold/warm timing distributions.' }
        $timings += $runTimings
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
    $timings | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $folder 'timings.json')
    $summary | Format-Table -AutoSize
    $environment = [ordered]@{
        os = [System.Runtime.InteropServices.RuntimeInformation]::OSDescription
        timestampUtc = [DateTime]::UtcNow.ToString('o')
        runs = $Runs; profile = 'release'; backend = 'dx12'; hidden = $true; memoryPolicy = $Policy
        binarySha256 = (Get-FileHash target/release/examples/memory_probe.exe -Algorithm SHA256).Hash
        sampleIntervalMs = 200; quietStageSeconds = 3; tailMedianSamples = 5
        workloadVersion = 1
        measurements = 'CPU elapsed through submit/present call; excludes GPU completion and visible input latency'
        gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion)
        cpu = @(Get-CimInstance Win32_Processor | Select-Object Name)
        fontFixture = 'installed system fonts; recorded host inventory'
        fontInventorySha256 = (Get-ChildItem "$env:WINDIR/Fonts" -File | Sort-Object Name | ForEach-Object { "$($_.Name):$($_.Length):$($_.LastWriteTimeUtc.Ticks)" }) -join "`n"
    }
    $fontBytes = [Text.Encoding]::UTF8.GetBytes($environment.fontInventorySha256)
    $environment.fontInventorySha256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($fontBytes))
    $environment | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $folder 'environment.json')
    if ($Baseline) {
        $baselineEnvironment = Get-Content (Join-Path $Baseline 'environment.json') -Raw | ConvertFrom-Json
        foreach ($key in @('os','profile','backend','memoryPolicy','workloadVersion','fontInventorySha256')) {
            if ($baselineEnvironment.$key -ne $environment[$key]) { throw "Baseline environment differs: $key" }
        }
        foreach ($key in @('gpu','cpu')) {
            if (($baselineEnvironment.$key | ConvertTo-Json -Compress) -ne ($environment[$key] | ConvertTo-Json -Compress)) {
                throw "Baseline hardware differs: $key"
            }
        }
        $baselineTimings = @(Get-Content (Join-Path $Baseline 'timings.json') -Raw | ConvertFrom-Json)
        foreach ($group in ($timings | Group-Object workload,stage)) {
            $sample = $group.Group[0]
            $reference = @($baselineTimings | Where-Object { $_.workload -eq $sample.workload -and $_.stage -eq $sample.stage })
            if ($reference.Count -eq 0) { throw "Baseline missing $($group.Name)" }
            foreach ($percentile in @('p50_ms','p95_ms','p99_ms')) {
                $measured = @($group.Group.$percentile | Sort-Object)
                $prior = @($reference.$percentile | Sort-Object)
                $actual = $measured[[int][math]::Floor($measured.Count / 2)]
                $limit = $prior[[int][math]::Floor($prior.Count / 2)] * $RegressionFactor
                if ($actual -gt $limit) { throw "Timing regression: $($group.Name) $percentile $actual ms exceeds $limit ms" }
            }
        }
    }
} finally { Pop-Location }

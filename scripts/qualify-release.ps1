[CmdletBinding()]
param(
    [switch]$Profiles,
    [switch]$RequirePublishable
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    $output = Join-Path $repoRoot 'target/verification/release'
    New-Item -ItemType Directory -Force $output | Out-Null
    function Read-Metadata {
        param([string[]]$Arguments)
        $raw = & cargo metadata --format-version 1 --locked --offline @Arguments
        if ($LASTEXITCODE -ne 0) { throw "cargo metadata $Arguments failed" }
        return ($raw | ConvertFrom-Json)
    }
    function Invoke-Cargo {
        param([string[]]$Arguments)
        & cargo @Arguments
        if ($LASTEXITCODE -ne 0) { throw "cargo $Arguments failed ($LASTEXITCODE)" }
    }
    $metadata = Read-Metadata @('--no-deps')
    $packages = @($metadata.packages | Where-Object id -In $metadata.workspace_members)
    $byName = @{}
    foreach ($package in $packages) { $byName[$package.name] = $package }
    $issues = [Collections.Generic.List[string]]::new()
    $blockers = [Collections.Generic.List[string]]::new()
    $version = $byName['view'].version
    $compiler = @(& rustc -vV)
    if ($LASTEXITCODE -ne 0) { throw 'rustc unavailable' }
    if ($compiler[0] -notmatch '^rustc 1\.98\.1 ') { $issues.Add('Release qualification must use the pinned Rust 1.98.1 compiler.') }
    foreach ($package in $packages) {
        if ($package.version -ne $version) { $issues.Add("Version differs from coordinated $version`: $($package.name)") }
        if ($package.edition -ne '2024' -or $package.rust_version -ne '1.98.1') { $issues.Add("Edition/compiler mismatch: $($package.name)") }
        if ($package.license -ne 'MIT OR Apache-2.0') { $issues.Add("Unexpected first-party license: $($package.name)") }
        if ($null -eq $package.publish -or @($package.publish).Count -ne 0) { $issues.Add("Publication must remain disabled: $($package.name)") }
        foreach ($dependency in $package.dependencies | Where-Object { $_.path -and $_.kind -ne 'dev' }) {
            if (-not $byName.ContainsKey($dependency.name)) { $issues.Add("Nonworkspace path dependency: $($package.name) -> $($dependency.name)") }
            elseif ($dependency.req -notin @($version, "^$version", "=$version")) { $issues.Add("Sibling path lacks coordinated version: $($package.name) -> $($dependency.name) [$($dependency.req)]") }
        }
    }
    # These contracts are stronger than checking a few forbidden package names:
    # core has no dependencies at all; the harness has only core.
    if (@($byName['view-core'].dependencies).Count -ne 0) { $issues.Add('Core must remain dependency-free.') }
    $testingDeps = @($byName['view-testing'].dependencies | Where-Object kind -NE 'dev')
    if ($testingDeps.Count -ne 1 -or $testingDeps[0].name -ne 'view-core') { $issues.Add('Harness may depend only on core.') }
    $facadeSource = [IO.File]::ReadAllText((Join-Path $repoRoot 'crates/view/src/lib.rs'))
    if ($facadeSource -notmatch '#!\[forbid\(unsafe_code\)\]') { $issues.Add('Public facade must forbid unsafe code.') }
    $consumerManifest = 'tests/compatibility/facade-consumer/Cargo.toml'
    $minimal = Read-Metadata @('--manifest-path', $consumerManifest, '--no-default-features')
    $minimalGraph = @($minimal.packages | Where-Object id -In $minimal.resolve.nodes.id)
    if (($minimalGraph.name | Sort-Object) -join ',' -ne 'view,view-core,view-facade-consumer') { $issues.Add("Minimal consumer graph leaked dependencies: $($minimalGraph.name -join ', ')") }

    # Topological package order includes optional normal/build edges; dev edges
    # need not be published first. A cycle here is a release error.
    $ordered = [Collections.Generic.List[object]]::new()
    $pending = @($packages)
    while ($pending.Count -gt 0) {
        $ready = @($pending | Where-Object {
            $package = $_
            @($package.dependencies | Where-Object { $_.kind -ne 'dev' -and $_.name -in $pending.name }).Count -eq 0
        } | Sort-Object name)
        if ($ready.Count -eq 0) { throw 'Workspace normal/build dependency cycle.' }
        foreach ($package in $ready) { $ordered.Add($package) }
        $pending = @($pending | Where-Object name -NotIn $ready.name)
    }
    $contentReports = foreach ($package in $ordered) {
        $files = @(& cargo package -p $package.name --list --allow-dirty --locked --offline)
        if ($LASTEXITCODE -ne 0) { throw "Package listing failed: $($package.name)" }
        $files | Set-Content (Join-Path $output "$($package.name)-contents.txt")
        foreach ($required in @('Cargo.toml', 'LICENSE-MIT', 'LICENSE-APACHE')) {
            if ($required -notin $files) { $issues.Add("Package $($package.name) omits $required") }
        }
        foreach ($licenseName in @('LICENSE-MIT', 'LICENSE-APACHE')) {
            $localLicense = Join-Path (Split-Path $package.manifest_path -Parent) $licenseName
            if ((Test-Path $localLicense) -and (Get-FileHash $localLicense).Hash -ne (Get-FileHash (Join-Path $repoRoot $licenseName)).Hash) { $issues.Add("Package $($package.name) has stale $licenseName") }
        }
        $unexpected = @($files | Where-Object { $_ -match '(^|/)(target|work|\.env|\.aws|\.codex)(/|$)|\.(etl|dmp|pdb)$' })
        if ($unexpected.Count -gt 0) { $issues.Add("Unexpected package contents in $($package.name): $($unexpected -join ', ')") }
        $packageRoot = Split-Path $package.manifest_path -Parent
        $fixtureManifestPath = Join-Path $packageRoot 'fixtures/manifest.json'
        if (Test-Path -LiteralPath $fixtureManifestPath) {
            $fixtureManifest = [IO.File]::ReadAllText($fixtureManifestPath) | ConvertFrom-Json
            foreach ($fixture in $fixtureManifest.fixtures) {
                $copyPath = Join-Path $packageRoot $fixture.path
                $canonicalPath = Join-Path $repoRoot $fixture.source
                if ($fixture.path -notin $files) { $issues.Add("Archive omits fixture: $($package.name)/$($fixture.path)") }
                if ((Get-FileHash -LiteralPath $copyPath -Algorithm SHA256).Hash -ne $fixture.sha256 -or
                    (Get-FileHash -LiteralPath $canonicalPath -Algorithm SHA256).Hash -ne $fixture.sha256) {
                    $issues.Add("Fixture copy differs from owned source: $($package.name)/$($fixture.path)")
                }
            }
        }
        if (@($files | Where-Object { $_.EndsWith('.ttf') }).Count -gt 0 -and 'fixtures/fonts/NotoSans-LICENSE' -notin $files) {
            $issues.Add("Bundled font license missing: $($package.name)")
        }
        foreach ($file in $files | Where-Object { $_.EndsWith('.rs') }) {
            $sourcePath = Join-Path $packageRoot $file
            if (-not (Test-Path -LiteralPath $sourcePath)) { continue }
            $source = [IO.File]::ReadAllText($sourcePath)
            foreach ($match in [regex]::Matches($source, 'include_(?:bytes|str)!\s*\(\s*"([^"]+)"')) {
                $asset = [IO.Path]::GetFullPath((Join-Path (Split-Path $sourcePath -Parent) $match.Groups[1].Value))
                $relativeAsset = [IO.Path]::GetRelativePath($packageRoot, $asset).Replace('\', '/')
                if ($relativeAsset.StartsWith('../') -or $relativeAsset -notin $files) {
                    $blockers.Add("Archive-local include missing: $($package.name)/$file -> $relativeAsset")
                }
            }
        }
        [ordered]@{ name=$package.name; version=$package.version; files=$files.Count; publishDisabled=$true }
    }
    # This command creates and verifies an actual dependency-free .crate archive;
    # package lists above do not pretend to verify registry-dependent archives.
    $packageTarget = Join-Path $repoRoot 'target/release-qualification'
    Invoke-Cargo @('package', '-p', 'view-core', '--allow-dirty', '--locked', '--offline', '--target-dir', $packageTarget)
    $coreArchive = Join-Path $packageTarget "package/view-core-$version.crate"
    $firstHash = (Get-FileHash $coreArchive -Algorithm SHA256).Hash
    Invoke-Cargo @('package', '-p', 'view-core', '--allow-dirty', '--locked', '--offline', '--target-dir', $packageTarget)
    $secondHash = (Get-FileHash $coreArchive -Algorithm SHA256).Hash
    if ($firstHash -ne $secondHash) { $issues.Add('Repeated core packaging produced different archive hashes.') }

    $profileResults = [Collections.Generic.List[string]]::new()
    if ($Profiles) {
        foreach ($package in $packages | Sort-Object name) {
            Invoke-Cargo @('check', '-p', $package.name, '--no-default-features', '--all-targets', '--locked', '--offline')
            Invoke-Cargo @('check', '-p', $package.name, '--all-targets', '--locked', '--offline')
            Invoke-Cargo @('check', '-p', $package.name, '--all-features', '--all-targets', '--locked', '--offline')
            $profileResults.Add("$($package.name): minimal/default/full all-target checks")
        }
        Invoke-Cargo @('test', '--manifest-path', $consumerManifest, '--no-default-features', '--locked', '--offline', '--target-dir', 'target/consumer')
        $profileResults.Add('consumer: minimal tests')
        foreach ($feature in $minimal.packages.Where({ $_.name -eq 'view-facade-consumer' }).features.PSObject.Properties.Name | Sort-Object) {
            Invoke-Cargo @('test', '--manifest-path', $consumerManifest, '--no-default-features', '--features', $feature, '--locked', '--offline', '--target-dir', 'target/consumer')
            $profileResults.Add("consumer: $feature tests")
        }
        Invoke-Cargo @('test', '--manifest-path', $consumerManifest, '--all-features', '--locked', '--offline', '--target-dir', 'target/consumer')
        $profileResults.Add('consumer: all features tests')
    }
    $blockers.Add('The view facade name was occupied in the 2026-10-25 registry audit; an owner naming decision and fresh availability checks are required. No names are reserved.')
    $blockers.Add('Registry-dependent archive verification and clean reviewed-revision reproducibility remain release gates.')
    $blockers.Add('Native/physical-input, GPU performance and current Linux/macOS coverage require their own evidence; CPU packaging cannot close them.')
    $report = [ordered]@{
        timestampUtc=[DateTime]::UtcNow.ToString('o'); os=[Runtime.InteropServices.RuntimeInformation]::OSDescription
        compiler=$compiler; version=$version; dirtyRevisionAllowed=$true
        minimalConsumerPackages=@($minimalGraph.name | Sort-Object); dependencyOrder=@($ordered.name)
        packages=@($contentReports); verifiedCoreArchive=[ordered]@{path=$coreArchive;sha256=$secondHash;repeatedBytesMatch=($firstHash -eq $secondHash)}
        profilesRequested=[bool]$Profiles; profiles=@($profileResults); auditFailures=@($issues); releaseBlockers=@($blockers)
    }
    $report | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $output 'qualification.json')
    if ($issues.Count -gt 0) { throw ($issues -join [Environment]::NewLine) }
    if ($RequirePublishable -and $blockers.Count -gt 0) { throw ($blockers -join [Environment]::NewLine) }
    Write-Host "Local release audit passed for $($packages.Count) packages; reports: $output"
    Write-Host 'Publication and unexecuted platform/native gates remain open.'
} finally { Pop-Location }

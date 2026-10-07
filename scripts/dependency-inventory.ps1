[CmdletBinding()]
param([string]$Target = 'x86_64-pc-windows-msvc')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $raw = & cargo metadata --format-version 1 --locked --filter-platform $Target
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $data = $raw | ConvertFrom-Json
    $directNames = @('winit','windows','wgpu','cosmic-text','glyphon','accesskit','accesskit_winit','lyon','unicode-segmentation','bytemuck')
    $direct = foreach ($package in $data.packages | Where-Object name -In $directNames | Sort-Object name) {
        $node = $data.resolve.nodes | Where-Object id -EQ $package.id
        [ordered]@{name=$package.name;version=$package.version;license=$package.license;rustVersion=$package.rust_version;repository=$package.repository;metadataResolvedFeatures=@($node.features)}
    }
    $duplicates = @($data.packages | Group-Object name | Where-Object Count -gt 1 | ForEach-Object {
        [ordered]@{name=$_.Name;versions=@($_.Group.version)}
    })
    $summary = [ordered]@{
        target=$Target
        packageCount=$data.packages.Count
        registryPackageCount=@($data.packages | Where-Object source -Like 'registry*').Count
        workspacePackageCount=$data.workspace_members.Count
        directSelections=@($direct)
        duplicateFamilies=$duplicates
    }
    New-Item -ItemType Directory -Force target/verification | Out-Null
    $summary | ConvertTo-Json -Depth 8 | Set-Content target/verification/dependencies.json
    & cargo tree --workspace --locked --target $Target -e features | Set-Content target/verification/dependency-features.txt
    if ($LASTEXITCODE -ne 0) { throw 'cargo feature tree failed' }
    Write-Host "Inventory: $($summary.registryPackageCount) registry packages for $Target (workspace qualification graph, not facade)."
} finally { Pop-Location }

$ErrorActionPreference = 'Stop'
$fixtureRoot = Join-Path (Split-Path $PSScriptRoot -Parent) 'tests/fixtures'
$manifest = Get-Content (Join-Path $fixtureRoot 'manifest.json') -Raw | ConvertFrom-Json
foreach ($fixture in $manifest.fixtures) {
    $path = Join-Path $fixtureRoot $fixture.path
    # Normalize text line endings; Git checks out LF and this permits local editors.
    $text = [IO.File]::ReadAllText($path).Replace("`r`n", "`n")
    $bytes = [Text.Encoding]::UTF8.GetBytes($text)
    $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
    if ($hash -ne $fixture.sha256LfUtf8) { throw "Fixture hash changed: $($fixture.path)" }
}
Write-Host "Verified $($manifest.fixtures.Count) owned fixtures."

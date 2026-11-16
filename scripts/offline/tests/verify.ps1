$ErrorActionPreference = "Stop"
. "$PSScriptRoot/fixture.ps1"

$dir = Join-Path ([IO.Path]::GetTempPath()) ("offline-verify-" + [guid]::NewGuid())
try {
    New-VerifyFixture -Dir $dir
    & "$PSScriptRoot/../verify.ps1" -PackDir $dir
    if ($LASTEXITCODE -ne 0) { throw "verify failed on a good pack" }

    $pmtiles = Join-Path $dir "moldova.pmtiles"
    $bytes = [IO.File]::ReadAllBytes($pmtiles)
    $bytes[10] = $bytes[10] -bxor 0xFF
    [IO.File]::WriteAllBytes($pmtiles, $bytes)

    $threw = $false
    try {
        & "$PSScriptRoot/../verify.ps1" -PackDir $dir
    } catch {
        $threw = $true
    }
    if (-not $threw) { throw "tampered pack accepted" }
    Write-Host "verify ok"
} finally {
    Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
}

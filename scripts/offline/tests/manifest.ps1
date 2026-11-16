$ErrorActionPreference = "Stop"
. "$PSScriptRoot/../manifest.ps1"

$dir = Join-Path ([IO.Path]::GetTempPath()) ("offline-manifest-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$a = Join-Path $dir "moldova.pmtiles"
$b = Join-Path $dir "moldova-search.sqlite"
$manifest = Join-Path $dir "manifest.json"
[IO.File]::WriteAllBytes($a, [Text.Encoding]::UTF8.GetBytes("pmtiles"))
[IO.File]::WriteAllBytes($b, [Text.Encoding]::UTF8.GetBytes("sqlite"))

$region = [pscustomobject]@{
    id = "moldova"
    name = "Moldova"
    bbox = @(26.6, 45.4, 30.2, 48.5)
    minzoom = 0
    maxzoom = 14
}

try {
    New-OfflineManifest -Region $region -Version "moldova-20260101" -Files @($a, $b) -Output $manifest -BaseUrl "https://example.test/packs/moldova" | Out-Null
    if (-not (Test-Path $manifest)) { throw "manifest missing" }
    $doc = Get-Content $manifest -Raw | ConvertFrom-Json
    if ($doc.id -ne "moldova") { throw "id mismatch" }
    if ($doc.name -ne "Moldova") { throw "name mismatch" }
    if ($doc.version -ne "moldova-20260101") { throw "version mismatch" }
    if ($doc.files.Count -ne 2) { throw "file count mismatch" }
    if ($doc.files[0].url -ne "https://example.test/packs/moldova/moldova.pmtiles") { throw "url mismatch" }
    if ($doc.files[0].bytes -ne 7) { throw "bytes mismatch" }
    if ($doc.files[0].sha256 -ne (Get-FileHash -Algorithm SHA256 -Path $a).Hash.ToLowerInvariant()) { throw "hash mismatch" }
    if ($doc.attribution -notmatch "OpenStreetMap") { throw "attribution missing" }
    if ($doc.license -ne "ODbL-1.0") { throw "license mismatch" }
    Remove-Item $manifest -Force

    $threw = $false
    try {
        New-OfflineManifest -Region $region -Version "x" -Files @((Join-Path $dir "missing.pmtiles")) -Output $manifest | Out-Null
    } catch {
        $threw = $true
    }
    if (-not $threw) { throw "missing file accepted" }
    if (Test-Path $manifest) { throw "failed manifest left output" }
    Write-Host "manifest ok"
} finally {
    Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
}

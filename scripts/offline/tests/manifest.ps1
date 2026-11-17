$ErrorActionPreference = "Stop"
. "$PSScriptRoot/../manifest.ps1"

$dir = Join-Path ([IO.Path]::GetTempPath()) ("offline-manifest-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$a = Join-Path $dir "moldova.pmtiles"
$b = Join-Path $dir "moldova-search.sqlite"
$empty = Join-Path $dir "empty.pmtiles"
$manifest = Join-Path $dir "manifest.json"
[IO.File]::WriteAllBytes($a, [Text.Encoding]::UTF8.GetBytes("pmtiles"))
[IO.File]::WriteAllBytes($b, [Text.Encoding]::UTF8.GetBytes("sqlite"))
[IO.File]::WriteAllBytes($empty, (New-Object byte[] 0))

$region = [pscustomobject]@{
    id = "moldova"
    name = "Moldova"
    bbox = @(26.6, 45.4, 30.2, 48.5)
    minzoom = 0
    maxzoom = 14
}

function Assert-NewManifestFails {
    param(
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][string[]]$Files,
        [Parameter(Mandatory = $true)][string]$Expected
    )

    $message = $null
    try {
        New-OfflineManifest -Region $region -Version $Version -Files $Files -Output $manifest | Out-Null
    } catch {
        $message = $_.Exception.Message
    }
    if ($null -eq $message) { throw "accepted a bad manifest; expected: $Expected" }
    if (-not $message.Contains($Expected)) {
        throw "unexpected manifest failure: $message (expected: $Expected)"
    }
    if (Test-Path $manifest) { throw "failed manifest left output" }
}

try {
    $sourceSha = "a" * 64
    New-OfflineManifest -Region $region -Version "moldova-20260101" -Files @($a, $b) -Output $manifest -BaseUrl "https://example.test/packs/moldova" `
        -SourceSha256 $sourceSha -SourceBytes 12345 -SourceUrl "https://download.geofabrik.de/europe/moldova-latest.osm.pbf" -DownloadedAt "2026-11-17T12:00:00Z" | Out-Null
    if (-not (Test-Path $manifest)) { throw "manifest missing" }
    $doc = [IO.File]::ReadAllText($manifest, [Text.Encoding]::UTF8) | ConvertFrom-Json
    if ($doc.id -ne "moldova") { throw "id mismatch" }
    if ($doc.source_sha256 -ne $sourceSha) { throw "source sha256 mismatch" }
    if ($doc.source_bytes -ne 12345) { throw "source bytes mismatch" }
    if ($doc.source_url -ne "https://download.geofabrik.de/europe/moldova-latest.osm.pbf") { throw "source url mismatch" }
    if ($doc.downloaded_at -ne "2026-11-17T12:00:00Z") { throw "downloaded_at mismatch" }
    if ($doc.name -ne "Moldova") { throw "name mismatch" }
    if ($doc.version -ne "moldova-20260101") { throw "version mismatch" }
    if ($doc.files.Count -ne 2) { throw "file count mismatch" }
    if ($doc.files[0].url -ne "https://example.test/packs/moldova/moldova.pmtiles") { throw "url mismatch" }
    if ($doc.files[1].url -ne "https://example.test/packs/moldova/moldova-search.sqlite") { throw "search url mismatch" }
    if ($doc.files[0].bytes -ne 7) { throw "bytes mismatch" }
    if ($doc.files[0].sha256 -ne (Get-FileHash -Algorithm SHA256 -Path $a).Hash.ToLowerInvariant()) { throw "hash mismatch" }
    if ($doc.attribution -cne "$([char]0x00A9) OpenStreetMap contributors") { throw "attribution mismatch" }
    $rawManifest = [IO.File]::ReadAllBytes($manifest)
    if ($rawManifest.Length -ge 3 -and $rawManifest[0] -eq 0xEF -and $rawManifest[1] -eq 0xBB -and $rawManifest[2] -eq 0xBF) { throw "manifest has a UTF-8 BOM" }
    if ($doc.license -cne "ODbL-1.0") { throw "license mismatch" }
    if ($doc.license_url -cne "https://opendatacommons.org/licenses/odbl/1-0/") { throw "license url mismatch" }
    Remove-Item $manifest -Force

    Assert-NewManifestFails -Version "moldova-20260101" -Files @((Join-Path $dir "missing.pmtiles")) -Expected "manifest file missing"
    Assert-NewManifestFails -Version "moldova-20260101" -Files @($empty) -Expected "manifest file is empty"
    Assert-NewManifestFails -Version "moldova-test" -Files @($a) -Expected "version must match"

    Write-Host "manifest ok"
} finally {
    Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
}

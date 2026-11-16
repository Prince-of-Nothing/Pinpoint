param(
    [Parameter(Mandatory = $true)][string]$PackDir,
    [switch]$AllowMissingRouting
)

$ErrorActionPreference = "Stop"

$manifestPath = Join-Path $PackDir "manifest.json"
if (-not (Test-Path $manifestPath)) { throw "manifest missing: $manifestPath" }
$manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json

if ([string]::IsNullOrWhiteSpace($manifest.id)) { throw "manifest id missing" }
if ([string]::IsNullOrWhiteSpace($manifest.version)) { throw "manifest version missing" }
if ($manifest.attribution -notmatch "OpenStreetMap") { throw "ODbL attribution missing" }
if ($manifest.license -ne "ODbL-1.0") { throw "ODbL license missing" }
if (-not $manifest.files -or $manifest.files.Count -eq 0) { throw "manifest has no files" }

Write-Host "manifest: $($manifest.id) $($manifest.version) ($($manifest.files.Count) files)"

$repo = Resolve-Path (Join-Path $PSScriptRoot "../..")
$index = $env:PINPOINT_OFFLINE_INDEX
if ($index -and -not (Test-Path $index)) {
    throw "PINPOINT_OFFLINE_INDEX points to a missing binary: $index"
}
if (-not $index) {
    $candidate = @(
        (Join-Path $repo "target/debug/pinpoint-offline-index.exe"),
        (Join-Path $repo "target/debug/pinpoint-offline-index"),
        (Join-Path $repo "target/release/pinpoint-offline-index.exe"),
        (Join-Path $repo "target/release/pinpoint-offline-index")
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($candidate) { $index = $candidate }
}
if (-not $index) {
    throw "offline index binary not found; build with cargo build --features offline-build --bin pinpoint-offline-index"
}

foreach ($file in $manifest.files) {
    if ([string]::IsNullOrWhiteSpace($file.url)) { throw "manifest file has no URL: $($file.name)" }
    $path = Join-Path $PackDir $file.name
    if (-not (Test-Path $path)) { throw "missing artifact: $($file.name)" }
    $item = Get-Item $path
    if ($item.Length -ne $file.bytes) { throw "size mismatch: $($file.name)" }
    $hash = (Get-FileHash -Algorithm SHA256 -Path $path).Hash.ToLowerInvariant()
    if ($hash -ne $file.sha256) { throw "SHA-256 mismatch: $($file.name)" }
}

$pmtiles = $manifest.files | Where-Object { $_.name -like "*.pmtiles" } | Select-Object -First 1
if (-not $pmtiles) { throw "manifest has no PMTiles basemap" }
$pmPath = Join-Path $PackDir $pmtiles.name
$bytes = [IO.File]::ReadAllBytes($pmPath)
if ($bytes.Length -lt 7) { throw "PMTiles file is too short" }
$magic = [Text.Encoding]::ASCII.GetString($bytes[0..6])
if ($magic -ne "PMTiles") { throw "not a PMTiles archive" }
Write-Host "pmtiles: ok"

$search = $manifest.files | Where-Object { $_.name -like "*-search.sqlite" } | Select-Object -First 1
if (-not $search) { throw "manifest has no search index" }
$searchPath = Join-Path $PackDir $search.name
$verifyOutput = & $index --verify $searchPath 2>&1
if ($LASTEXITCODE -ne 0) { throw "search index verify failed: $verifyOutput" }
Write-Host "search: $verifyOutput"

$valhalla = $manifest.files | Where-Object { $_.name -like "*valhalla.tar" } | Select-Object -First 1
if (-not $valhalla) {
    if (-not $AllowMissingRouting) { throw "manifest has no Valhalla routing tiles" }
    Write-Host "routing: skipped"
} else {
    $valhallaPath = Join-Path $PackDir $valhalla.name
    $entries = & tar -tf $valhallaPath 2>&1
    if ($LASTEXITCODE -ne 0) { throw "routing tar unreadable: $entries" }
    if (-not ($entries | Where-Object { $_ -match '\.gph$' })) { throw "routing tar has no .gph tiles" }
    Write-Host "routing: ok"
}

$attribution = Join-Path $PackDir "ATTRIBUTION.md"
if (-not (Test-Path $attribution)) { throw "ATTRIBUTION.md missing" }
if ((Get-Content $attribution -Raw) -notmatch "OpenStreetMap") { throw "ATTRIBUTION.md does not credit OSM" }
Write-Host "attribution: ok"

Write-Host "verified $($manifest.files.Count) files for $($manifest.id)"

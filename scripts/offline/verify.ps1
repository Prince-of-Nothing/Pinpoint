param(
    [Parameter(Mandatory = $true)][string]$PackDir,
    [switch]$AllowMissingRouting,
    [switch]$AllowEmptyUrls
)

$ErrorActionPreference = "Stop"

$manifestPath = Join-Path $PackDir "manifest.json"
if (-not (Test-Path $manifestPath)) { throw "manifest missing: $manifestPath" }
$manifest = [IO.File]::ReadAllText($manifestPath, [Text.Encoding]::UTF8) | ConvertFrom-Json

$licenseUrl = "https://opendatacommons.org/licenses/odbl/1-0/"
$odblAttribution = "$([char]0x00A9) OpenStreetMap contributors"

if ([string]::IsNullOrWhiteSpace($manifest.id)) { throw "manifest id missing" }
if ([string]::IsNullOrWhiteSpace($manifest.version)) { throw "manifest version missing" }
if ($manifest.attribution -cne $odblAttribution) { throw "ODbL attribution must be '$odblAttribution'" }
if ($manifest.license -cne "ODbL-1.0") { throw "ODbL license must be ODbL-1.0" }
if ($manifest.license_url -cne $licenseUrl) { throw "ODbL license URL must be $licenseUrl" }
if (-not $manifest.files -or @($manifest.files).Count -eq 0) { throw "manifest has no files" }

Write-Host "manifest: $($manifest.id) $($manifest.version) ($(@($manifest.files).Count) files)"

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
    $name = [string]$file.name
    if ([string]::IsNullOrWhiteSpace($name) -or
        $name -eq "." -or
        $name -eq ".." -or
        $name -notmatch '^[A-Za-z0-9._-]+$' -or
        [IO.Path]::GetFileName($name) -ne $name) {
        throw "unsafe artifact name: $name"
    }
    if (-not $AllowEmptyUrls -and [string]::IsNullOrWhiteSpace([string]$file.url)) {
        throw "empty artifact url: $name"
    }
    $declaredHash = [string]$file.sha256
    if ($declaredHash -notmatch '^[A-Fa-f0-9]{64}$') { throw "SHA-256 mismatch: $name" }
    $path = Join-Path $PackDir $name
    if (-not (Test-Path $path)) { throw "missing artifact: $name" }
    $item = Get-Item $path
    if ($item.Length -ne $file.bytes) { throw "size mismatch: $name" }
    $hash = (Get-FileHash -Algorithm SHA256 -Path $path).Hash.ToLowerInvariant()
    if ($hash -ne $declaredHash.ToLowerInvariant()) { throw "SHA-256 mismatch: $name" }
}

$pmtiles = $manifest.files | Where-Object { $_.name -like "*.pmtiles" } | Select-Object -First 1
if (-not $pmtiles) { throw "manifest has no PMTiles basemap" }
$pmPath = Join-Path $PackDir $pmtiles.name
$header = New-Object byte[] 8
$stream = [IO.File]::OpenRead($pmPath)
try {
    $read = 0
    while ($read -lt $header.Length) {
        $count = $stream.Read($header, $read, $header.Length - $read)
        if ($count -le 0) { break }
        $read += $count
    }
} finally {
    $stream.Dispose()
}
if ($read -lt $header.Length) { throw "PMTiles file is too short" }
$magic = [Text.Encoding]::ASCII.GetString($header, 0, 7)
if ($magic -ne "PMTiles") { throw "not a PMTiles archive" }
if ($header[7] -ne 3) { throw "unsupported PMTiles version $($header[7]) (expected 3)" }
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
$text = [IO.File]::ReadAllText($attribution, [Text.Encoding]::UTF8)
if (-not $text.Contains("OpenStreetMap")) { throw "ATTRIBUTION.md does not credit OpenStreetMap" }
if (-not $text.Contains("ODbL")) { throw "ATTRIBUTION.md does not name ODbL" }
if (-not $text.Contains($licenseUrl)) { throw "ATTRIBUTION.md does not link the ODbL license" }
Write-Host "attribution: ok"

Write-Host "verified $(@($manifest.files).Count) files for $($manifest.id)"

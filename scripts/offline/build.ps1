param(
    [Parameter(Mandatory = $true)][string]$Region,
    [string]$OutputRoot = "dist/offline",
    [switch]$DryRun,
    [switch]$SkipDownload,
    [switch]$SkipBasemap,
    [switch]$SkipSearch,
    [switch]$SkipRouting
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "../..")
if (-not [System.IO.Path]::IsPathRooted($OutputRoot)) {
    $OutputRoot = Join-Path $repoRoot $OutputRoot
}

$configPath = Join-Path $PSScriptRoot "regions.json"
if (-not (Test-Path $configPath)) { throw "region config not found: $configPath" }
$regions = Get-Content $configPath -Raw | ConvertFrom-Json
$cfg = $regions.$Region
if ($null -eq $cfg) { throw "unknown region: $Region" }

$packDir = Join-Path $OutputRoot $Region
$packDir = [System.IO.Path]::GetFullPath($packDir)
$osm = Join-Path $packDir "$Region.osm.pbf"
$pmtiles = Join-Path $packDir "$Region.pmtiles"
$search = Join-Path $packDir "$Region-search.sqlite"
$valhalla = Join-Path $packDir "$Region-valhalla.tar"
$manifest = Join-Path $packDir "manifest.json"
$attribution = Join-Path $packDir "ATTRIBUTION.md"

function Write-PlannedCommand([string]$label, [string]$command) {
    Write-Output "  $label"
    Write-Output "    $command"
}

if ($DryRun) {
    Write-Output "DRY RUN - no files will be written"
    Write-Output "region: $($cfg.name)"
    Write-Output "bbox: $($cfg.bbox -join ',')"
    Write-Output "pack dir: $packDir"
    Write-Output "artifacts: $pmtiles $search $valhalla $manifest $attribution"
    if (-not $SkipDownload) {
        Write-PlannedCommand "download" "curl -4 -L $($cfg.geofabrik) -> $osm"
    }
    if (-not $SkipBasemap) {
        Write-PlannedCommand "basemap" "docker run --rm -v `"$packDir`:/data`" openmaptiles/planetiler-openmaptiles:latest --force --download --osm-path=/data/$Region.osm.pbf --output=/data/$Region.pmtiles --maxzoom=$($cfg.maxzoom)"
    }
    if (-not $SkipSearch) {
        Write-PlannedCommand "search" "pinpoint-offline-index --osm $osm --out $search"
    }
    if (-not $SkipRouting) {
        Write-PlannedCommand "routing-config" "docker run --rm --entrypoint valhalla_build_config -v `"$packDir`:/data`" ghcr.io/valhalla/valhalla:latest --mjolnir-tile-dir /data/valhalla_tiles --mjolnir-tile-extract /data/$Region-valhalla.tar --mjolnir-timezone /data/valhalla_tiles/timezones.sqlite --mjolnir-admin /data/valhalla_tiles/admins.sqlite > $packDir/$Region-valhalla.json"
        Write-PlannedCommand "routing" "docker run --rm --entrypoint valhalla_build_tiles -v `"$packDir`:/data`" ghcr.io/valhalla/valhalla:latest -c /data/$Region-valhalla.json /data/$Region.osm.pbf"
    }
    return
}

if (-not (Test-Path $packDir)) {
    New-Item -ItemType Directory -Force -Path $packDir | Out-Null
}
if (-not $SkipDownload) {
    . "$PSScriptRoot/download.ps1"
    Get-OsmExtract -Region $cfg -Destination $osm | Out-Null
}
if (-not $SkipBasemap) {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { throw "docker not found; install Docker Desktop" }
    & docker run --rm -v "${packDir}:/data" openmaptiles/planetiler-openmaptiles:latest --force --download --osm-path="/data/$Region.osm.pbf" --output="/data/$Region.pmtiles" --maxzoom=$($cfg.maxzoom)
    if ($LASTEXITCODE -ne 0) { throw "planetiler basemap build failed" }
}
if (-not $SkipSearch) {
    $indexBin = @(
        (Join-Path $repoRoot "target/debug/pinpoint-offline-index.exe"),
        (Join-Path $repoRoot "target/debug/pinpoint-offline-index"),
        (Join-Path $repoRoot "target/release/pinpoint-offline-index.exe"),
        (Join-Path $repoRoot "target/release/pinpoint-offline-index")
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $indexBin) {
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo not found; install Rust" }
        & cargo build --manifest-path (Join-Path $repoRoot "Cargo.toml") --features offline-build --bin pinpoint-offline-index
        if ($LASTEXITCODE -ne 0) { throw "offline index build failed; ensure the C++ linker is on PATH" }
        $indexBin = @(
            (Join-Path $repoRoot "target/debug/pinpoint-offline-index.exe"),
            (Join-Path $repoRoot "target/debug/pinpoint-offline-index")
        ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    }
    if (-not $indexBin) { throw "offline index binary not found" }
    & $indexBin --osm $osm --out $search
    if ($LASTEXITCODE -ne 0) { throw "search index build failed" }
}
if (-not $SkipRouting) {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) { throw "docker not found; install Docker Desktop" }
    $config = Join-Path $packDir "$Region-valhalla.json"
    & docker run --rm --entrypoint valhalla_build_config -v "${packDir}:/data" ghcr.io/valhalla/valhalla:latest --mjolnir-tile-dir /data/valhalla_tiles --mjolnir-tile-extract "/data/$Region-valhalla.tar" --mjolnir-timezone /data/valhalla_tiles/timezones.sqlite --mjolnir-admin /data/valhalla_tiles/admins.sqlite | Set-Content -Path $config -Encoding ASCII
    if ($LASTEXITCODE -ne 0) { throw "valhalla_build_config failed" }
    & docker run --rm --entrypoint valhalla_build_tiles -v "${packDir}:/data" ghcr.io/valhalla/valhalla:latest -c "/data/$Region-valhalla.json" "/data/$Region.osm.pbf"
    if ($LASTEXITCODE -ne 0) { throw "valhalla_build_tiles failed" }
    & tar -C $packDir -cf $valhalla "valhalla_tiles"
    if ($LASTEXITCODE -ne 0) { throw "valhalla tar failed" }
}

$built = @()
if (Test-Path $pmtiles) { $built += $pmtiles }
if (Test-Path $search) { $built += $search }
if (Test-Path $valhalla) { $built += $valhalla }
if ($built.Count -eq 0) { throw "no artifacts were built for $Region" }
Copy-Item (Join-Path $PSScriptRoot "ATTRIBUTION.md") (Join-Path $packDir "ATTRIBUTION.md") -Force
. "$PSScriptRoot/manifest.ps1"
New-OfflineManifest -Region $cfg -Version "$Region-$(Get-Date -Format yyyyMMdd)" -Files $built -Output $manifest | Out-Null
& "$PSScriptRoot/verify.ps1" -PackDir $packDir -AllowMissingRouting:$SkipRouting
if ($LASTEXITCODE -ne 0) { throw "pack verification failed" }

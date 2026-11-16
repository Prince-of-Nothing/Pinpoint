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

$configPath = Join-Path $PSScriptRoot "regions.json"
if (-not (Test-Path $configPath)) { throw "region config not found: $configPath" }
$regions = Get-Content $configPath -Raw | ConvertFrom-Json
$cfg = $regions.$Region
if ($null -eq $cfg) { throw "unknown region: $Region" }

$packDir = Join-Path $OutputRoot $Region
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
        Write-PlannedCommand "download" "Invoke-WebRequest $($cfg.geofabrik) -> $osm"
    }
    if (-not $SkipBasemap) {
        Write-PlannedCommand "basemap" "docker run --rm -v `"$packDir`:/data`" openmaptiles/planetiler-openmaptiles:latest --force --osm-path=/data/$Region.osm.pbf --output=/data/$Region.pmtiles --maxzoom=$($cfg.maxzoom)"
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
if (-not $SkipRouting) {
    $config = Join-Path $packDir "$Region-valhalla.json"
    & docker run --rm --entrypoint valhalla_build_config -v "${packDir}:/data" ghcr.io/valhalla/valhalla:latest --mjolnir-tile-dir /data/valhalla_tiles --mjolnir-tile-extract "/data/$Region-valhalla.tar" --mjolnir-timezone /data/valhalla_tiles/timezones.sqlite --mjolnir-admin /data/valhalla_tiles/admins.sqlite | Set-Content -Path $config -Encoding UTF8
    if ($LASTEXITCODE -ne 0) { throw "valhalla_build_config failed" }
    & docker run --rm --entrypoint valhalla_build_tiles -v "${packDir}:/data" ghcr.io/valhalla/valhalla:latest -c "/data/$Region-valhalla.json" "/data/$Region.osm.pbf"
    if ($LASTEXITCODE -ne 0) { throw "valhalla_build_tiles failed" }
    & tar -C $packDir -cf $valhalla "valhalla_tiles"
    if ($LASTEXITCODE -ne 0) { throw "valhalla tar failed" }
}

throw "pipeline steps are added in Task 7"

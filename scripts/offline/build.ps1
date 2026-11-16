param(
    [Parameter(Mandatory = $true)][string]$Region,
    [string]$OutputRoot = "dist/offline",
    [string]$BaseUrl,
    [string]$OsmPath,
    [Alias("PmtilesPath")][string]$BasemapPath,
    [switch]$DryRun,
    [switch]$SkipDownload,
    [switch]$SkipBasemap,
    [switch]$SkipSearch,
    [switch]$SkipRouting
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
if (-not [System.IO.Path]::IsPathRooted($OutputRoot)) {
    $OutputRoot = Join-Path $repoRoot $OutputRoot
}
$OutputRoot = [System.IO.Path]::GetFullPath($OutputRoot)

$configPath = Join-Path $PSScriptRoot "regions.json"
if (-not (Test-Path $configPath)) { throw "region config not found: $configPath" }
$regions = Get-Content $configPath -Raw | ConvertFrom-Json
$cfg = $regions.$Region
if ($null -eq $cfg) { throw "unknown region: $Region" }

if ([string]::IsNullOrWhiteSpace($BaseUrl)) { $BaseUrl = $cfg.base_url }
if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
    throw "region '$Region' has no base_url in regions.json; pass -BaseUrl <https://host/prefix>"
}
if ($BaseUrl -notmatch '^https?://') { throw "base URL must be an http(s) URL: $BaseUrl" }
$BaseUrl = $BaseUrl.TrimEnd('/')

$buildId = [guid]::NewGuid().ToString("N")
$packDir = [System.IO.Path]::GetFullPath((Join-Path $OutputRoot $Region))
$stagingRoot = Join-Path $OutputRoot ".staging"
$workDir = Join-Path $stagingRoot "$Region-$buildId-work"
$publishDir = Join-Path $stagingRoot "$Region-$buildId-publish"
$backupDir = Join-Path $stagingRoot "$Region-$buildId-previous"

$osm = Join-Path $workDir "$Region.osm.pbf"
$pmtiles = Join-Path $workDir "$Region.pmtiles"
$search = Join-Path $workDir "$Region-search.sqlite"
$valhalla = Join-Path $workDir "$Region-valhalla.tar"
$valhallaConfig = Join-Path $workDir "$Region-valhalla.json"
$manifest = Join-Path $workDir "manifest.json"
$attribution = Join-Path $workDir "ATTRIBUTION.md"

function Write-PlannedCommand([string]$label, [string]$command) {
    Write-Output "  $label"
    Write-Output "    $command"
}

function Test-Application([string]$Name) {
    return [bool](Get-Command $Name -CommandType Application -ErrorAction SilentlyContinue)
}

function Assert-Application([string]$Name, [string]$Message) {
    if (-not (Test-Application $Name)) { throw $Message }
}

function Find-OfflineIndexBinary([string]$Root) {
    $candidates = @(
        (Join-Path $Root "target/debug/pinpoint-offline-index.exe"),
        (Join-Path $Root "target/debug/pinpoint-offline-index"),
        (Join-Path $Root "target/release/pinpoint-offline-index.exe"),
        (Join-Path $Root "target/release/pinpoint-offline-index")
    )
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

. "$PSScriptRoot/download.ps1"

function Resolve-OfflineIndexBinary([string]$Root) {
    $cargo = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cargo) {
        Write-Host "offline index: cargo build --locked --features offline-build --bin pinpoint-offline-index"
        $code = Invoke-NativeCommand { & $cargo.Source build --manifest-path (Join-Path $Root "Cargo.toml") --locked --features offline-build --bin pinpoint-offline-index }
        if ($code -ne 0) {
            throw "offline index build failed; ensure a C++ linker (cl.exe/link.exe) is on PATH"
        }
        $binary = Find-OfflineIndexBinary -Root $Root
        if ($binary) { return $binary }
        throw "cargo build succeeded but pinpoint-offline-index was not found in target/debug or target/release"
    }
    $binary = Find-OfflineIndexBinary -Root $Root
    if ($binary) {
        Write-Host "offline index: using prebuilt $binary (cargo not found)"
        return $binary
    }
    throw "cargo not found and no prebuilt pinpoint-offline-index in target/debug or target/release"
}

if ($DryRun) {
    Write-Output "DRY RUN - no files will be written"
    Write-Output "region: $($cfg.name)"
    Write-Output "bbox: $($cfg.bbox -join ',')"
    Write-Output "base url: $BaseUrl"
    Write-Output "staging work dir: $workDir"
    Write-Output "publish: $packDir"
    Write-Output "publish files: $Region.pmtiles $Region-search.sqlite $Region-valhalla.tar manifest.json ATTRIBUTION.md"
    if (-not $SkipDownload) {
        Write-PlannedCommand "download" "curl -4 -L --fail --connect-timeout 30 --max-time 3600 $($cfg.geofabrik) -> $osm (sha256 $($cfg.source.sha256))"
    } elseif ($OsmPath) {
        Write-PlannedCommand "source" "copy $OsmPath -> $osm"
    }
    if (-not $SkipBasemap) {
        Write-PlannedCommand "basemap" "docker run --rm -v `"$workDir`:/data`" $($cfg.images.planetiler) --force --download --osm-path=/data/$Region.osm.pbf --output=/data/$Region.pmtiles --maxzoom=$($cfg.maxzoom)"
    } elseif ($BasemapPath) {
        Write-PlannedCommand "basemap" "copy prebuilt $BasemapPath -> $pmtiles"
    }
    if (-not $SkipSearch) {
        Write-PlannedCommand "search" "pinpoint-offline-index --osm $osm --out $search"
    }
    if (-not $SkipRouting) {
        Write-PlannedCommand "routing-config" "docker run --rm --entrypoint valhalla_build_config -v `"$workDir`:/data`" $($cfg.images.valhalla) --mjolnir-tile-dir /data/valhalla_tiles --mjolnir-tile-extract /data/$Region-valhalla.tar --mjolnir-timezone /data/valhalla_tiles/timezones.sqlite --mjolnir-admin /data/valhalla_tiles/admins.sqlite --mjolnir-concurrency 2 > $valhallaConfig"
        Write-PlannedCommand "routing" "docker run --rm --entrypoint valhalla_build_tiles -v `"$workDir`:/data`" $($cfg.images.valhalla) -c /data/$Region-valhalla.json /data/$Region.osm.pbf"
        Write-PlannedCommand "routing-tar" "tar -C $workDir -cf $valhalla valhalla_tiles"
    }
    Write-PlannedCommand "manifest" "New-OfflineManifest -BaseUrl $BaseUrl -> $manifest"
    Write-PlannedCommand "verify" "verify.ps1 -PackDir $workDir -AllowMissingRouting:$SkipRouting"
    Write-PlannedCommand "publish" "replace $packDir with the five-file pack only after verify succeeds"
    return
}

if ($SkipDownload -and -not $OsmPath) {
    throw "-SkipDownload requires -OsmPath <file.osm.pbf>; the published pack no longer stores the source PBF"
}
if ($OsmPath -and -not (Test-Path $OsmPath)) { throw "OSM extract not found: $OsmPath" }
if ($BasemapPath -and -not $SkipBasemap) {
    throw "-BasemapPath requires -SkipBasemap (it supplies the prebuilt PMTiles instead of running Planetiler)"
}
if ($SkipBasemap -and $BasemapPath -and -not (Test-Path $BasemapPath)) {
    throw "basemap not found: $BasemapPath"
}

# Preflight before the long download: fail fast on missing tools, respecting -Skip*.
if (-not $SkipDownload -and -not $OsmPath) {
    if (-not (Get-CurlExecutable)) { throw "curl not found; install curl (curl.exe on Windows)" }
}
if (-not $SkipBasemap) {
    Assert-Application "docker" "docker not found; install Docker Engine or Docker Desktop"
    if ([string]::IsNullOrWhiteSpace($cfg.images.planetiler)) {
        throw "region '$Region' has no pinned Planetiler image (images.planetiler)"
    }
    if ($cfg.images.planetiler -notmatch '@sha256:[0-9a-fA-F]{64}$') {
        throw "Planetiler image is not pinned by digest: $($cfg.images.planetiler)"
    }
}
if (-not $SkipRouting) {
    Assert-Application "docker" "docker not found; install Docker Engine or Docker Desktop"
    Assert-Application "tar" "tar not found; install tar"
    if ([string]::IsNullOrWhiteSpace($cfg.images.valhalla)) {
        throw "region '$Region' has no pinned Valhalla image (images.valhalla)"
    }
    if ($cfg.images.valhalla -notmatch '@sha256:[0-9a-fA-F]{64}$') {
        throw "Valhalla image is not pinned by digest: $($cfg.images.valhalla)"
    }
}
if (-not $SkipSearch) {
    if (-not (Test-Application "cargo") -and -not (Find-OfflineIndexBinary -Root $repoRoot)) {
        throw "cargo not found and no prebuilt pinpoint-offline-index; install Rust or run cargo build --features offline-build --bin pinpoint-offline-index"
    }
}

$previousIndex = $env:PINPOINT_OFFLINE_INDEX
$indexBin = $null
$oldPackMoved = $false
$newPackInstalled = $false

try {
    New-Item -ItemType Directory -Force -Path $workDir | Out-Null

    if ($OsmPath) {
        Write-Output "using local OSM extract: $OsmPath"
        Get-OsmExtract -Region $cfg -Destination $osm -SourcePath $OsmPath | Out-Null
    } else {
        if ([string]::IsNullOrWhiteSpace($cfg.source.sha256)) {
            throw "region '$Region' has no source sha256; add source.sha256 to regions.json"
        }
        Write-Output "downloading $($cfg.geofabrik)"
        Get-OsmExtract -Region $cfg -Destination $osm -ExpectedSha256 $cfg.source.sha256 | Out-Null
    }

    if (-not $SkipBasemap) {
        Write-Output "basemap: $($cfg.images.planetiler)"
        $code = Invoke-NativeCommand { & docker run --rm -v "${workDir}:/data" $cfg.images.planetiler --force --download --osm-path="/data/$Region.osm.pbf" --output="/data/$Region.pmtiles" --maxzoom=$($cfg.maxzoom) }
        if ($code -ne 0) { throw "planetiler basemap build failed" }
    } elseif ($BasemapPath) {
        Write-Output "basemap: using prebuilt $BasemapPath"
        Copy-Item -Path $BasemapPath -Destination $pmtiles -Force
        if ((Get-Item $pmtiles).Length -eq 0) { throw "prebuilt basemap is empty: $BasemapPath" }
    }

    if (-not $SkipSearch) {
        $indexBin = Resolve-OfflineIndexBinary -Root $repoRoot
        $code = Invoke-NativeCommand { & $indexBin --osm $osm --out $search }
        if ($code -ne 0) { throw "search index build failed" }
        if (-not (Test-Path $search) -or (Get-Item $search).Length -eq 0) {
            throw "search index was not written: $search"
        }
    }

    if (-not $SkipRouting) {
        Write-Output "routing: $($cfg.images.valhalla)"
        $code = Invoke-NativeCommand { & docker run --rm --entrypoint valhalla_build_config -v "${workDir}:/data" $cfg.images.valhalla --mjolnir-tile-dir /data/valhalla_tiles --mjolnir-tile-extract "/data/$Region-valhalla.tar" --mjolnir-timezone /data/valhalla_tiles/timezones.sqlite --mjolnir-admin /data/valhalla_tiles/admins.sqlite --mjolnir-concurrency 2 | Set-Content -Path $valhallaConfig -Encoding ASCII }
        if ($code -ne 0) { throw "valhalla_build_config failed" }
        if (-not (Test-Path $valhallaConfig) -or (Get-Item $valhallaConfig).Length -eq 0) {
            throw "valhalla_build_config wrote no config"
        }
        $code = Invoke-NativeCommand { & docker run --rm --entrypoint valhalla_build_tiles -v "${workDir}:/data" $cfg.images.valhalla -c "/data/$Region-valhalla.json" "/data/$Region.osm.pbf" }
        if ($code -ne 0) { throw "valhalla_build_tiles failed" }
        $code = Invoke-NativeCommand { & tar -C $workDir -cf $valhalla "valhalla_tiles" }
        if ($code -ne 0) { throw "valhalla tar failed" }
    }

    $built = @()
    foreach ($candidate in @($pmtiles, $search, $valhalla)) {
        if (Test-Path $candidate) { $built += $candidate }
    }
    if ($built.Count -eq 0) { throw "no artifacts were built for $Region" }

    Copy-Item (Join-Path $PSScriptRoot "ATTRIBUTION.md") $attribution -Force
    . "$PSScriptRoot/manifest.ps1"
    New-OfflineManifest -Region $cfg -Version "$Region-$(Get-Date -Format yyyyMMdd)" -Files $built -Output $manifest -BaseUrl $BaseUrl | Out-Null

    if ($indexBin) {
        $env:PINPOINT_OFFLINE_INDEX = $indexBin
    }
    & "$PSScriptRoot/verify.ps1" -PackDir $workDir -AllowMissingRouting:$SkipRouting
    if ($LASTEXITCODE -ne 0) { throw "pack verification failed" }

    # Verification passed: stage exactly the publishable files, then swap the
    # published directory. Build inputs (PBF, generated config, raw tile dir)
    # never enter the published tree.
    New-Item -ItemType Directory -Force -Path $publishDir | Out-Null
    $publishPlan = @(
        @{ Source = $pmtiles; Name = "$Region.pmtiles" },
        @{ Source = $search; Name = "$Region-search.sqlite" },
        @{ Source = $valhalla; Name = "$Region-valhalla.tar" },
        @{ Source = $manifest; Name = "manifest.json" },
        @{ Source = $attribution; Name = "ATTRIBUTION.md" }
    )
    $published = @()
    foreach ($entry in $publishPlan) {
        if (Test-Path $entry.Source) {
            $destination = Join-Path $publishDir $entry.Name
            Move-Item -Path $entry.Source -Destination $destination
            $published += $destination
        }
    }
    if ($published.Count -ne ($built.Count + 2)) {
        throw "publish staging is incomplete: $($published.Count) files for $($built.Count) artifacts"
    }

    if (Test-Path $packDir) {
        Move-Item -Path $packDir -Destination $backupDir
        $oldPackMoved = $true
    }
    try {
        Move-Item -Path $publishDir -Destination $packDir
        $newPackInstalled = $true
    } catch {
        if ($oldPackMoved -and -not (Test-Path $packDir) -and (Test-Path $backupDir)) {
            Move-Item -Path $backupDir -Destination $packDir
            $oldPackMoved = $false
        }
        throw
    }
    if ($oldPackMoved) {
        Remove-Item $backupDir -Recurse -Force
        $oldPackMoved = $false
    }
    Write-Output "published $packDir ($($published.Count) files)"
} finally {
    Remove-Item $workDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $publishDir -Recurse -Force -ErrorAction SilentlyContinue
    if ($oldPackMoved -and -not $newPackInstalled) {
        if (-not (Test-Path $packDir) -and (Test-Path $backupDir)) {
            Move-Item -Path $backupDir -Destination $packDir -ErrorAction SilentlyContinue
        }
    }
    if ($newPackInstalled -or -not $oldPackMoved) {
        Remove-Item $backupDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $stagingRoot) {
        $left = @(Get-ChildItem $stagingRoot -Force)
        if ($left.Count -eq 0) { Remove-Item $stagingRoot -Force -ErrorAction SilentlyContinue }
    }
    if ($null -eq $previousIndex) {
        Remove-Item Env:PINPOINT_OFFLINE_INDEX -ErrorAction SilentlyContinue
    } else {
        $env:PINPOINT_OFFLINE_INDEX = $previousIndex
    }
}

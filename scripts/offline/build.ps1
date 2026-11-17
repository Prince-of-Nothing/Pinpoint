param(
    [Parameter(Mandatory = $true)][string]$Region,
    [string]$OutputRoot = "dist/offline",
    [string]$BaseUrl,
    [string]$OsmPath,
    [Alias("PmtilesPath")][string]$BasemapPath,
    [string]$Maxzoom,
    [switch]$DryRun,
    [switch]$SkipDownload,
    [switch]$SkipBasemap,
    [switch]$SkipSearch,
    [switch]$SkipRouting,
    [switch]$SkipGraph,
    [switch]$AllowUnpinnedSource
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

$basemapMaxzoom = [int]$cfg.maxzoom
if (-not [string]::IsNullOrWhiteSpace($Maxzoom)) {
    if ($Maxzoom -notmatch '^\d{1,2}$') {
        throw "-Maxzoom must be an integer from 0 to 14: $Maxzoom"
    }
    $basemapMaxzoom = [int]$Maxzoom
}
if ($basemapMaxzoom -gt 14) {
    throw "maxzoom $basemapMaxzoom exceeds the cap of 14; lower -Maxzoom so each release asset stays under the 2 GiB limit"
}
$cfg.maxzoom = $basemapMaxzoom

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
$graph = Join-Path $workDir "$Region.graph.bin"
$graphGz = Join-Path $workDir "$Region.graph.bin.gz"
$manifest = Join-Path $workDir "manifest.json"
$attribution = Join-Path $workDir "ATTRIBUTION.md"
$nativeRouterDir = Join-Path $repoRoot "tools/native-router"

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

function Find-NativeRouterBinary {
    $candidates = @(
        (Join-Path $nativeRouterDir "target/release/native-router.exe"),
        (Join-Path $nativeRouterDir "target/release/native-router")
    )
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) { return $candidate }
    }
    return $null
}

function Resolve-NativeRouterBinary {
    $cargo = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cargo) {
        Write-Host "native-router: cargo build --release --locked --manifest-path tools/native-router/Cargo.toml"
        $code = Invoke-NativeCommand { & $cargo.Source build --release --locked --manifest-path (Join-Path $nativeRouterDir "Cargo.toml") }
        if ($code -ne 0) { throw "native-router release build failed" }
        $binary = Find-NativeRouterBinary
        if ($binary) { return $binary }
        throw "cargo build succeeded but native-router was not found in tools/native-router/target/release"
    }
    $binary = Find-NativeRouterBinary
    if ($binary) {
        Write-Host "native-router: using prebuilt $binary (cargo not found)"
        return $binary
    }
    throw "cargo not found and no prebuilt native-router in tools/native-router/target/release"
}

function Compress-GzipFile([string]$Source, [string]$Destination) {
    # .NET GZipStream writes a fixed zero timestamp, but deflate bytes can still
    # differ between .NET Framework and .NET; the manifest hash pins one build.
    $inputStream = [IO.File]::OpenRead($Source)
    try {
        $outputStream = [IO.File]::Create($Destination)
        try {
            $gzip = New-Object System.IO.Compression.GZipStream($outputStream, [System.IO.Compression.CompressionMode]::Compress)
            try { $inputStream.CopyTo($gzip) } finally { $gzip.Dispose() }
        } finally {
            $outputStream.Dispose()
        }
    } finally {
        $inputStream.Dispose()
    }
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
    Write-Output "maxzoom: $basemapMaxzoom"
    Write-Output "allow unpinned source: $AllowUnpinnedSource"
    Write-Output "base url: $BaseUrl"
    Write-Output "staging work dir: $workDir"
    Write-Output "publish: $packDir"
    $publishNames = @("$Region.pmtiles", "$Region-search.sqlite", "$Region-valhalla.tar")
    if (-not $SkipGraph) {
        $publishNames += @("$Region.graph.bin", "$Region.graph.bin.gz")
    }
    $publishNames += @("manifest.json", "ATTRIBUTION.md")
    Write-Output "publish files: $($publishNames -join ' ')"
    if (-not $SkipDownload) {
        $pin = if ([string]::IsNullOrWhiteSpace($cfg.source.sha256)) { "unpinned" } else { "sha256: $($cfg.source.sha256)" }
        Write-PlannedCommand "download" "curl -4 -L --fail --connect-timeout 30 --max-time 3600 $($cfg.geofabrik) -> $osm ($pin)"
    } elseif ($OsmPath) {
        Write-PlannedCommand "source" "copy $OsmPath -> $osm"
    }
    if (-not $SkipBasemap) {
        Write-PlannedCommand "basemap" "docker run --rm -v `"$workDir`:/data`" $($cfg.images.planetiler) --force --download --osm-path=/data/$Region.osm.pbf --output=/data/$Region.pmtiles --maxzoom=$basemapMaxzoom"
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
    if (-not $SkipGraph) {
        Write-PlannedCommand "graph-build-tool" "cargo build --release --locked --manifest-path tools/native-router/Cargo.toml"
        Write-PlannedCommand "graph" "native-router build --osm $osm --out $graph"
        Write-PlannedCommand "graph-gzip" "gzip $graph -> $graphGz"
    }
    Write-PlannedCommand "manifest" "New-OfflineManifest -BaseUrl $BaseUrl -> $manifest"
    Write-PlannedCommand "verify" "verify.ps1 -PackDir $workDir -AllowMissingRouting:$SkipRouting -SkipGraph:$SkipGraph"
    Write-PlannedCommand "publish" "replace $packDir with the verified pack only after verify succeeds"
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
if (-not $SkipGraph) {
    if (-not (Test-Application "cargo") -and -not (Find-NativeRouterBinary)) {
        throw "cargo not found and no prebuilt native-router; install Rust or run cargo build --release --manifest-path tools/native-router/Cargo.toml"
    }
}

$previousIndex = $env:PINPOINT_OFFLINE_INDEX
$indexBin = $null
$oldPackMoved = $false
$newPackInstalled = $false

try {
    New-Item -ItemType Directory -Force -Path $workDir | Out-Null

    $pinnedSha = ([string]$cfg.source.sha256).Trim().ToLowerInvariant()
    if ($OsmPath) {
        Write-Output "using local OSM extract: $OsmPath"
        Get-OsmExtract -Region $cfg -Destination $osm -SourcePath $OsmPath | Out-Null
    } else {
        Write-Output "downloading $($cfg.geofabrik)"
        if (-not [string]::IsNullOrWhiteSpace($pinnedSha) -and -not $AllowUnpinnedSource) {
            # A pinned build fails on any upstream -latest change. Scheduled
            # builds opt out with -AllowUnpinnedSource and record the new hash.
            Get-OsmExtract -Region $cfg -Destination $osm -ExpectedSha256 $pinnedSha | Out-Null
        } else {
            Get-OsmExtract -Region $cfg -Destination $osm | Out-Null
        }
    }

    $sourceSha = (Get-FileHash -Algorithm SHA256 -Path $osm).Hash.ToLowerInvariant()
    $sourceBytes = (Get-Item $osm).Length
    $downloadedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $sourceUrl = if ($OsmPath) { (Resolve-Path $OsmPath).Path } else { [string]$cfg.geofabrik }
    Write-Output "source sha256: $sourceSha ($sourceBytes bytes)"
    if (-not $OsmPath -and [string]::IsNullOrWhiteSpace($pinnedSha)) {
        Write-Warning "region '$Region' has no pinned source sha256; downloaded unpinned"
        Write-Warning "pin source.sha256 = $sourceSha and source.bytes = $sourceBytes for region '$Region' in regions.json"
    } elseif (-not $OsmPath -and $sourceSha -ne $pinnedSha) {
        # Only reachable with -AllowUnpinnedSource; the pinned path fails earlier.
        Write-Warning "source sha256 changed for '$Region': pinned $pinnedSha, downloaded $sourceSha ($sourceBytes bytes)"
        Write-Warning "continuing because -AllowUnpinnedSource is set; the manifest records the downloaded hash"
    }

    if (-not $SkipBasemap) {
        Write-Output "basemap: $($cfg.images.planetiler)"
        $code = Invoke-NativeCommand { & docker run --rm -v "${workDir}:/data" $cfg.images.planetiler --force --download --osm-path="/data/$Region.osm.pbf" --output="/data/$Region.pmtiles" --maxzoom=$basemapMaxzoom }
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

    if (-not $SkipGraph) {
        $router = Resolve-NativeRouterBinary
        Write-Output "graph: native-router build"
        $code = Invoke-NativeCommand { & $router build --osm $osm --out $graph }
        if ($code -ne 0) { throw "native-router graph build failed" }
        if (-not (Test-Path $graph) -or (Get-Item $graph).Length -eq 0) {
            throw "native-router wrote no graph: $graph"
        }
        Compress-GzipFile -Source $graph -Destination $graphGz
        if (-not (Test-Path $graphGz) -or (Get-Item $graphGz).Length -eq 0) {
            throw "graph compression wrote no file: $graphGz"
        }
    }

    $built = @()
    foreach ($candidate in @($pmtiles, $search, $valhalla)) {
        if (Test-Path $candidate) { $built += $candidate }
    }
    if (-not $SkipGraph) { $built += $graphGz }
    if ($built.Count -eq 0) { throw "no artifacts were built for $Region" }

    Copy-Item (Join-Path $PSScriptRoot "ATTRIBUTION.md") $attribution -Force
    . "$PSScriptRoot/manifest.ps1"
    New-OfflineManifest -Region $cfg -Version "$Region-$(Get-Date -Format yyyyMMdd)" -Files $built -Output $manifest -BaseUrl $BaseUrl `
        -SourceSha256 $sourceSha -SourceBytes $sourceBytes -SourceUrl $sourceUrl -DownloadedAt $downloadedAt | Out-Null

    if ($indexBin) {
        $env:PINPOINT_OFFLINE_INDEX = $indexBin
    }
    & "$PSScriptRoot/verify.ps1" -PackDir $workDir -AllowMissingRouting:$SkipRouting -SkipGraph:$SkipGraph
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
    if (-not $SkipGraph) {
        $publishPlan += @(
            @{ Source = $graphGz; Name = "$Region.graph.bin.gz" },
            @{ Source = $graph; Name = "$Region.graph.bin" }
        )
    }
    $published = @()
    foreach ($entry in $publishPlan) {
        if (Test-Path $entry.Source) {
            $destination = Join-Path $publishDir $entry.Name
            Move-Item -Path $entry.Source -Destination $destination
            $published += $destination
        }
    }
    $expectedPublished = $built.Count + 2
    if (-not $SkipGraph) { $expectedPublished++ }
    if ($published.Count -ne $expectedPublished) {
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

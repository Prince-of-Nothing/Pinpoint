<#
.SYNOPSIS
    Estimate Pinpoint offline pack sizes for one or more regions.

.DESCRIPTION
    Reads scripts/offline/regions.json for the Geofabrik PBF URL and the
    pinned byte size, then estimates the pack artifacts with the ratios
    measured on the Moldova build (docs/offline/moldova.md). Every size
    except the PBF input is an ESTIMATE. The PBF is never downloaded: without
    -Offline a curl HEAD reads Content-Length only.

.PARAMETER Region
    One or more region ids from regions.json. A comma-separated list works
    from any shell, including powershell -File (which passes it literally).

.PARAMETER Geofabrik
    Override the PBF URL, including for an id that is not in regions.json.
    Must point at an .osm.pbf extract. One region only.

.PARAMETER PbfBytes
    PBF size to use when a region has no pinned size. Treated like a pin, so
    it works with -Offline. One region only.

.PARAMETER Offline
    Do not touch the network; use the pinned size or -PbfBytes only.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/estimate.ps1 -Region moldova -Offline

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/estimate.ps1 -Region moldova,monaco -Offline

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/estimate.ps1 -Region romania `
        -Geofabrik https://download.geofabrik.de/europe/romania-latest.osm.pbf `
        -PbfBytes 333657899 -Offline
#>
param(
    [Parameter(Mandatory = $true, Position = 0)][string[]]$Region,
    [string]$Geofabrik,
    [long]$PbfBytes = 0,
    [switch]$Offline
)

$ErrorActionPreference = "Stop"

$assetCapBytes = 2147483648  # 2 GiB, GitHub's per-release-asset limit
$invariant = [System.Globalization.CultureInfo]::InvariantCulture

function Format-Bytes([long]$Bytes) {
    return $Bytes.ToString("N0", $invariant)
}

function Format-MiB([long]$Bytes) {
    return ($Bytes / 1MB).ToString("N1", $invariant)
}

function Format-GiB([long]$Bytes) {
    return ($Bytes / 1GB).ToString("N2", $invariant)
}

function Write-AssetRow([string]$Name, [long]$Bytes, [string]$Basis) {
    Write-Output ("  {0,-24} {1,15} {2,10}  {3}" -f $Name, (Format-Bytes $Bytes), (Format-MiB $Bytes), $Basis)
}

function Get-RemotePbfBytes([string]$Url) {
    $curl = Get-Command curl.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $curl) {
        $curl = Get-Command curl -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    }
    if (-not $curl) { throw "curl not found; install curl (curl.exe on Windows) or use -Offline" }

    # Windows PowerShell 5.1 turns native stderr into errors under Stop.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $lines = & $curl.Source -sIL --fail --connect-timeout 20 --max-time 60 $Url
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previous
    }
    if ($code -ne 0) { throw "HEAD failed for $Url (curl exit $code)" }

    # -L follows Geofabrik's dated redirect; the last Content-Length wins.
    $bytes = 0
    foreach ($line in $lines) {
        if ($line -match '^(?i)content-length:\s*(\d+)\s*$') { $bytes = [long]$Matches[1] }
    }
    if ($bytes -le 0) { throw "HEAD response for $Url has no Content-Length" }
    return $bytes
}

# powershell -File passes "a,b" as one literal string, so split it here.
$Region = @($Region | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" })
if ($Region.Count -eq 0) { throw "-Region must name at least one region" }

if ($Region.Count -gt 1 -and (-not [string]::IsNullOrWhiteSpace($Geofabrik) -or $PbfBytes -gt 0)) {
    throw "-Geofabrik and -PbfBytes apply to one region; pass -Region once per URL/size"
}
if ($PbfBytes -lt 0) { throw "-PbfBytes must be zero or positive" }
if (-not [string]::IsNullOrWhiteSpace($Geofabrik) -and $Geofabrik -notmatch '^https?://.+\.osm\.pbf$') {
    throw "-Geofabrik must be an http(s) URL to an .osm.pbf extract: $Geofabrik"
}

$configPath = Join-Path $PSScriptRoot "regions.json"
if (-not (Test-Path $configPath)) { throw "region config not found: $configPath" }
$regions = Get-Content $configPath -Raw | ConvertFrom-Json

Write-Output "Pinpoint offline pack size estimator - ESTIMATE ONLY"
Write-Output "ratios measured on the Moldova pack (docs/offline/moldova.md):"
Write-Output "  pmtiles   = pbf x 0.92"
Write-Output "  search    = pbf x 0.067"
Write-Output "  graph     = pbf x 1.09"
Write-Output "  graph.gz  = graph x 0.41"
Write-Output "  transfer  = pmtiles + graph.gz   (app download; search SQLite is a build artifact)"
Write-Output "  1 MiB = 1,048,576 B; single-transfer-asset cap = 2 GiB = 2,147,483,648 B"

$failed = $false
foreach ($id in $Region) {
    $property = $regions.PSObject.Properties[$id]
    $cfg = $null
    if ($property) { $cfg = $property.Value }

    $url = $Geofabrik
    if ([string]::IsNullOrWhiteSpace($url) -and $cfg) { $url = $cfg.geofabrik }
    if ([string]::IsNullOrWhiteSpace($url)) {
        if ($cfg) {
            Write-Warning "region '$id' has no geofabrik URL in regions.json; pass -Geofabrik <url.osm.pbf>"
        } else {
            $known = ($regions.PSObject.Properties.Name | Sort-Object) -join ", "
            Write-Warning "unknown region '$id'; known: $known (or pass -Geofabrik <url.osm.pbf>)"
        }
        $failed = $true
        continue
    }

    $pinned = 0
    if ($cfg -and $cfg.source -and $cfg.source.bytes) { $pinned = [long]$cfg.source.bytes }

    $pbf = 0
    $basis = ""
    if ($PbfBytes -gt 0) {
        $pbf = $PbfBytes
        $basis = "EXACT supplied (-PbfBytes)"
    } elseif ($Offline -and $pinned -gt 0) {
        $pbf = $pinned
        $basis = "EXACT pinned (regions.json)"
    } elseif (-not $Offline) {
        try {
            $pbf = Get-RemotePbfBytes $url
            $basis = "EXACT measured (HEAD)"
        } catch {
            if ($pinned -gt 0) {
                Write-Warning "$($_.Exception.Message); using the pinned size"
                $pbf = $pinned
                $basis = "EXACT pinned (HEAD failed)"
            }
        }
    }

    if ($pbf -le 0) {
        if ($Offline) {
            Write-Warning "region '$id' has no pinned PBF size; pass -PbfBytes <bytes>"
        } else {
            Write-Warning "region '$id' has no pinned PBF size and no HEAD measurement; pass -PbfBytes <bytes>"
        }
        $failed = $true
        continue
    }

    $driftNote = $null
    if ($basis -eq "EXACT measured (HEAD)" -and $pinned -gt 0 -and $pinned -ne $pbf) {
        $drift = ($pbf - $pinned).ToString("+#;-#;0", $invariant)
        $driftNote = "  note: pinned {0} B; HEAD drift {1} B" -f (Format-Bytes $pinned), $drift
    }

    $pmtiles = [long][math]::Round($pbf * 0.92)
    $search = [long][math]::Round($pbf * 0.067)
    $graph = [long][math]::Round($pbf * 1.09)
    $graphGz = [long][math]::Round($graph * 0.41)
    $transfer = $pmtiles + $graphGz

    $name = $id
    if ($cfg -and $cfg.name) { $name = $cfg.name }
    Write-Output ""
    Write-Output "region: $id ($name)"
    Write-Output "source: $url"
    if ($driftNote) { Write-Output $driftNote }
    Write-Output ("  {0,-24} {1,15} {2,10}  {3}" -f "asset", "ESTIMATE bytes", "ESTIMATE MiB", "basis")
    Write-AssetRow "pbf extract" $pbf $basis
    Write-AssetRow "pmtiles" $pmtiles "ESTIMATE pbf x 0.92"
    Write-AssetRow "search sqlite" $search "ESTIMATE pbf x 0.067"
    Write-AssetRow "graph installed" $graph "ESTIMATE pbf x 1.09"
    Write-AssetRow "graph .gz transfer" $graphGz "ESTIMATE graph x 0.41"
    Write-AssetRow "transfer total" $transfer "ESTIMATE pmtiles + graph.gz"

    $overCap = @()
    foreach ($asset in @(
        @{ Name = "pmtiles"; Bytes = $pmtiles },
        @{ Name = "search sqlite"; Bytes = $search },
        @{ Name = "graph .gz transfer"; Bytes = $graphGz }
    )) {
        if ($asset.Bytes -gt $assetCapBytes) { $overCap += $asset }
    }
    if ($overCap.Count -eq 0) {
        Write-Output "  cap: all transfer assets are under 2 GiB (2,147,483,648 B)"
    } else {
        foreach ($asset in $overCap) {
            Write-Output ("  FLAG: {0} ESTIMATE {1} GiB > 2 GiB (2,147,483,648 B); split the region or lower maxzoom" -f $asset.Name, (Format-GiB $asset.Bytes))
        }
    }
}

if ($failed) { exit 1 }

function New-PmtilesFixture {
    param([Parameter(Mandatory = $true)][string]$Path)

    $parent = Split-Path -Parent $Path
    if ($parent -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $header = [Text.Encoding]::ASCII.GetBytes("PMTiles") + [byte[]](3) + (New-Object byte[] 24)
    [IO.File]::WriteAllBytes($Path, $header)
}

function New-VerifyFixture {
    param(
        [Parameter(Mandatory = $true)][string]$Dir,
        [string]$BaseUrl = "https://example.test/packs/moldova"
    )

    $ErrorActionPreference = "Stop"
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null

    $repo = Resolve-Path (Join-Path $PSScriptRoot "../../..")
    $bin = @(
        (Join-Path $repo "target/debug/pinpoint-offline-index.exe"),
        (Join-Path $repo "target/debug/pinpoint-offline-index"),
        (Join-Path $repo "target/release/pinpoint-offline-index.exe"),
        (Join-Path $repo "target/release/pinpoint-offline-index")
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $bin) {
        throw "offline index binary not found; run cargo build --features offline-build --bin pinpoint-offline-index"
    }

    $pmtiles = Join-Path $Dir "moldova.pmtiles"
    New-PmtilesFixture -Path $pmtiles

    $search = Join-Path $Dir "moldova-search.sqlite"
    & $bin --fixture $search | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "fixture index failed" }

    $tar = Join-Path $Dir "moldova-valhalla.tar"
    $tileDir = Join-Path $Dir "valhalla_tiles/0/000/000"
    New-Item -ItemType Directory -Force -Path $tileDir | Out-Null
    Set-Content -Path (Join-Path $tileDir "000.gph") -Value "gph" -NoNewline
    & tar -cf $tar -C $Dir "valhalla_tiles"
    if ($LASTEXITCODE -ne 0) { throw "fixture tar failed" }

    $attribution = Join-Path $Dir "ATTRIBUTION.md"
    Set-Content -Path $attribution -Value "Map data © OpenStreetMap contributors, ODbL 1.0." -Encoding UTF8

    . "$PSScriptRoot/../manifest.ps1"
    $region = [pscustomobject]@{
        id = "moldova"
        name = "Moldova"
        bbox = @(26.6, 45.4, 30.2, 48.5)
        minzoom = 0
        maxzoom = 14
    }
    New-OfflineManifest -Region $region -Version "moldova-test" -Files @($pmtiles, $search, $tar) -Output (Join-Path $Dir "manifest.json") -BaseUrl $BaseUrl | Out-Null
}

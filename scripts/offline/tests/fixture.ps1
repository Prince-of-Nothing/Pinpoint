function New-VerifyFixture {
    param([Parameter(Mandatory = $true)][string]$Dir)

    $ErrorActionPreference = "Stop"
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null

    $repo = Resolve-Path (Join-Path $PSScriptRoot "../../..")
    $bin = Get-ChildItem (Join-Path $repo "target/debug") -Filter "pinpoint-offline-index*" -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq "pinpoint-offline-index" -or $_.Name -eq "pinpoint-offline-index.exe" } |
        Select-Object -First 1
    if (-not $bin) {
        throw "offline index binary not found; run cargo build --features offline-build --bin pinpoint-offline-index"
    }

    $pmtiles = Join-Path $Dir "moldova.pmtiles"
    $header = [Text.Encoding]::ASCII.GetBytes("PMTiles") + [byte[]](3) + (New-Object byte[] 24)
    [IO.File]::WriteAllBytes($pmtiles, $header)

    $search = Join-Path $Dir "moldova-search.sqlite"
    & $bin.FullName --fixture $search | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "fixture index failed" }

    $tar = Join-Path $Dir "moldova-valhalla.tar"
    $tileDir = Join-Path $Dir "valhalla_tiles/0/000/000"
    New-Item -ItemType Directory -Force -Path $tileDir | Out-Null
    Set-Content -Path (Join-Path $tileDir "000.gph") -Value "gph" -NoNewline
    & tar -cf $tar -C $Dir "valhalla_tiles"
    if ($LASTEXITCODE -ne 0) { throw "fixture tar failed" }

    $attribution = Join-Path $Dir "ATTRIBUTION.md"
    $canonical = @(
        "Map data $([char]0x00A9) OpenStreetMap contributors, available under the Open Database License (ODbL) 1.0."
        ""
        "https://www.openstreetmap.org/copyright"
        "https://opendatacommons.org/licenses/odbl/1-0/"
    ) -join "`n"
    [IO.File]::WriteAllText($attribution, $canonical, (New-Object Text.UTF8Encoding($false)))

    . "$PSScriptRoot/../manifest.ps1"
    $region = [pscustomobject]@{
        id = "moldova"
        name = "Moldova"
        bbox = @(26.6, 45.4, 30.2, 48.5)
        minzoom = 0
        maxzoom = 14
    }
    New-OfflineManifest -Region $region -Version "moldova-20260101" -Files @($pmtiles, $search, $tar) -Output (Join-Path $Dir "manifest.json") | Out-Null
}

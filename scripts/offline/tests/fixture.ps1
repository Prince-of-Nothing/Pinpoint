function New-PmtilesFixture {
    param([Parameter(Mandatory = $true)][string]$Path)

    $parent = Split-Path -Parent $Path
    if ($parent -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $header = [Text.Encoding]::ASCII.GetBytes("PMTiles") + [byte[]](3) + (New-Object byte[] 24)
    [IO.File]::WriteAllBytes($Path, $header)
}

function New-GzipFixture {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

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

    $graph = Join-Path $Dir "moldova.graph.bin"
    [IO.File]::WriteAllText($graph, "graph", (New-Object Text.UTF8Encoding($false)))
    $graphGz = "$graph.gz"
    New-GzipFixture -Source $graph -Destination $graphGz

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
    New-OfflineManifest -Region $region -Version "moldova-20260101" -Files @($pmtiles, $search, $tar, $graphGz) -Output (Join-Path $Dir "manifest.json") -BaseUrl "https://packs.pinpoint.example/moldova" | Out-Null
}

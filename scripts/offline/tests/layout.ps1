$ErrorActionPreference = "Stop"
. "$PSScriptRoot/fixture.ps1"

$root = Join-Path ([IO.Path]::GetTempPath()) ("offline-layout-" + [guid]::NewGuid())
$pbf = Join-Path $PSScriptRoot "fixtures/monaco.osm.pbf"
$packDir = Join-Path $root "monaco"
$manifest = Join-Path $packDir "manifest.json"
$expected = @("ATTRIBUTION.md", "manifest.json", "monaco-search.sqlite", "monaco.pmtiles")
$config = Get-Content (Join-Path $PSScriptRoot "../regions.json") -Raw | ConvertFrom-Json
$configuredBase = $config.monaco.base_url.TrimEnd("/")
$pbfHash = (Get-FileHash -Algorithm SHA256 -Path $pbf).Hash.ToLowerInvariant()
$pbfBytes = (Get-Item $pbf).Length
$pbfPath = (Resolve-Path $pbf).Path

function Get-PublishedNames {
    @(Get-ChildItem $packDir -Force | Select-Object -ExpandProperty Name)
}

function Assert-ManifestUrls([string]$BaseUrl) {
    $doc = Get-Content $manifest -Raw | ConvertFrom-Json
    if ($doc.files.Count -ne 2) { throw "expected two published payload files, got $($doc.files.Count)" }
    foreach ($file in $doc.files) {
        if ([string]::IsNullOrWhiteSpace($file.url)) { throw "manifest url empty for $($file.name)" }
        $wanted = "$BaseUrl/$($file.name)"
        if ($file.url -ne $wanted) { throw "manifest url mismatch: $($file.url) != $wanted" }
        if ($file.name -like "*.osm.pbf") { throw "manifest lists the source PBF" }
    }
    if ($doc.source_sha256 -ne $pbfHash) { throw "manifest source sha256 mismatch" }
    if ($doc.source_bytes -ne $pbfBytes) { throw "manifest source bytes mismatch" }
    $expectedSource = "local:" + (Split-Path -Leaf $pbfPath)
    if ($doc.source_url -ne $expectedSource) { throw "manifest source url mismatch: $($doc.source_url) != $expectedSource" }
    # ConvertFrom-Json on pwsh 7 turns ISO timestamps into DateTime objects.
    $stamp = $doc.downloaded_at
    if ($stamp -is [datetime]) {
        $stamp = $stamp.ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    }
    if ($stamp -notmatch '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|\+00:00)$') {
        throw "manifest downloaded_at is not UTC: $stamp"
    }
}

try {
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $fixturePmtiles = Join-Path $root "fixture.pmtiles"
    New-PmtilesFixture -Path $fixturePmtiles

    $good = & "$PSScriptRoot/../build.ps1" -Region monaco -OutputRoot $root `
        -OsmPath $pbf -BasemapPath $fixturePmtiles `
        -SkipDownload -SkipBasemap -SkipRouting -SkipGraph 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "build failed: $good" }

    $actual = Get-PublishedNames
    $layoutDiff = Compare-Object -ReferenceObject $expected -DifferenceObject $actual -CaseSensitive
    if ($layoutDiff) {
        throw "published layout mismatch: $($actual -join ", ")"
    }
    foreach ($unexpected in @("monaco.osm.pbf", "valhalla_tiles", "monaco-valhalla.json", "sources")) {
        if (Test-Path (Join-Path $packDir $unexpected)) { throw "published build input: $unexpected" }
    }
    Assert-ManifestUrls $configuredBase

    $override = "https://packs.test/monaco"
    $good = & "$PSScriptRoot/../build.ps1" -Region monaco -OutputRoot $root `
        -OsmPath $pbf -BasemapPath $fixturePmtiles -BaseUrl $override `
        -SkipDownload -SkipBasemap -SkipRouting -SkipGraph 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "override build failed: $good" }
    Assert-ManifestUrls $override

    Write-Host "layout ok"
} finally {
    Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}

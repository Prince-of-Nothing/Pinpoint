$ErrorActionPreference = "Stop"
. "$PSScriptRoot/fixture.ps1"

$root = Join-Path ([IO.Path]::GetTempPath()) ("offline-atomic-" + [guid]::NewGuid())
$pbf = Join-Path $PSScriptRoot "fixtures/monaco.osm.pbf"
$packDir = Join-Path $root "monaco"
$manifest = Join-Path $packDir "manifest.json"
$expected = @("ATTRIBUTION.md", "manifest.json", "monaco-search.sqlite", "monaco.pmtiles")

try {
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $fixturePmtiles = Join-Path $root "fixture.pmtiles"
    New-PmtilesFixture -Path $fixturePmtiles

    $good = & "$PSScriptRoot/../build.ps1" -Region monaco -OutputRoot $root `
        -OsmPath $pbf -BasemapPath $fixturePmtiles `
        -SkipDownload -SkipBasemap -SkipRouting -SkipGraph 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "good build failed: $good" }
    if (-not (Test-Path $manifest)) { throw "good build published no manifest" }

    $actual = @(Get-ChildItem $packDir -Force | Select-Object -ExpandProperty Name)
    $layoutDiff = Compare-Object -ReferenceObject $expected -DifferenceObject $actual -CaseSensitive
    if ($layoutDiff) {
        throw "published layout mismatch: $($actual -join ", ")"
    }

    $beforeHash = (Get-FileHash -Algorithm SHA256 -Path $manifest).Hash
    $beforeText = Get-Content $manifest -Raw

    $empty = Join-Path $root "empty.osm.pbf"
    [IO.File]::WriteAllBytes($empty, (New-Object byte[] 0))

    $failed = $false
    try {
        & "$PSScriptRoot/../build.ps1" -Region monaco -OutputRoot $root `
            -OsmPath $empty -SkipDownload -SkipBasemap -SkipRouting -SkipGraph 2>&1 | Out-String | Out-Null
    } catch {
        $failed = $true
    }
    if (-not $failed) { throw "failing build was accepted" }

    if ((Get-FileHash -Algorithm SHA256 -Path $manifest).Hash -ne $beforeHash) {
        throw "failing build changed the published manifest"
    }
    if ((Get-Content $manifest -Raw) -ne $beforeText) {
        throw "failing build rewrote the published manifest"
    }
    $after = @(Get-ChildItem $packDir -Force | Select-Object -ExpandProperty Name)
    $afterDiff = Compare-Object -ReferenceObject $expected -DifferenceObject $after -CaseSensitive
    if ($afterDiff) {
        throw "failing build changed the published layout: $($after -join ", ")"
    }
    if (Get-ChildItem $root -Force -Directory | Where-Object { $_.Name -like ".staging*" }) {
        throw "failing build left a staging directory"
    }
    if (Get-ChildItem $root -Recurse -Force -File | Where-Object { $_.Name -like "*.part" }) {
        throw "failing build left a .part file"
    }

    # Failure after the manifest exists in the work directory but verification
    # rejects the basemap: the published pack must still be untouched.
    $badPmtiles = Join-Path $root "bad.pmtiles"
    [IO.File]::WriteAllBytes($badPmtiles, [Text.Encoding]::ASCII.GetBytes("not a pmtiles archive"))
    $failed = $false
    try {
        & "$PSScriptRoot/../build.ps1" -Region monaco -OutputRoot $root `
            -OsmPath $pbf -BasemapPath $badPmtiles `
            -SkipDownload -SkipBasemap -SkipRouting -SkipGraph 2>&1 | Out-String | Out-Null
    } catch {
        $failed = $true
    }
    if (-not $failed) { throw "build with an invalid basemap was accepted" }
    if ((Get-FileHash -Algorithm SHA256 -Path $manifest).Hash -ne $beforeHash) {
        throw "failed verification changed the published manifest"
    }
    $afterVerify = @(Get-ChildItem $packDir -Force | Select-Object -ExpandProperty Name)
    if (Compare-Object -ReferenceObject $expected -DifferenceObject $afterVerify -CaseSensitive) {
        throw "failed verification changed the published layout"
    }
    if (Get-ChildItem $root -Force -Directory | Where-Object { $_.Name -like ".staging*" }) {
        throw "failed verification left a staging directory"
    }

    Write-Host "atomic ok"
} finally {
    Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}

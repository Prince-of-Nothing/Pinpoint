$ErrorActionPreference = "Stop"
. "$PSScriptRoot/fixture.ps1"

$root = Join-Path ([IO.Path]::GetTempPath()) ("offline-catalog-" + [guid]::NewGuid())
$baseUrl = "https://github.com/Prince-of-Nothing/pinpoint-maps/releases/download/data-moldova-20261117"
$catalog = Join-Path $root "catalog.json"

function New-CatalogRegion {
    param(
        [Parameter(Mandatory = $true)][string]$Dir,
        [Parameter(Mandatory = $true)][string]$Id,
        [Parameter(Mandatory = $true)][string]$Name,
        [int]$Maxzoom = 14
    )

    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    $pmtiles = Join-Path $Dir "$Id.pmtiles"
    New-PmtilesFixture -Path $pmtiles
    $graph = Join-Path $Dir "$Id.graph.bin"
    [IO.File]::WriteAllText($graph, "graph-$Id", (New-Object Text.UTF8Encoding($false)))
    $graphGz = "$graph.gz"
    New-GzipFixture -Source $graph -Destination $graphGz

    . "$PSScriptRoot/../manifest.ps1"
    $region = [pscustomobject]@{
        id = $Id
        name = $Name
        bbox = @(20.0, 43.0, 30.0, 49.0)
        minzoom = 0
        maxzoom = $Maxzoom
    }
    New-OfflineManifest -Region $region -Version "$Id-20260101" -Files @($pmtiles, $graphGz) `
        -Output (Join-Path $Dir "manifest.json") -BaseUrl "https://packs.example/$Id" `
        -SourceSha256 ("b" * 64) -SourceBytes 123456 `
        -SourceUrl "https://download.geofabrik.de/europe/$Id-latest.osm.pbf" `
        -DownloadedAt "2026-11-17T12:00:00Z" | Out-Null
    return [pscustomobject]@{
        Dir = $Dir
        Id = $Id
        Pmtiles = $pmtiles
        Graph = $graph
        GraphGz = $graphGz
    }
}

function Invoke-Catalog($RegionDir, $CatalogPath, $Url, $Version) {
    & "$PSScriptRoot/../catalog.ps1" -RegionDir $RegionDir -Catalog $CatalogPath -BaseUrl $Url -Version $Version
}

function Assert-CatalogFails {
    param(
        [Parameter(Mandatory = $true)][string]$RegionDir,
        [Parameter(Mandatory = $true)][string]$CatalogPath,
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][string]$Expected
    )

    $message = $null
    try {
        Invoke-Catalog $RegionDir $CatalogPath $Url $Version | Out-Null
    } catch {
        $message = $_.Exception.Message
    }
    if ($null -eq $message) { throw "catalog accepted bad input; expected: $Expected" }
    if (-not $message.Contains($Expected)) {
        throw "unexpected catalog failure: $message (expected: $Expected)"
    }
    if (Test-Path $CatalogPath) { throw "failed catalog run wrote $CatalogPath" }
}

try {
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $moldova = New-CatalogRegion -Dir (Join-Path $root "moldova") -Id "moldova" -Name "Moldova"

    $output = Invoke-Catalog $moldova.Dir $catalog $baseUrl "2026-11-17" | Out-String
    if ($output -notmatch "1 region") { throw "catalog did not report one region" }
    if (-not (Test-Path $catalog)) { throw "catalog was not created" }

    $doc = [IO.File]::ReadAllText($catalog, [Text.Encoding]::UTF8) | ConvertFrom-Json
    if ($doc.schema -ne 1) { throw "schema mismatch" }
    if (@($doc.regions).Count -ne 1) { throw "region count mismatch" }
    $region = @($doc.regions)[0]
    if ($region.id -ne "moldova") { throw "id mismatch" }
    if ($region.name -ne "Moldova") { throw "name mismatch" }
    if (@($region.bbox).Count -ne 4) { throw "bbox mismatch" }
    if ($region.maxzoom -ne 14) { throw "maxzoom mismatch" }
    if ($region.version -ne "2026-11-17") { throw "version mismatch" }
    if ($region.source_sha256 -ne ("b" * 64)) { throw "source sha256 mismatch" }
    if (@($region.files).Count -ne 2) { throw "file count mismatch" }

    $basemap = @($region.files)[0]
    if ($basemap.role -ne "basemap") { throw "basemap role mismatch" }
    if ($basemap.transfer_name -ne "moldova.pmtiles") { throw "basemap transfer name mismatch" }
    if ($basemap.installed_name -ne "moldova.pmtiles") { throw "basemap installed name mismatch" }
    if ($basemap.url -ne "$baseUrl/moldova.pmtiles") { throw "basemap url mismatch" }
    if ($basemap.transfer_bytes -ne (Get-Item $moldova.Pmtiles).Length) { throw "basemap transfer bytes mismatch" }
    if ($basemap.transfer_sha256 -ne (Get-FileHash -Algorithm SHA256 -Path $moldova.Pmtiles).Hash.ToLowerInvariant()) { throw "basemap hash mismatch" }
    if ($basemap.compression -ne "identity") { throw "basemap compression mismatch" }

    $graph = @($region.files)[1]
    if ($graph.role -ne "graph") { throw "graph role mismatch" }
    if ($graph.transfer_name -ne "moldova.graph.bin.gz") { throw "graph transfer name mismatch" }
    if ($graph.installed_name -ne "moldova.graph.bin") { throw "graph installed name mismatch" }
    if ($graph.url -ne "$baseUrl/moldova.graph.bin.gz") { throw "graph url mismatch" }
    if ($graph.transfer_bytes -ne (Get-Item $moldova.GraphGz).Length) { throw "graph transfer bytes mismatch" }
    if ($graph.installed_bytes -ne (Get-Item $moldova.Graph).Length) { throw "graph installed bytes mismatch" }
    if ($graph.transfer_sha256 -ne (Get-FileHash -Algorithm SHA256 -Path $moldova.GraphGz).Hash.ToLowerInvariant()) { throw "graph transfer hash mismatch" }
    if ($graph.installed_sha256 -ne (Get-FileHash -Algorithm SHA256 -Path $moldova.Graph).Hash.ToLowerInvariant()) { throw "graph installed hash mismatch" }
    if ($graph.compression -ne "gzip") { throw "graph compression mismatch" }

    $raw = [IO.File]::ReadAllBytes($catalog)
    if ($raw.Length -ge 3 -and $raw[0] -eq 0xEF -and $raw[1] -eq 0xBB -and $raw[2] -eq 0xBF) {
        throw "catalog has a UTF-8 BOM"
    }

    # Re-running the same region replaces its entry instead of duplicating it.
    Invoke-Catalog $moldova.Dir $catalog $baseUrl "2026-11-18" | Out-Null
    $doc = [IO.File]::ReadAllText($catalog, [Text.Encoding]::UTF8) | ConvertFrom-Json
    if (@($doc.regions).Count -ne 1) { throw "same-region rerun duplicated the entry" }
    if (@($doc.regions)[0].version -ne "2026-11-18") { throw "same-region rerun did not update the version" }

    # Merging a second region keeps deterministic id ordering.
    $romania = New-CatalogRegion -Dir (Join-Path $root "romania") -Id "romania" -Name "Romania" -Maxzoom 13
    Invoke-Catalog $romania.Dir $catalog $baseUrl "2026-11-17" | Out-Null
    $doc = [IO.File]::ReadAllText($catalog, [Text.Encoding]::UTF8) | ConvertFrom-Json
    $ids = @($doc.regions | ForEach-Object { $_.id })
    if ($ids.Count -ne 2) { throw "merge did not keep both regions" }
    if ($ids[0] -ne "moldova" -or $ids[1] -ne "romania") { throw "region order is not deterministic: $($ids -join ',')" }
    if (@($doc.regions)[0].maxzoom -ne 14 -or @($doc.regions)[1].maxzoom -ne 13) {
        throw "merged maxzoom mismatch"
    }
    foreach ($file in @($doc.regions)[0].files) {
        if ($file.transfer_sha256 -cne $file.transfer_sha256.ToLowerInvariant()) { throw "hash is not lowercase" }
    }

    $badCatalog = Join-Path $root "bad.json"
    Assert-CatalogFails -RegionDir $moldova.Dir -CatalogPath $badCatalog -Url "ftp://example.test/data" -Version "2026-11-17" -Expected "BaseUrl"
    Assert-CatalogFails -RegionDir $moldova.Dir -CatalogPath $badCatalog -Url $baseUrl -Version "bad version!" -Expected "Version"

    $noGraph = Join-Path $root "nograph"
    New-Item -ItemType Directory -Force -Path $noGraph | Out-Null
    $manifest = [IO.File]::ReadAllText((Join-Path $moldova.Dir "manifest.json"), [Text.Encoding]::UTF8) | ConvertFrom-Json
    $manifest.files = @($manifest.files | Where-Object { $_.name -ne "moldova.graph.bin.gz" })
    [IO.File]::WriteAllText((Join-Path $noGraph "manifest.json"), ($manifest | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding($false)))
    Assert-CatalogFails -RegionDir $noGraph -CatalogPath $badCatalog -Url $baseUrl -Version "2026-11-17" -Expected "has no graph"

    $tampered = Join-Path $root "tampered"
    Copy-Item -Path $moldova.Dir -Destination $tampered -Recurse
    $graphPath = Join-Path $tampered "moldova.graph.bin"
    $bytes = [IO.File]::ReadAllBytes($graphPath)
    $bytes[0] = $bytes[0] -bxor 0xFF
    [IO.File]::WriteAllBytes($graphPath, $bytes)
    Assert-CatalogFails -RegionDir $tampered -CatalogPath $badCatalog -Url $baseUrl -Version "2026-11-17" -Expected "graph installed SHA-256 mismatch"

    Write-Host "catalog ok"
} finally {
    Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}

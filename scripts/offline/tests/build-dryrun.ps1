$ErrorActionPreference = "Stop"
$root = Join-Path ([IO.Path]::GetTempPath()) ("offline-dryrun-" + [guid]::NewGuid())
$out = & "$PSScriptRoot/../build.ps1" -Region moldova -OutputRoot $root -DryRun 2>&1 | Out-String
foreach ($needle in @("moldova.pmtiles", "moldova-search.sqlite", "moldova-valhalla.tar", "moldova.graph.bin", "moldova.graph.bin.gz", "planetiler", "valhalla_build_config", "valhalla_build_tiles", "pinpoint-offline-index", "native-router", "sha256:", "packs.pinpoint.example", ".staging", "--mjolnir-concurrency 2", "maxzoom: 14")) {
    if ($out -notmatch [regex]::Escape($needle)) { throw "dry-run missing $needle" }
}
if ($out -notmatch "DRY RUN") { throw "dry-run marker missing" }
if ($out -notmatch "publish") { throw "dry-run does not describe publication" }
if ($out -notmatch "allow unpinned source: False") { throw "dry-run did not show the default source-pin policy" }

$out = & "$PSScriptRoot/../build.ps1" -Region moldova -OutputRoot $root -DryRun -Maxzoom 10 -AllowUnpinnedSource 2>&1 | Out-String
if ($out -notmatch "maxzoom: 10") { throw "dry-run ignored -Maxzoom" }
if ($out -notmatch "--maxzoom=10") { throw "dry-run did not pass -Maxzoom to planetiler" }
if ($out -notmatch "allow unpinned source: True") { throw "dry-run ignored -AllowUnpinnedSource" }

$out = & "$PSScriptRoot/../build.ps1" -Region moldova -OutputRoot $root -DryRun -SkipGraph 2>&1 | Out-String
if ($out -match "native-router") { throw "dry-run planned the graph with -SkipGraph" }
if ($out -match [regex]::Escape("moldova.graph.bin.gz")) { throw "dry-run published graph files with -SkipGraph" }

$failed = $false
try {
    & "$PSScriptRoot/../build.ps1" -Region moldova -OutputRoot $root -DryRun -Maxzoom 15 2>&1 | Out-String | Out-Null
} catch {
    $failed = $true
}
if (-not $failed) { throw "dry-run accepted maxzoom above the cap" }
if (Test-Path $root) { throw "dry-run created the output directory" }
Write-Host "build dry-run ok"

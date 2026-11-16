$ErrorActionPreference = "Stop"
$root = Join-Path ([IO.Path]::GetTempPath()) ("offline-dryrun-" + [guid]::NewGuid())
$out = & "$PSScriptRoot/../build.ps1" -Region moldova -OutputRoot $root -DryRun 2>&1 | Out-String
foreach ($needle in @("moldova.pmtiles", "moldova-search.sqlite", "moldova-valhalla.tar", "planetiler", "valhalla_build_config", "valhalla_build_tiles", "pinpoint-offline-index")) {
    if ($out -notmatch [regex]::Escape($needle)) { throw "dry-run missing $needle" }
}
if ($out -notmatch "DRY RUN") { throw "dry-run marker missing" }
if (Test-Path $root) { throw "dry-run created the output directory" }
Write-Host "build dry-run ok"

$ErrorActionPreference = "Stop"
. "$PSScriptRoot/../download.ps1"

$src = Join-Path ([IO.Path]::GetTempPath()) ("osm-src-" + [guid]::NewGuid() + ".pbf")
$dst = Join-Path ([IO.Path]::GetTempPath()) ("osm-dst-" + [guid]::NewGuid() + ".pbf")
$part = "$dst.part"
Set-Content -Path $src -Value "hello osm" -NoNewline

try {
    Get-OsmExtract -Region @{ name = "test" } -Destination $dst -SourcePath $src | Out-Null
    if (-not (Test-Path $dst)) { throw "destination missing" }
    if ((Get-Content $dst -Raw) -ne "hello osm") { throw "content mismatch" }
    Remove-Item $dst -Force

    $threw = $false
    try {
        Get-OsmExtract -Region @{} -Destination $dst -SourcePath $src -ExpectedSha256 ("0" * 64) | Out-Null
    } catch {
        $threw = $true
    }
    if (-not $threw) { throw "bad hash accepted" }
    if (Test-Path $dst) { throw "failed download left destination" }
    if (Test-Path $part) { throw "failed download left .part" }
    Write-Host "download ok"
} finally {
    Remove-Item $src, $dst, $part -Force -ErrorAction SilentlyContinue
}

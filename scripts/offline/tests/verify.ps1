$ErrorActionPreference = "Stop"
. "$PSScriptRoot/fixture.ps1"

$licenseUrl = "https://opendatacommons.org/licenses/odbl/1-0/"
$odblAttribution = "$([char]0x00A9) OpenStreetMap contributors"

function Invoke-Verify {
    param(
        [Parameter(Mandatory = $true)][string]$PackDir,
        [switch]$AllowEmptyUrls,
        [switch]$SkipGraph
    )

    $verifyArgs = @{
        PackDir = $PackDir
        AllowEmptyUrls = [bool]$AllowEmptyUrls
        SkipGraph = [bool]$SkipGraph
    }
    & "$PSScriptRoot/../verify.ps1" @verifyArgs
}

function Assert-VerifyPasses {
    param(
        [Parameter(Mandatory = $true)][string]$PackDir,
        [switch]$AllowEmptyUrls,
        [switch]$SkipGraph
    )

    try {
        Invoke-Verify -PackDir $PackDir -AllowEmptyUrls:$AllowEmptyUrls -SkipGraph:$SkipGraph
    } catch {
        throw "verify rejected a good pack: $($_.Exception.Message)"
    }
}

function Assert-VerifyFails {
    param(
        [Parameter(Mandatory = $true)][string]$PackDir,
        [Parameter(Mandatory = $true)][string]$Expected,
        [switch]$AllowEmptyUrls,
        [switch]$SkipGraph
    )

    $message = $null
    try {
        Invoke-Verify -PackDir $PackDir -AllowEmptyUrls:$AllowEmptyUrls -SkipGraph:$SkipGraph
    } catch {
        $message = $_.Exception.Message
    }
    if ($null -eq $message) { throw "verify accepted a bad pack; expected: $Expected" }
    if (-not $message.Contains($Expected)) {
        throw "unexpected verify failure: $message (expected: $Expected)"
    }
}

function Read-Manifest([string]$Dir) {
    [IO.File]::ReadAllText((Join-Path $Dir "manifest.json"), [Text.Encoding]::UTF8) | ConvertFrom-Json
}

function Write-Manifest([string]$Dir, $Document) {
    $json = $Document | ConvertTo-Json -Depth 6
    [IO.File]::WriteAllText((Join-Path $Dir "manifest.json"), $json, (New-Object Text.UTF8Encoding($false)))
}

function Copy-Pack([string]$Source, [string]$Destination) {
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-Item -Path (Join-Path $Source "*") -Destination $Destination -Recurse
}

$root = Join-Path ([IO.Path]::GetTempPath()) ("offline-verify-" + [guid]::NewGuid())
try {
    $base = Join-Path $root "base"
    New-VerifyFixture -Dir $base

    Assert-VerifyPasses -PackDir $base

    $dir = Join-Path $root "empty-url"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.files[0].url = ""
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -Expected "empty artifact url: moldova.pmtiles"

    $dir = Join-Path $root "uppercase-hash"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.files[0].sha256 = $doc.files[0].sha256.ToUpperInvariant()
    Write-Manifest $dir $doc
    Assert-VerifyPasses -PackDir $dir -AllowEmptyUrls

    $dir = Join-Path $root "tampered"
    Copy-Pack $base $dir
    $pmtiles = Join-Path $dir "moldova.pmtiles"
    $bytes = [IO.File]::ReadAllBytes($pmtiles)
    $bytes[10] = $bytes[10] -bxor 0xFF
    [IO.File]::WriteAllBytes($pmtiles, $bytes)
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "SHA-256 mismatch: moldova.pmtiles"

    $dir = Join-Path $root "size-mismatch"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.files[0].bytes = $doc.files[0].bytes + 1
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "size mismatch: moldova.pmtiles"

    $dir = Join-Path $root "pmtiles-version"
    Copy-Pack $base $dir
    $pmtiles = Join-Path $dir "moldova.pmtiles"
    $bytes = [IO.File]::ReadAllBytes($pmtiles)
    $bytes[7] = 4
    [IO.File]::WriteAllBytes($pmtiles, $bytes)
    $doc = Read-Manifest $dir
    $entry = $doc.files | Where-Object { $_.name -eq "moldova.pmtiles" }
    $entry.sha256 = (Get-FileHash -Algorithm SHA256 -Path $pmtiles).Hash.ToLowerInvariant()
    $entry.bytes = (Get-Item $pmtiles).Length
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "unsupported PMTiles version 4 (expected 3)"

    $dir = Join-Path $root "traversal-slash"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.files[0].name = "../outside.bin"
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "unsafe artifact name: ../outside.bin"

    $dir = Join-Path $root "traversal-backslash"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.files[0].name = '..\outside.bin'
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected 'unsafe artifact name: ..\outside.bin'

    $dir = Join-Path $root "attribution-missing"
    Copy-Pack $base $dir
    Remove-Item (Join-Path $dir "ATTRIBUTION.md") -Force
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "ATTRIBUTION.md missing"

    $dir = Join-Path $root "attribution-no-odbl"
    Copy-Pack $base $dir
    $text = "Map data $([char]0x00A9) OpenStreetMap contributors.`n$licenseUrl`n"
    [IO.File]::WriteAllText((Join-Path $dir "ATTRIBUTION.md"), $text, (New-Object Text.UTF8Encoding($false)))
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "ATTRIBUTION.md does not name ODbL"

    $dir = Join-Path $root "attribution-no-url"
    Copy-Pack $base $dir
    $text = "Map data $([char]0x00A9) OpenStreetMap contributors, ODbL 1.0.`n"
    [IO.File]::WriteAllText((Join-Path $dir "ATTRIBUTION.md"), $text, (New-Object Text.UTF8Encoding($false)))
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "ATTRIBUTION.md does not link the ODbL license"

    $dir = Join-Path $root "attribution-no-copyright"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.attribution = "OpenStreetMap contributors"
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "ODbL attribution must be '$odblAttribution'"

    $dir = Join-Path $root "license-mismatch"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.license = "CC-BY-4.0"
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "ODbL license must be ODbL-1.0"

    $dir = Join-Path $root "license-url-mismatch"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.license_url = "https://example.test/licenses/odbl"
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "ODbL license URL must be $licenseUrl"

    $dir = Join-Path $root "graph-transfer-missing"
    Copy-Pack $base $dir
    Remove-Item (Join-Path $dir "moldova.graph.bin.gz") -Force
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "missing artifact: moldova.graph.bin.gz"

    $dir = Join-Path $root "graph-installed-missing"
    Copy-Pack $base $dir
    Remove-Item (Join-Path $dir "moldova.graph.bin") -Force
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "missing graph: moldova.graph.bin"

    $dir = Join-Path $root "graph-installed-tampered"
    Copy-Pack $base $dir
    $graph = Join-Path $dir "moldova.graph.bin"
    $bytes = [IO.File]::ReadAllBytes($graph)
    $bytes[0] = $bytes[0] -bxor 0xFF
    [IO.File]::WriteAllBytes($graph, $bytes)
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "graph SHA-256 mismatch: moldova.graph.bin"

    $dir = Join-Path $root "graph-installed-size"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $graphEntry = $doc.files | Where-Object { $_.role -eq "graph" }
    $graphEntry.installed_bytes = $graphEntry.installed_bytes + 1
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "graph size mismatch: moldova.graph.bin"

    $dir = Join-Path $root "graph-entry-missing"
    Copy-Pack $base $dir
    $doc = Read-Manifest $dir
    $doc.files = @($doc.files | Where-Object { $_.name -ne "moldova.graph.bin.gz" })
    Write-Manifest $dir $doc
    Assert-VerifyFails -PackDir $dir -AllowEmptyUrls -Expected "manifest has no routing graph"
    Assert-VerifyPasses -PackDir $dir -AllowEmptyUrls -SkipGraph

    Write-Host "verify ok"
} finally {
    Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}

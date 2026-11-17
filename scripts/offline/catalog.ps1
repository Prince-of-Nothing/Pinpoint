param(
    [Parameter(Mandatory = $true)][string]$RegionDir,
    [Parameter(Mandatory = $true)][string]$Catalog,
    [Parameter(Mandatory = $true)][string]$BaseUrl,
    [Parameter(Mandatory = $true)][string]$Version
)

# Emit or merge a schema-1 catalog entry from one published region directory.
# The catalog is what the app reads; the manifest stays the build record.

$ErrorActionPreference = "Stop"
$maxRegions = 64
$maxFilesPerRegion = 32
$roleRank = @{ basemap = 0; graph = 1 }

function Assert-SafeName([string]$Name, [string]$Label) {
    if ([string]::IsNullOrWhiteSpace($Name) -or
        $Name -eq "." -or
        $Name -eq ".." -or
        $Name -notmatch '^[A-Za-z0-9._-]+$' -or
        [IO.Path]::GetFileName($Name) -ne $Name) {
        throw "$Label is not a safe file name: $Name"
    }
}

function Assert-Sha256([string]$Value, [string]$Label) {
    if ($Value -cnotmatch '^[0-9a-f]{64}$') {
        throw "$Label must be 64 lowercase hex characters: $Value"
    }
}

function Assert-HttpUrl([string]$Value, [string]$Label) {
    if ($Value -notmatch '^https?://[^\s]+$') {
        throw "$Label must be an absolute http(s) URL: $Value"
    }
}

function Assert-PositiveSize($Value, [string]$Label) {
    if ([string]$Value -notmatch '^\d+$') { throw "$Label must be a positive integer: $Value" }
    $number = [long]$Value
    if ($number -le 0) { throw "$Label must be a positive integer: $Value" }
    return $number
}

function Assert-Bbox($Value) {
    $values = @($Value)
    if ($values.Count -ne 4) { throw "bbox must have four numbers, got $($values.Count)" }
    foreach ($item in $values) {
        $number = [double]$item
        if ([double]::IsNaN($number) -or [double]::IsInfinity($number)) {
            throw "bbox must have four finite numbers"
        }
    }
}

function Assert-Maxzoom($Value) {
    if ([string]$Value -notmatch '^\d{1,2}$') {
        throw "maxzoom must be an integer from 0 to 24: $Value"
    }
    $number = [int]$Value
    if ($number -gt 24) { throw "maxzoom must be an integer from 0 to 24: $Value" }
    return $number
}

function Assert-PublishedFile([string]$Dir, [string]$Name, [long]$Bytes, [string]$Sha256, [string]$Label) {
    $path = Join-Path $Dir $Name
    if (-not (Test-Path $path)) { throw "$Label file missing: $Name" }
    $item = Get-Item $path
    if ($item.Length -ne $Bytes) { throw "$Label size mismatch: $Name" }
    $actual = (Get-FileHash -Algorithm SHA256 -Path $path).Hash.ToLowerInvariant()
    if ($actual -ne $Sha256) { throw "$Label SHA-256 mismatch: $Name" }
}

function New-CatalogFileEntry($File, [string]$RegionId, [string]$Base) {
    $role = [string]$File.role
    if ($role -ne "basemap" -and $role -ne "graph") {
        throw "region '$RegionId' has unsupported file role '$role'"
    }
    $transferName = [string]$File.name
    Assert-SafeName $transferName "region '$RegionId' transfer_name"
    $transferBytes = Assert-PositiveSize $File.bytes "region '$RegionId' bytes"
    $transferSha = ([string]$File.sha256).ToLowerInvariant()
    Assert-Sha256 $transferSha "region '$RegionId' sha256"

    if ($role -eq "basemap") {
        $installedName = $transferName
        $installedBytes = $transferBytes
        $installedSha = $transferSha
        $compression = "identity"
    } else {
        $installedName = [string]$File.installed_name
        Assert-SafeName $installedName "region '$RegionId' installed_name"
        if ($installedName -eq $transferName) {
            throw "region '$RegionId' graph installed_name must differ from transfer_name"
        }
        if ($transferName -notlike "*.gz") {
            throw "region '$RegionId' graph transfer_name must end in .gz: $transferName"
        }
        if ($installedName -like "*.gz") {
            throw "region '$RegionId' graph installed_name must be the decompressed file: $installedName"
        }
        $installedBytes = Assert-PositiveSize $File.installed_bytes "region '$RegionId' installed_bytes"
        $installedSha = ([string]$File.installed_sha256).ToLowerInvariant()
        Assert-Sha256 $installedSha "region '$RegionId' installed_sha256"
        $compression = "gzip"
    }

    $url = "$Base/$transferName"
    Assert-HttpUrl $url "region '$RegionId' url"
    return [ordered]@{
        role = $role
        transfer_name = $transferName
        installed_name = $installedName
        url = $url
        transfer_bytes = $transferBytes
        transfer_sha256 = $transferSha
        installed_bytes = $installedBytes
        installed_sha256 = $installedSha
        compression = $compression
    }
}

function Assert-CatalogRegion($Region) {
    $regionId = [string]$Region.id
    if ($regionId -notmatch '^[a-z0-9-]{1,64}$') { throw "catalog region id is invalid: $regionId" }
    if ([string]::IsNullOrWhiteSpace([string]$Region.name)) { throw "catalog region '$regionId' has no name" }
    if ([string]$Region.version -notmatch '^[A-Za-z0-9._-]{1,64}$') {
        throw "catalog region '$regionId' version must match [A-Za-z0-9._-]{1,64}: $($Region.version)"
    }
    Assert-Bbox $Region.bbox
    [void](Assert-Maxzoom $Region.maxzoom)

    $files = @($Region.files)
    if ($files.Count -lt 1 -or $files.Count -gt $maxFilesPerRegion) {
        throw "catalog region '$regionId' must have 1 to $maxFilesPerRegion files, got $($files.Count)"
    }
    $roles = @{}
    foreach ($file in $files) {
        $role = [string]$file.role
        if ($role -ne "basemap" -and $role -ne "graph") {
            throw "catalog region '$regionId' has unsupported role: $role"
        }
        if ($roles.ContainsKey($role)) { throw "catalog region '$regionId' repeats role: $role" }
        $roles[$role] = $true

        $transferName = [string]$file.transfer_name
        $installedName = [string]$file.installed_name
        Assert-SafeName $transferName "catalog region '$regionId' transfer_name"
        Assert-SafeName $installedName "catalog region '$regionId' installed_name"
        Assert-HttpUrl ([string]$file.url) "catalog region '$regionId' url"
        [void](Assert-PositiveSize $file.transfer_bytes "catalog region '$regionId' transfer_bytes")
        [void](Assert-PositiveSize $file.installed_bytes "catalog region '$regionId' installed_bytes")
        Assert-Sha256 ([string]$file.transfer_sha256).ToLowerInvariant() "catalog region '$regionId' transfer_sha256"
        Assert-Sha256 ([string]$file.installed_sha256).ToLowerInvariant() "catalog region '$regionId' installed_sha256"
        if (([string]$file.transfer_sha256) -cne ([string]$file.transfer_sha256).ToLowerInvariant()) {
            throw "catalog region '$regionId' transfer_sha256 must be lowercase"
        }
        if (([string]$file.installed_sha256) -cne ([string]$file.installed_sha256).ToLowerInvariant()) {
            throw "catalog region '$regionId' installed_sha256 must be lowercase"
        }
        if ($role -eq "basemap") {
            if ([string]$file.compression -ne "identity") {
                throw "catalog region '$regionId' basemap compression must be identity"
            }
            if ($installedName -ne $transferName -or
                [long]$file.installed_bytes -ne [long]$file.transfer_bytes -or
                ([string]$file.installed_sha256) -ne ([string]$file.transfer_sha256)) {
                throw "catalog region '$regionId' basemap install fields must match transfer fields"
            }
        } else {
            if ([string]$file.compression -ne "gzip") {
                throw "catalog region '$regionId' graph compression must be gzip"
            }
            if ($installedName -eq $transferName) {
                throw "catalog region '$regionId' graph installed_name must differ from transfer_name"
            }
        }
    }
    if (-not $roles.ContainsKey("basemap")) { throw "catalog region '$regionId' has no basemap" }
    if (-not $roles.ContainsKey("graph")) { throw "catalog region '$regionId' has no graph" }
}

if ([string]::IsNullOrWhiteSpace($BaseUrl) -or $BaseUrl -notmatch '^https?://[^\s]+$') {
    throw "-BaseUrl must be an absolute http(s) URL: $BaseUrl"
}
$BaseUrl = $BaseUrl.TrimEnd('/')
if ($Version -notmatch '^[A-Za-z0-9._-]{1,64}$') {
    throw "-Version must match [A-Za-z0-9._-]{1,64}: $Version"
}

$manifestPath = Join-Path $RegionDir "manifest.json"
if (-not (Test-Path $manifestPath)) { throw "region manifest missing: $manifestPath" }
$manifest = [IO.File]::ReadAllText($manifestPath, [Text.Encoding]::UTF8) | ConvertFrom-Json

$regionId = [string]$manifest.id
if ($regionId -notmatch '^[a-z0-9-]{1,64}$') {
    throw "manifest id is not a catalog region id: $regionId"
}
$manifestFiles = @($manifest.files)
$basemap = $manifestFiles |
    Where-Object { $_.role -eq "basemap" -or ($null -eq $_.role -and $_.name -like "*.pmtiles") } |
    Select-Object -First 1
$graph = $manifestFiles |
    Where-Object { $_.role -eq "graph" -or ($null -eq $_.role -and $_.name -like "*.graph.bin.gz") } |
    Select-Object -First 1
if (-not $basemap) { throw "manifest for '$regionId' has no basemap (.pmtiles)" }
if (-not $graph) { throw "manifest for '$regionId' has no graph (.graph.bin.gz)" }

$basemapEntry = New-CatalogFileEntry $basemap $regionId $BaseUrl
$graphEntry = New-CatalogFileEntry $graph $regionId $BaseUrl
Assert-PublishedFile $RegionDir $basemapEntry.transfer_name $basemapEntry.transfer_bytes $basemapEntry.transfer_sha256 "basemap"
Assert-PublishedFile $RegionDir $graphEntry.transfer_name $graphEntry.transfer_bytes $graphEntry.transfer_sha256 "graph transfer"
Assert-PublishedFile $RegionDir $graphEntry.installed_name $graphEntry.installed_bytes $graphEntry.installed_sha256 "graph installed"

$newRegion = [ordered]@{
    id = $regionId
    name = [string]$manifest.name
    bbox = @($manifest.bbox)
    maxzoom = (Assert-Maxzoom $manifest.maxzoom)
    version = $Version
    files = @($basemapEntry, $graphEntry)
}
Assert-CatalogRegion $newRegion

$regions = @()
if (Test-Path $Catalog) {
    $existing = [IO.File]::ReadAllText($Catalog, [Text.Encoding]::UTF8) | ConvertFrom-Json
    if ($null -eq $existing -or $existing.schema -ne 1) { throw "catalog schema must be 1: $Catalog" }
    $regions = @($existing.regions)
    $ids = @{}
    foreach ($region in $regions) {
        $id = [string]$region.id
        if ($ids.ContainsKey($id)) { throw "catalog has duplicate region id: $id" }
        $ids[$id] = $true
    }
}

$regions = @($regions | Where-Object { [string]$_.id -ne $regionId })
$regions += $newRegion
if ($regions.Count -gt $maxRegions) {
    throw "catalog would have $($regions.Count) regions; the limit is $maxRegions"
}
$regions = @($regions | Sort-Object -Property id)
foreach ($region in $regions) {
    $region.files = @($region.files | Sort-Object { $roleRank[[string]$_.role] }, { [string]$_.transfer_name })
    Assert-CatalogRegion $region
}

$catalogDir = Split-Path -Parent $Catalog
if ($catalogDir -and -not (Test-Path $catalogDir)) {
    New-Item -ItemType Directory -Force -Path $catalogDir | Out-Null
}
$document = [ordered]@{
    schema = 1
    updated = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    regions = @($regions)
}
$part = "$Catalog.part"
Remove-Item $part -Force -ErrorAction SilentlyContinue
try {
    $json = $document | ConvertTo-Json -Depth 8
    [IO.File]::WriteAllText($part, $json, (New-Object System.Text.UTF8Encoding($false)))
    Move-Item -Path $part -Destination $Catalog -Force
} catch {
    Remove-Item $part -Force -ErrorAction SilentlyContinue
    throw
}
Write-Output "catalog: $($regions.Count) region(s) -> $Catalog"

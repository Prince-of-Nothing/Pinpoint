function Get-OfflineArtifactRole([string]$Name) {
    if ($Name -like "*.graph.bin.gz") { return "graph" }
    if ($Name -like "*.pmtiles") { return "basemap" }
    if ($Name -like "*-search.sqlite") { return "search" }
    if ($Name -like "*-valhalla.tar") { return "routing" }
    return "other"
}

function New-OfflineManifest {
    param(
        [Parameter(Mandatory = $true)]$Region,
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][string[]]$Files,
        [Parameter(Mandatory = $true)][string]$Output,
        [string]$BaseUrl,
        [string]$SourceSha256,
        [long]$SourceBytes,
        [string]$SourceUrl,
        [string]$DownloadedAt
    )

    $ErrorActionPreference = "Stop"
    if ([string]::IsNullOrWhiteSpace($Region.id)) { throw "region id is missing" }
    if ([string]::IsNullOrWhiteSpace($Version)) { throw "version is missing" }
    if ($Version -cnotmatch '^[a-z0-9-]+-\d{8}$') { throw "version must match <region>-YYYYMMDD: $Version" }

    $entries = @()
    $seen = @{}
    foreach ($file in $Files) {
        if (-not (Test-Path $file)) { throw "manifest file missing: $file" }
        $item = Get-Item $file
        if ($item.Length -eq 0) { throw "manifest file is empty: $file" }
        if ($seen.ContainsKey($item.Name)) { throw "duplicate manifest file: $($item.Name)" }
        $seen[$item.Name] = $true
        $url = if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
            ""
        } else {
            "$($BaseUrl.TrimEnd('/'))/$($item.Name)"
        }
        $role = Get-OfflineArtifactRole $item.Name
        $entry = [ordered]@{
            name = $item.Name
            role = $role
            url = $url
            bytes = $item.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -Path $file).Hash.ToLowerInvariant()
        }
        if ($role -eq "graph") {
            # The manifest describes the gzip transfer and the installed graph
            # in one entry; both files must sit in the published directory.
            $installedName = $item.Name.Substring(0, $item.Name.Length - 3)
            $installedPath = Join-Path $item.DirectoryName $installedName
            if (-not (Test-Path $installedPath)) {
                throw "graph install file missing: $installedName"
            }
            $installedItem = Get-Item $installedPath
            if ($installedItem.Length -eq 0) { throw "graph install file is empty: $installedName" }
            $entry.installed_name = $installedItem.Name
            $entry.installed_bytes = $installedItem.Length
            $entry.installed_sha256 = (Get-FileHash -Algorithm SHA256 -Path $installedPath).Hash.ToLowerInvariant()
        }
        $entries += $entry
    }

    $document = [ordered]@{
        id = $Region.id
        name = $Region.name
        version = $Version
        built_at = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        bbox = @($Region.bbox)
        minzoom = $Region.minzoom
        maxzoom = $Region.maxzoom
        files = @($entries)
        attribution = "$([char]0x00A9) OpenStreetMap contributors"
        license = "ODbL-1.0"
        license_url = "https://opendatacommons.org/licenses/odbl/1-0/"
    }

    # build.ps1 always supplies the actual source bytes; a manifest without
    # them is accepted for small local fixtures only.
    if (-not [string]::IsNullOrWhiteSpace($SourceSha256)) {
        if ($SourceSha256 -notmatch '^[A-Fa-f0-9]{64}$') {
            throw "source sha256 must be 64 hex characters: $SourceSha256"
        }
        if ($SourceBytes -le 0) { throw "source bytes must be positive" }
        if ([string]::IsNullOrWhiteSpace($SourceUrl)) { throw "source url is missing" }
        if ($DownloadedAt -notmatch '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$') {
            throw "downloaded_at must be UTC yyyy-MM-ddTHH:mm:ssZ: $DownloadedAt"
        }
        $document.source_url = $SourceUrl
        $document.source_sha256 = $SourceSha256.ToLowerInvariant()
        $document.source_bytes = $SourceBytes
        $document.downloaded_at = $DownloadedAt
    }

    $parent = Split-Path -Parent $Output
    if ($parent -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    $part = "$Output.part"
    Remove-Item $part -Force -ErrorAction SilentlyContinue
    try {
        $json = $document | ConvertTo-Json -Depth 6
        [System.IO.File]::WriteAllText($part, $json, (New-Object System.Text.UTF8Encoding($false)))
        Move-Item -Path $part -Destination $Output -Force
        return $Output
    } catch {
        Remove-Item $part -Force -ErrorAction SilentlyContinue
        throw
    }
}

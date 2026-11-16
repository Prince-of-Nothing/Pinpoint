function New-OfflineManifest {
    param(
        [Parameter(Mandatory = $true)]$Region,
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][string[]]$Files,
        [Parameter(Mandatory = $true)][string]$Output,
        [string]$BaseUrl
    )

    $ErrorActionPreference = "Stop"
    if ([string]::IsNullOrWhiteSpace($Region.id)) { throw "region id is missing" }
    if ([string]::IsNullOrWhiteSpace($Version)) { throw "version is missing" }

    $entries = @()
    foreach ($file in $Files) {
        if (-not (Test-Path $file)) { throw "manifest file missing: $file" }
        $item = Get-Item $file
        if ($item.Length -eq 0) { throw "manifest file is empty: $file" }
        $url = if ([string]::IsNullOrWhiteSpace($BaseUrl)) {
            ""
        } else {
            "$($BaseUrl.TrimEnd('/'))/$($item.Name)"
        }
        $entries += [ordered]@{
            name = $item.Name
            url = $url
            bytes = $item.Length
            sha256 = (Get-FileHash -Algorithm SHA256 -Path $file).Hash.ToLowerInvariant()
        }
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

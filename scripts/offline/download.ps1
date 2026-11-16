function Get-OsmExtract {
    param(
        [Parameter(Mandatory = $true)]$Region,
        [Parameter(Mandatory = $true)][string]$Destination,
        [string]$SourcePath,
        [string]$ExpectedSha256
    )

    $ErrorActionPreference = "Stop"
    $part = "$Destination.part"

    $parent = Split-Path -Parent $Destination
    if ($parent -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }

    if (Test-Path $Destination) {
        if ([string]::IsNullOrWhiteSpace($ExpectedSha256)) {
            return $Destination
        }
        $existing = (Get-FileHash -Algorithm SHA256 -Path $Destination).Hash
        if ($existing -eq $ExpectedSha256.ToUpperInvariant()) {
            return $Destination
        }
    }

    Remove-Item $part -Force -ErrorAction SilentlyContinue
    try {
        if ($SourcePath) {
            if (-not (Test-Path $SourcePath)) { throw "source not found: $SourcePath" }
            Copy-Item -Path $SourcePath -Destination $part -Force
        } else {
            if (-not $Region.geofabrik) { throw "region has no geofabrik URL" }
            $curlPath = $null
            $curlExe = Get-Command curl.exe -CommandType Application -ErrorAction SilentlyContinue
            if ($curlExe) {
                $curlPath = $curlExe.Source
            } else {
                $curl = Get-Command curl -CommandType Application -ErrorAction SilentlyContinue
                if ($curl) { $curlPath = $curl.Source }
            }
            if (-not $curlPath) { throw "curl not found; install curl" }
            & $curlPath -4 -L --fail --retry 3 --retry-delay 2 -o $part $Region.geofabrik
            if ($LASTEXITCODE -ne 0) { throw "download failed for $($Region.geofabrik)" }
        }

        if ((Get-Item $part).Length -eq 0) { throw "downloaded file is empty: $Destination" }

        if ($ExpectedSha256) {
            $actual = (Get-FileHash -Algorithm SHA256 -Path $part).Hash
            if ($actual -ne $ExpectedSha256.ToUpperInvariant()) {
                throw "SHA-256 mismatch for $Destination"
            }
        }

        Move-Item -Path $part -Destination $Destination -Force
        return $Destination
    } catch {
        Remove-Item $part -Force -ErrorAction SilentlyContinue
        throw
    }
}

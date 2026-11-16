function Invoke-NativeCommand {
    param([Parameter(Mandatory = $true)][scriptblock]$Command)

    # Windows PowerShell 5.1 turns native stderr into terminating errors when
    # $ErrorActionPreference is Stop. Tool progress output (cargo, docker, curl)
    # goes to stderr, so run natives with Continue and inspect $LASTEXITCODE.
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Command | Out-Host
        return $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previous
    }
}

function Get-CurlExecutable {
    $curlExe = Get-Command curl.exe -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($curlExe) { return $curlExe.Source }
    $curl = Get-Command curl -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($curl) { return $curl.Source }
    return $null
}

function Get-OsmExtract {
    param(
        [Parameter(Mandatory = $true)]$Region,
        [Parameter(Mandatory = $true)][string]$Destination,
        [string]$SourcePath,
        [string]$ExpectedSha256
    )

    $ErrorActionPreference = "Stop"
    $part = "$Destination.part"
    $expected = $null
    if (-not [string]::IsNullOrWhiteSpace($ExpectedSha256)) {
        $expected = $ExpectedSha256.Trim().ToUpperInvariant()
    }

    $parent = Split-Path -Parent $Destination
    if ($parent -and -not (Test-Path $parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }

    # Reuse only a destination whose bytes match the supplied hash. A stale or
    # mismatching file is removed and rebuilt; an unhashed destination is redone.
    if (Test-Path $Destination) {
        if ($expected) {
            $existing = (Get-FileHash -Algorithm SHA256 -Path $Destination).Hash
            if ($existing -eq $expected -and (Get-Item $Destination).Length -gt 0) { return $Destination }
        }
        Remove-Item $Destination -Force
    }

    Remove-Item $part -Force -ErrorAction SilentlyContinue
    try {
        if ($SourcePath) {
            if (-not (Test-Path $SourcePath)) { throw "source not found: $SourcePath" }
            Copy-Item -Path $SourcePath -Destination $part -Force
        } else {
            if (-not $Region.geofabrik) { throw "region has no geofabrik URL" }
            $curlPath = Get-CurlExecutable
            if (-not $curlPath) { throw "curl not found; install curl (curl.exe on Windows)" }
            $curlArgs = @(
                "-L", "--fail", "--retry", "3", "--retry-delay", "2",
                "--connect-timeout", "30", "--max-time", "3600",
                "-o", $part, $Region.geofabrik
            )
            $code = Invoke-NativeCommand { & $curlPath @curlArgs -4 }
            if ($code -ne 0) {
                Remove-Item $part -Force -ErrorAction SilentlyContinue
                Write-Warning "IPv4 download failed (curl exit $code); retrying without -4"
                $code = Invoke-NativeCommand { & $curlPath @curlArgs }
                if ($code -ne 0) { throw "download failed for $($Region.geofabrik)" }
            }
        }

        if ((Get-Item $part).Length -eq 0) { throw "downloaded file is empty: $Destination" }

        if ($expected) {
            $actual = (Get-FileHash -Algorithm SHA256 -Path $part).Hash
            if ($actual -ne $expected) {
                throw "SHA-256 mismatch for $Destination (expected $ExpectedSha256, got $($actual.ToLowerInvariant()))"
            }
        }

        Move-Item -Path $part -Destination $Destination -Force
        return $Destination
    } catch {
        Remove-Item $part -Force -ErrorAction SilentlyContinue
        throw
    }
}

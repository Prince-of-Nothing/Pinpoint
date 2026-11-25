# scripts/check-release.ps1 - fail if the version string drifts between
# Cargo.toml, Android/iOS metadata, the web cache marker, a running /health,
# and an APK built from this tree.
#
# Usage:
#   pwsh -File scripts/check-release.ps1 -SkipHealth -SkipApk   # CI metadata
#   pwsh -File scripts/check-release.ps1 -Base http://127.0.0.1:8080 `
#       -Apk .\Pinpoint-android-3.0.0.apk
#
# Exit code = number of failed checks (0 = all green).

param(
    [string]$Apk = "",
    [string]$Aapt = "",
    [string]$Base = "http://127.0.0.1:8080",
    [switch]$SkipHealth,
    [switch]$SkipApk
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$script:fail = 0

function Pass($Message) { Write-Host "  PASS  $Message" }
function Fail($Message) { Write-Host "  FAIL  $Message"; $script:fail += 1 }
function Skip($Message) { Write-Host "  SKIP  $Message" }

# ---------------------------------------------------------------------------
# Cargo version and the Android versionCode formula
# ---------------------------------------------------------------------------
$cargo = Get-Content Cargo.toml -Raw
$match = [regex]::Match($cargo, '(?m)^version\s*=\s*"([^"]+)"')
if (-not $match.Success) {
    Fail "Cargo.toml has no version"
    exit 1
}
$version = $match.Groups[1].Value
$parts = ($version -split "-")[0].Split(".")
$major = if ($parts.Count -gt 0) { [int]$parts[0] } else { 0 }
$minor = if ($parts.Count -gt 1) { [int]$parts[1] } else { 0 }
$patch = if ($parts.Count -gt 2) { [int]$parts[2] } else { 0 }
$expectedCode = $major * 10000 + $minor * 100 + $patch
Pass "Cargo.toml version = $version (Android versionCode $expectedCode)"

# ---------------------------------------------------------------------------
# Android reads Cargo and derives versionName/versionCode
# ---------------------------------------------------------------------------
$gradle = Get-Content android/app/build.gradle.kts -Raw
if ($gradle -match 'val appVersion = Regex\("[^"]*version' -and
    $gradle -match '\*\s*10_000' -and
    $gradle -match 'versionCode = appVersionCode' -and
    $gradle -match 'versionName = appVersion') {
    Pass "Gradle derives versionName/versionCode from Cargo.toml"
} else {
    Fail "Gradle no longer derives both version fields from Cargo.toml"
}

# ---------------------------------------------------------------------------
# iOS metadata must track the same version
# ---------------------------------------------------------------------------
$plist = Get-Content ios/Info.plist -Raw
$shortMatch = [regex]::Match(
    $plist,
    '<key>CFBundleShortVersionString</key>\s*<string>([^<]+)</string>'
)
$buildMatch = [regex]::Match(
    $plist,
    '<key>CFBundleVersion</key>\s*<string>([^<]+)</string>'
)
if ($shortMatch.Success -and $shortMatch.Groups[1].Value -eq $version) {
    Pass "iOS CFBundleShortVersionString = $version"
} else {
    Fail "iOS CFBundleShortVersionString does not match $version"
}
if ($buildMatch.Success -and $buildMatch.Groups[1].Value -eq "$expectedCode") {
    Pass "iOS CFBundleVersion = $expectedCode"
} else {
    Fail "iOS CFBundleVersion does not match $expectedCode"
}

# ---------------------------------------------------------------------------
# Web cache marker (bump on shell changes, not tied to semver)
# ---------------------------------------------------------------------------
if ((Get-Content web/sw.js -Raw) -match 'const VERSION = "pinpoint-offline-v\d+";') {
    Pass "web service-worker cache marker is present"
} else {
    Fail "web/sw.js has no versioned pinpoint-offline cache marker"
}

# The privacy page is served to users and names the release, so keep it in sync.
if ((Get-Content web/privacy.html -Raw) -match "Version $([regex]::Escape($version))") {
    Pass "web privacy page names $version"
} else {
    Fail "web/privacy.html does not name version $version"
}

# ---------------------------------------------------------------------------
# Running server /health
# ---------------------------------------------------------------------------
if ($SkipHealth) {
    Skip "server /health and /privacy (skipped)"
} else {
    try {
        $health = Invoke-RestMethod -Uri "$Base/health" -TimeoutSec 5
        if ($health.version -eq $version) {
            Pass "/health version = $version"
        } else {
            Fail "/health version '$($health.version)' != $version"
        }
        $privacy = (Invoke-WebRequest -Uri "$Base/privacy" -TimeoutSec 5 -UseBasicParsing).Content
        if ($privacy -match "Version $([regex]::Escape($version))") {
            Pass "/privacy names $version"
        } else {
            Fail "/privacy does not name $version"
        }
    } catch {
        Fail "could not read $Base/health or /privacy ($($_.Exception.Message))"
    }
}

# ---------------------------------------------------------------------------
# APK metadata via aapt
# ---------------------------------------------------------------------------
if ($SkipApk -or [string]::IsNullOrWhiteSpace($Apk)) {
    Skip "APK metadata (no -Apk)"
} elseif (-not (Test-Path $Apk)) {
    Fail "APK not found: $Apk"
} else {
    if (-not $Aapt) {
        $command = Get-Command aapt -ErrorAction SilentlyContinue
        if ($command) {
            $Aapt = $command.Source
        } else {
            $sdk = $env:ANDROID_HOME
            if (-not $sdk) { $sdk = $env:ANDROID_SDK_ROOT }
            if (-not $sdk -and $env:LOCALAPPDATA) {
                $sdk = Join-Path $env:LOCALAPPDATA "Android/Sdk"
            }
            if ($sdk) {
                $Aapt = Get-ChildItem "$sdk/build-tools" -Directory |
                    Sort-Object Name -Descending |
                    ForEach-Object {
                        Join-Path $_.FullName "aapt.exe"
                        Join-Path $_.FullName "aapt"
                    } |
                    Where-Object { Test-Path $_ } |
                    Select-Object -First 1
            }
        }
    }
    if (-not $Aapt) {
        Fail "aapt not found; pass -Aapt or set ANDROID_HOME"
    } else {
        $badging = (& $Aapt dump badging $Apk 2>&1 | Out-String)
        $badge = [regex]::Match($badging, "versionCode='(\d+)'\s+versionName='([^']+)'")
        if (-not $badge.Success) {
            Fail "aapt could not read versionCode/versionName"
        } else {
            $apkCode = [int]$badge.Groups[1].Value
            $apkName = $badge.Groups[2].Value
            if ($apkName -eq $version) {
                Pass "APK versionName = $version"
            } else {
                Fail "APK versionName '$apkName' != $version"
            }
            if ($apkCode -eq $expectedCode) {
                Pass "APK versionCode = $expectedCode"
            } else {
                Fail "APK versionCode '$apkCode' != $expectedCode"
            }
            $nativeLine = ($badging -split "`n" |
                Where-Object { $_ -match '^native-code:' }) -join ' '
            $abis = [regex]::Matches($nativeLine, "'([^']+)'") |
                ForEach-Object { $_.Groups[1].Value }
            if ($abis.Count -gt 0) {
                Pass "APK native-code = $($abis -join ', ')"
            }
        }
    }
}

Write-Host ""
if ($script:fail -eq 0) {
    Write-Host "release check passed for $version"
} else {
    Write-Host "$($script:fail) release check(s) failed"
}
exit $script:fail

# scripts/android-smoke.ps1 - install, launch and render-check the Android APK.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/android-smoke.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/android-smoke.ps1 -Serial emulator-5554
#
# Picks the first connected device when -Serial is omitted. Exit code 0 means
# the APK installed, a focused Pinpoint window rendered non-blank pixels, and
# its logcat has no FATAL, ANR or Rust panic.

param(
    [string]$Serial = "",
    [string]$Apk = "android/app/build/outputs/apk/debug/app-debug.apk",
    [int]$WaitSeconds = 30
)

$ErrorActionPreference = "Stop"

$adb = Join-Path $env:LOCALAPPDATA "Android\Sdk\platform-tools\adb.exe"
if (-not (Test-Path $adb)) {
    throw "adb not found at $adb"
}
if (-not (Test-Path $Apk)) {
    throw "APK not found at $Apk (run android\gradlew.bat assembleDebug first)"
}

if (-not $Serial) {
    $device = (& $adb devices) | Select-String -Pattern '^\S+\s+device$' | Select-Object -First 1
    if (-not $device) {
        throw "No connected Android device or emulator."
    }
    $Serial = ($device.Line -split '\s+')[0]
}

& $adb -s $Serial install -r $Apk | Out-Host
if ($LASTEXITCODE -ne 0) {
    throw "adb install failed for $Apk"
}

& $adb -s $Serial shell am force-stop com.pinpoint.desktop | Out-Null
& $adb -s $Serial logcat -c | Out-Null
& $adb -s $Serial shell am start -n com.pinpoint.desktop/.PinpointActivity | Out-Host

$appPid = ""
$pidDeadline = (Get-Date).AddSeconds(10)
do {
    Start-Sleep -Seconds 1
    $appPid = (& $adb -s $Serial shell pidof com.pinpoint.desktop).Trim()
} while (-not $appPid -and (Get-Date) -lt $pidDeadline)
if (-not $appPid) {
    throw "Pinpoint process is not running."
}

$shot = Join-Path $env:TEMP "pinpoint-android-smoke.png"
Add-Type -AssemblyName System.Drawing
$rendered = $false
$renderDeadline = (Get-Date).AddSeconds([Math]::Max($WaitSeconds, 30))
do {
    Start-Sleep -Seconds 2
    $focus = (& $adb -s $Serial shell "dumpsys window | grep mCurrentFocus") -join "`n"
    if ($focus -match 'Not Responding') {
        throw "Pinpoint is not responding: $focus"
    }
    if ($focus -notmatch 'com\.pinpoint\.desktop') {
        continue
    }
    & $adb -s $Serial shell screencap -p /sdcard/pinpoint-android-smoke.png | Out-Null
    & $adb -s $Serial pull /sdcard/pinpoint-android-smoke.png $shot | Out-Null
    $bitmap = [System.Drawing.Bitmap]::FromFile($shot)
    $nonBlack = 0
    $samples = 0
    for ($x = 0; $x -lt $bitmap.Width; $x += 24) {
        for ($y = 0; $y -lt $bitmap.Height; $y += 24) {
            $pixel = $bitmap.GetPixel($x, $y)
            if (($pixel.R + $pixel.G + $pixel.B) -gt 24) {
                $nonBlack++
            }
            $samples++
        }
    }
    $bitmap.Dispose()
    if ($nonBlack -ge [Math]::Max(10, $samples * 0.02)) {
        $rendered = $true
    }
} while (-not $rendered -and (Get-Date) -lt $renderDeadline)
if (-not $rendered) {
    throw "Screenshot is blank; Pinpoint has not rendered a focused screen."
}

$logs = & $adb -s $Serial logcat -d "--pid=$appPid"
$crash = $logs | Select-String -Pattern 'FATAL EXCEPTION|AndroidRuntime|panicked at'
if ($crash) {
    throw "Crash in logcat: $($crash -join '; ')"
}

$anr = & $adb -s $Serial logcat -d | Select-String -Pattern 'ANR in com\.pinpoint\.desktop'
if ($anr) {
    throw "ANR in logcat: $($anr -join '; ')"
}

Write-Host "PASS  ${Serial}: app drawn and focused (PID $appPid), no crash/ANR; screenshot $shot"

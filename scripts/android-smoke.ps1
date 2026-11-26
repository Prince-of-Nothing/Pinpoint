# scripts/android-smoke.ps1 - install, launch and crash-check the Android APK.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/android-smoke.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/android-smoke.ps1 -Serial emulator-5554
#
# Picks the first connected device when -Serial is omitted. Exit code 0 means
# the APK installed, PinpointActivity is focused and its logcat has no FATAL,
# AndroidRuntime or Rust panic.

param(
    [string]$Serial = "",
    [string]$Apk = "android/app/build/outputs/apk/debug/app-debug.apk",
    [int]$WaitSeconds = 8
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
Start-Sleep -Seconds $WaitSeconds

$focus = (& $adb -s $Serial shell "dumpsys window | grep mCurrentFocus") -join "`n"
if ($focus -notmatch 'com\.pinpoint\.desktop') {
    throw "PinpointActivity is not focused: $focus"
}

$appPid = (& $adb -s $Serial shell pidof com.pinpoint.desktop).Trim()
if (-not $appPid) {
    throw "Pinpoint process is not running."
}

$logs = & $adb -s $Serial logcat -d "--pid=$appPid"
$crash = $logs | Select-String -Pattern 'FATAL EXCEPTION|AndroidRuntime|panicked at'
if ($crash) {
    throw "Crash in logcat: $($crash -join '; ')"
}

$shot = Join-Path $env:TEMP "pinpoint-android-smoke.png"
& $adb -s $Serial shell screencap -p /sdcard/pinpoint-android-smoke.png | Out-Null
& $adb -s $Serial pull /sdcard/pinpoint-android-smoke.png $shot | Out-Null

Write-Host "PASS  ${Serial}: app focused (PID $appPid), no crash; screenshot $shot"
exit 0

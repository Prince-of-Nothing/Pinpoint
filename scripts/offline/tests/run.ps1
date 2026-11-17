# Runs the offline data-pipeline tests that need neither Docker nor network.
# e2e.ps1 is excluded on purpose: it builds the real Moldova pack with Docker and
# downloads the Geofabrik extract.
#
# Usage: pwsh -NoProfile -File scripts/offline/tests/run.ps1
#        powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/run.ps1

$ErrorActionPreference = "Stop"

$tests = @("build-dryrun", "download", "manifest", "verify", "atomic", "layout", "catalog")

function Get-PowerShellHost {
    foreach ($candidate in @(
            (Join-Path $PSHOME "pwsh.exe"),
            (Join-Path $PSHOME "pwsh"),
            (Join-Path $PSHOME "powershell.exe"))) {
        if (Test-Path $candidate) { return $candidate }
    }
    foreach ($name in @("pwsh", "powershell")) {
        $found = Get-Command $name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) { return $found.Source }
    }
    throw "no PowerShell host found to run the offline tests"
}

$psExe = Get-PowerShellHost
Write-Host "offline pipeline tests ($psExe)"
Write-Host ""

$passed = 0
foreach ($test in $tests) {
    $path = Join-Path $PSScriptRoot "$test.ps1"
    if (-not (Test-Path $path)) { throw "missing test script: $path" }

    $label = "[$($passed + 1)/$($tests.Count)] $test"
    Write-Host "$label ..."
    & $psExe -NoProfile -ExecutionPolicy Bypass -File $path
    if ($LASTEXITCODE -ne 0) {
        Write-Host "$label FAILED (exit $LASTEXITCODE)"
        exit 1
    }

    $passed++
    Write-Host "$label PASS"
    Write-Host ""
}

Write-Host "[skipped] e2e (needs Docker and network)"
Write-Host "all $($tests.Count) offline tests passed"
exit 0

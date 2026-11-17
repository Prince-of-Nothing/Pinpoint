# Pinpoint 2.0 release gate.
#
# Runs every check the v2 release depends on. The default is strict: a missing
# test file fails the gate, because a skipped suite must never look like a pass.
#
# Usage:
#   pwsh -NoProfile -File scripts/v2/check.ps1
#   pwsh -NoProfile -File scripts/v2/check.ps1 -Partial   # during integration
#   pwsh -NoProfile -File scripts/v2/check.ps1 -SkipRust -SkipOffline
#
# On Windows, run this from a shell where the MSVC linker is on PATH, or set
# PINPOINT_CARGO_PREFIX to a command prefix (for example a setup .bat call).

param(
    [switch]$Partial,
    [switch]$SkipRust,
    [switch]$SkipOffline,
    [switch]$SkipWeb,
    [switch]$SkipNativeRouter
)

$ErrorActionPreference = "Stop"
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$script:failed = @()
$script:skipped = @()
$script:passed = @()

function Write-Section([string]$Name) {
    Write-Host ""
    Write-Host "=== $Name ==="
}

function Invoke-GateStep {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [switch]$Optional
    )
    Write-Section $Name
    $started = Get-Date
    try {
        & $Action
        if ($LASTEXITCODE -ne 0) {
            throw "exit code $LASTEXITCODE"
        }
        $elapsed = (Get-Date) - $started
        Write-Host ("PASS  {0}  ({1:n1}s)" -f $Name, $elapsed.TotalSeconds)
        $script:passed += $Name
    } catch {
        $elapsed = (Get-Date) - $started
        if ($Optional) {
            Write-Warning ("SKIP  {0}  ({1})" -f $Name, $_.Exception.Message)
            $script:skipped += $Name
            return
        }
        Write-Host ("FAIL  {0}  ({1:n1}s): {2}" -f $Name, $elapsed.TotalSeconds, $_.Exception.Message) -ForegroundColor Red
        $script:failed += $Name
    }
}

function Invoke-Cargo {
    param([string]$WorkingDirectory, [string[]]$Arguments)
    Push-Location $WorkingDirectory
    try {
        if ($env:PINPOINT_CARGO_PREFIX) {
            cmd /c "$env:PINPOINT_CARGO_PREFIX && cargo $($Arguments -join ' ')"
        } else {
            & cargo @Arguments
        }
    } finally {
        Pop-Location
    }
}

function Assert-TestFile([string]$RelativePath) {
    $path = Join-Path $root $RelativePath
    if (-not (Test-Path $path)) {
        throw "required test is missing: $RelativePath"
    }
    return $path
}

function Invoke-NodeTest([string]$RelativePath) {
    $path = Assert-TestFile $RelativePath
    Push-Location $root
    try {
        & node $path
    } finally {
        Pop-Location
    }
}

if (-not $env:NODE_PATH) {
    $candidate = Join-Path $env:TEMP "opencode\node_modules"
    if (Test-Path $candidate) {
        $env:NODE_PATH = $candidate
    }
}

if (-not $SkipRust) {
    Invoke-GateStep "Rust format" { Invoke-Cargo $root @("fmt", "--all", "--", "--check") }
    Invoke-GateStep "Rust clippy" { Invoke-Cargo $root @("clippy", "--all-targets", "--", "-D", "warnings") }
    Invoke-GateStep "Rust build" { Invoke-Cargo $root @("build", "--locked") }
    Invoke-GateStep "Rust tests" { Invoke-Cargo $root @("test", "--locked") }
    Invoke-GateStep "Offline index feature tests" {
        Invoke-Cargo $root @("test", "--features", "offline-build", "--bin", "pinpoint-offline-index")
    }
}

if (-not $SkipOffline) {
    $runner = Assert-TestFile "scripts/offline/tests/run.ps1"
    # verify.ps1 resolves target/debug/pinpoint-offline-index.exe; the test
    # build above produces a test harness, not the runnable binary.
    Invoke-GateStep "Offline index binary" {
        Invoke-Cargo $root @("build", "--features", "offline-build", "--bin", "pinpoint-offline-index")
    }
    Invoke-GateStep "Offline pipeline tests" {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $runner
    }
}

if (-not $SkipNativeRouter) {
    $routerDir = Join-Path $root "tools/native-router"
    if (-not (Test-Path $routerDir)) {
        if ($Partial) {
            Write-Warning "SKIP  native router: tools/native-router is missing"
            $script:skipped += "Native router"
        } else {
            $script:failed += "Native router"
            Write-Host "FAIL  native router: tools/native-router is missing" -ForegroundColor Red
        }
    } else {
        Invoke-GateStep "Native router format" { Invoke-Cargo $routerDir @("fmt", "--all", "--", "--check") }
        Invoke-GateStep "Native router clippy" { Invoke-Cargo $routerDir @("clippy", "--all-targets", "--", "-D", "warnings") }
        Invoke-GateStep "Native router tests" { Invoke-Cargo $routerDir @("test", "--locked") }
    }
}

if (-not $SkipWeb) {
    $webTests = @(
        "web/map/test/smoke.mjs",
        "web/map/test/search-smoke.mjs",
        "web/app/test/search-ui.mjs",
        "web/app/test/offline-smoke.mjs",
        "web/app/test/route-ui.mjs"
    )
    foreach ($test in $webTests) {
        $path = Join-Path $root $test
        if (-not (Test-Path $path)) {
            if ($Partial) {
                Write-Warning "SKIP  $test (not present yet)"
                $script:skipped += $test
                continue
            }
            $script:failed += $test
            Write-Host "FAIL  $test is missing" -ForegroundColor Red
            continue
        }
        Invoke-GateStep $test { Invoke-NodeTest $test }
    }

    # Any additional v2 app smoke test must run too; the adapter streams add
    # files rather than editing this list.
    $extra = Get-ChildItem (Join-Path $root "web/app/test") -Filter "*.mjs" -File -ErrorAction SilentlyContinue |
        Where-Object { $webTests -notcontains "web/app/test/$($_.Name)" -and $_.Name -notmatch '^(helpers?|stub|fixture)' } |
        Sort-Object Name
    foreach ($file in $extra) {
        $relative = "web/app/test/$($file.Name)"
        Invoke-GateStep $relative { Invoke-NodeTest $relative }
    }
}

Write-Host ""
Write-Host "=== v2 gate summary ==="
Write-Host ("passed:  {0}" -f $script:passed.Count)
Write-Host ("skipped: {0}" -f $script:skipped.Count)
Write-Host ("failed:  {0}" -f $script:failed.Count)
if ($script:skipped.Count -gt 0) {
    $script:skipped | ForEach-Object { Write-Host "  skip  $_" -ForegroundColor Yellow }
}
if ($script:failed.Count -gt 0) {
    $script:failed | ForEach-Object { Write-Host "  fail  $_" -ForegroundColor Red }
    exit 1
}
Write-Host "v2 gate passed"

# scripts/commit.ps1 — Gate + commit helper (run after a feature is done).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/commit.ps1 -Message "[FEATURE] area: ..." -Body "details"
#
# Runs fmt/clippy/build/test and the /goal queue check, prepends a CHANGELOG
# entry stub, commits. Refuses to run on main. Does NOT push; open a PR from
# the feature branch.

param(
    [Parameter(Mandatory = $true)][string]$Message,
    [string]$Body = "",
    [switch]$SkipE2E
)

$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path))

$branch = (& git branch --show-current).Trim()
if ($branch -eq "main" -or $branch -eq "master") {
    throw "Refusing to commit on '$branch'. Create a feature branch first: git switch -c feat/<slug>"
}

function Gate($cmd) { Write-Host ">> $cmd"; Invoke-Expression $cmd; if ($LASTEXITCODE -ne 0) { throw "gate failed: $cmd" } }

Gate "cargo fmt --all -- --check"
Gate "cargo clippy --all-targets -- -D warnings"
Gate "cargo build --locked"
Gate "cargo test --locked"
Gate "powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 check"

# Append a one-line stub to CHANGELOG [Unreleased] if not already present.
$cl = "CHANGELOG.md"
if (Test-Path $cl) {
    $text = Get-Content $cl -Raw
    if ($text -notmatch [regex]::Escape($Message)) {
        $stamp = (Get-Date).ToString("yyyy-MM-dd")
        $stub = "- $Message ($stamp)"
        $text = $text -replace "(## \[Unreleased\]\r?\n)", "`$1### Added`n$stub`n"
        Set-Content -Path $cl -Value $text -NoNewline
        Write-Host "CHANGELOG stub added."
    }
}

$gitArgs = @("commit", "-m", $Message)
if ($Body) { $gitArgs += @("-m", $Body) }

git add -A
git commit @gitArgs
Write-Host "Committed: $Message"
Write-Host "Next: git push -u origin $branch, open/update the PR, wait for CI, then squash-merge."
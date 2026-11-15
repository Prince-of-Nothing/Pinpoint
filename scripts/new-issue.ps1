# scripts/new-issue.ps1 — create a triaged GitHub issue with the next type ID.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/new-issue.ps1 `
#     -Prefix FEAT -Type Feature -Title "Offline map packs" `
#     -Severity medium -Priority normal -Label android `
#     -Body "Download a bounded area for use without a connection."
#
#   -DryRun prints the computed ID and command without creating anything.
#
# Every issue gets a native type, one severity:* and one priority:* label and a
# PREFIX-n title. See docs/ISSUE_TAXONOMY.md.

param(
    [Parameter(Mandatory = $true)][ValidateSet("BUG", "FEAT", "INFRA", "DOCS", "SEC", "TASK")][string]$Prefix,
    [Parameter(Mandatory = $true)][ValidateSet("Bug", "Feature", "Task")][string]$Type,
    [Parameter(Mandatory = $true)][string]$Title,
    [Parameter(Mandatory = $true)][ValidateSet("critical", "important", "medium", "minor")][string]$Severity,
    [Parameter(Mandatory = $true)][ValidateSet("asap", "high", "normal", "low")][string]$Priority,
    [string[]]$Label = @(),
    [string]$Body = "",
    [string]$BodyFile = "",
    [string]$Repo = "Prince-of-Nothing/Pinpoint",
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$expectedType = switch ($Prefix) {
    "BUG" { "Bug" }
    "FEAT" { "Feature" }
    default { "Task" }
}
if ($Type -ne $expectedType) {
    throw "$Prefix issues must use type $expectedType (got $Type)."
}
if ([string]::IsNullOrWhiteSpace($Title)) {
    throw "Title must not be empty."
}

$json = & gh issue list --repo $Repo --state all --limit 500 --search "in:title $Prefix-" --json title 2>$null
if ($LASTEXITCODE -ne 0) {
    throw "gh issue list failed; check gh auth and the repository."
}

$max = 0
foreach ($issue in ($json | ConvertFrom-Json)) {
    if ($issue.title -match "^$([regex]::Escape($Prefix))-(\d+)\b") {
        $value = [int]$Matches[1]
        if ($value -gt $max) { $max = $value }
    }
}

$id = "$Prefix-$($max + 1)"
$fullTitle = "$id`: $Title"
$labels = @("severity:$Severity", "priority:$Priority") + $Label

$createArgs = @("issue", "create", "--repo", $Repo, "--type", $Type, "--title", $fullTitle)
foreach ($name in $labels) {
    $createArgs += @("--label", $name)
}
if ($BodyFile) {
    if (-not (Test-Path $BodyFile)) { throw "Body file not found: $BodyFile" }
    $createArgs += @("--body-file", (Resolve-Path $BodyFile).Path)
} elseif ($Body) {
    $createArgs += @("--body", $Body)
}

if ($DryRun) {
    Write-Host "Next ID: $id"
    Write-Host ("gh " + ($createArgs -join " "))
    return
}

& gh @createArgs
if ($LASTEXITCODE -ne 0) {
    throw "gh issue create failed."
}

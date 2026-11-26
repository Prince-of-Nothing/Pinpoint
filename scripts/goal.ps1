# scripts/goal.ps1 - claim, release and validate the /goal prompt queue.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 list
#   powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 claim -Id 03 -Agent agent-c
#   powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 release -Id 03 -Agent agent-c
#   powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 done -Id 03 -Agent agent-c
#   powershell -ExecutionPolicy Bypass -File scripts/goal.ps1 check

[CmdletBinding()]
param(
    [ValidateSet("list", "claim", "done", "release", "check")]
    [string]$Action = "list",
    [int]$Id = 0,
    [string]$Agent = ""
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$goal = Join-Path $root "goal"
$prompts = Join-Path $goal "prompts"
$claimed = Join-Path $goal "claimed"
$requiredSections = @("## Context", "## Task", "## Steps", "## Verify", "## Done")
$maxLines = 120
$maxQueue = 10

function Require-Agent {
    if ([string]::IsNullOrWhiteSpace($Agent) -or $Agent -notmatch '^[a-z0-9][a-z0-9-]{0,31}$') {
        throw "Provide -Agent <name> using lowercase letters, digits and dashes."
    }
}

function Find-Pending([int]$promptId) {
    $pattern = "{0:D2}-*.md" -f $promptId
    Get-ChildItem -Path $prompts -Filter $pattern -File -ErrorAction SilentlyContinue |
        Select-Object -First 1
}

function Find-Claimed([int]$promptId, [string]$agentName) {
    $dir = Join-Path $claimed $agentName
    if (-not (Test-Path $dir)) {
        return $null
    }
    $pattern = "{0:D2}-*.md" -f $promptId
    Get-ChildItem -Path $dir -Filter $pattern -File -ErrorAction SilentlyContinue |
        Select-Object -First 1
}

function Get-Claims {
    if (-not (Test-Path $claimed)) {
        return @()
    }
    Get-ChildItem -Path $claimed -Directory -ErrorAction SilentlyContinue |
        ForEach-Object {
            Get-ChildItem -Path $_.FullName -Filter "*.md" -File -ErrorAction SilentlyContinue
        }
}

function Test-PromptFile($file) {
    $errors = New-Object System.Collections.Generic.List[string]
    if ($file.Name -notmatch '^\d{2}-[a-z0-9-]+\.md$') {
        $errors.Add("bad name: $($file.Name)")
    }
    $lines = @(Get-Content -Path $file.FullName)
    if ($lines.Count -gt $maxLines) {
        $errors.Add("$($file.Name) has $($lines.Count) lines (max $maxLines)")
    }
    $text = $lines -join "`n"
    foreach ($section in $requiredSections) {
        if (-not $text.Contains($section)) {
            $errors.Add("$($file.Name) is missing $section")
        }
    }
    if ($text -notmatch 'goal/CONTEXT\.md') {
        $errors.Add("$($file.Name) does not point at goal/CONTEXT.md")
    }
    if ($text -notmatch 'scripts/goal\.ps1 done -Id') {
        $errors.Add("$($file.Name) does not end with the done command")
    }
    return $errors
}

function Invoke-Check {
    $errors = New-Object System.Collections.Generic.List[string]
    foreach ($required in @("README.md", "CONTEXT.md", "TEMPLATE.md")) {
        if (-not (Test-Path (Join-Path $goal $required))) {
            $errors.Add("missing goal/$required")
        }
    }

    $pending = @()
    if (Test-Path $prompts) {
        $pending = @(Get-ChildItem -Path $prompts -Filter "*.md" -File -ErrorAction SilentlyContinue)
    }
    $claims = @(Get-Claims)
    if (($pending.Count + $claims.Count) -gt $maxQueue) {
        $errors.Add("queue has $($pending.Count + $claims.Count) prompts; cap is $maxQueue")
    }
    foreach ($file in @($pending) + @($claims)) {
        foreach ($message in (Test-PromptFile $file)) {
            $errors.Add($message)
        }
    }

    $stray = @(Get-ChildItem -Path $goal -Filter "*.md" -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notin @("README.md", "CONTEXT.md", "TEMPLATE.md") })
    foreach ($file in $stray) {
        $errors.Add("prompt outside prompts/: $($file.Name)")
    }

    if ($errors.Count -gt 0) {
        $errors | ForEach-Object { Write-Host "FAIL  $_" }
        exit 1
    }
    Write-Host "PASS  goal queue: $($pending.Count) pending, $($claims.Count) claimed"
}

switch ($Action) {
    "list" {
        $pending = @(Get-ChildItem -Path $prompts -Filter "*.md" -File -ErrorAction SilentlyContinue | Sort-Object Name)
        $claims = @(Get-Claims | Sort-Object FullName)
        Write-Host "Pending: $($pending.Count)"
        foreach ($file in $pending) { Write-Host "  $($file.Name)" }
        Write-Host "Claimed: $($claims.Count)"
        foreach ($file in $claims) { Write-Host "  $($file.FullName.Substring($claimed.Length + 1))" }
    }
    "claim" {
        Require-Agent
        if ($Id -lt 1) { throw "Provide -Id <number>." }
        $prompt = Find-Pending $Id
        if (-not $prompt) { throw ("No pending prompt {0:D2}." -f $Id) }
        $targetDir = Join-Path $claimed $Agent
        New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
        $target = Join-Path $targetDir $prompt.Name
        if (Test-Path $target) { throw "Already claimed: $target" }
        Move-Item -Path $prompt.FullName -Destination $target -ErrorAction Stop
        Write-Host "CLAIMED  $($prompt.Name) -> $target"
        Write-Host "Next: read goal/CONTEXT.md and the claimed prompt; delete it with 'done' after the PR merges."
    }
    "release" {
        Require-Agent
        if ($Id -lt 1) { throw "Provide -Id <number>." }
        $claimedPrompt = Find-Claimed $Id $Agent
        if (-not $claimedPrompt) { throw ("No claimed prompt {0:D2} for {1}." -f $Id, $Agent) }
        $target = Join-Path $prompts $claimedPrompt.Name
        if (Test-Path $target) { throw "A pending prompt with that name already exists: $target" }
        Move-Item -Path $claimedPrompt.FullName -Destination $target -ErrorAction Stop
        Write-Host "RELEASED  $($claimedPrompt.Name) -> $target"
    }
    "done" {
        Require-Agent
        if ($Id -lt 1) { throw "Provide -Id <number>." }
        $claimedPrompt = Find-Claimed $Id $Agent
        if (-not $claimedPrompt) { throw ("No claimed prompt {0:D2} for {1}." -f $Id, $Agent) }
        Remove-Item -Path $claimedPrompt.FullName -Force -ErrorAction Stop
        Write-Host "DONE  removed $($claimedPrompt.Name)"
    }
    "check" {
        Invoke-Check
    }
}

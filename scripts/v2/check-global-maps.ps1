# Pinpoint global maps v1 check.
#
# Verifies the schema-1 region catalog, the Rust global-map tests, the web/map
# suites, the pinpoint-maps build workflow and the ADR/design docs. Read-only:
# it downloads no pack and contacts no network.
#
# Missing in-progress pieces (catalog, region_catalog module, workflow) are
# SKIP, so an unfinished slice never looks green. A present artifact that
# breaks the contract is FAIL.
#
# Usage:
#   pwsh -NoProfile -File scripts/v2/check-global-maps.ps1
#   pwsh -NoProfile -File scripts/v2/check-global-maps.ps1 -SkipRust
#   pwsh -NoProfile -File scripts/v2/check-global-maps.ps1 -SkipRust -SkipWeb
#
# -SkipRust skips the cargo tests. -SkipWeb skips the catalog and node suites.
# On Windows, run from a shell where the MSVC linker is on PATH, or set
# PINPOINT_CARGO_PREFIX to a command prefix (see scripts/v2/check.ps1).

param(
    [switch]$SkipRust,
    [switch]$SkipWeb
)

$ErrorActionPreference = "Stop"
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
# The data repo is a separate checkout; absent on machines that never built it.
$dataRepo = "C:\Users\Dinusauuuur\Documents\GitHub\pinpoint-maps"
$script:failed = @()
$script:skipped = @()
$script:passed = @()

function Write-Section([string]$Name) {
    Write-Host ""
    Write-Host "=== $Name ==="
}

function Add-Skip([string]$Name, [string]$Reason) {
    Write-Host ("SKIP  {0}  ({1})" -f $Name, $Reason) -ForegroundColor Yellow
    $script:skipped += $Name
}

function Invoke-GateStep {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        # Only steps that run a native program check $LASTEXITCODE; a pure
        # PowerShell step must not inherit a stale code from an earlier step.
        [switch]$Native
    )
    Write-Section $Name
    $started = Get-Date
    try {
        & $Action
        if ($Native -and $LASTEXITCODE -ne 0) {
            throw "exit code $LASTEXITCODE"
        }
        $elapsed = (Get-Date) - $started
        Write-Host ("PASS  {0}  ({1:n1}s)" -f $Name, $elapsed.TotalSeconds)
        $script:passed += $Name
    } catch {
        $elapsed = (Get-Date) - $started
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

function Invoke-NodeTest([string]$Path) {
    Push-Location $root
    try {
        & node $Path
    } finally {
        Pop-Location
    }
}

# A YAML parser is optional; a missing module writes a traceback to stderr, and
# PowerShell 5.1 turns that into a terminating error under
# $ErrorActionPreference = "Stop". Capture with Continue and drop stderr.
function Invoke-Captured([scriptblock]$Action) {
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        return (& $Action)
    } finally {
        $ErrorActionPreference = $previous
    }
}

function Test-FiniteNumber($Value) {
    if ($Value -is [int] -or $Value -is [long] -or $Value -is [double] -or
        $Value -is [decimal] -or $Value -is [single]) {
        $number = [double]$Value
        return -not ([double]::IsNaN($number) -or [double]::IsInfinity($number))
    }
    return $false
}

function Test-PositiveInteger($Value) {
    return (($Value -is [int] -or $Value -is [long]) -and $Value -gt 0)
}

function Test-CatalogFile([string]$Path) {
    try {
        $catalog = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    } catch {
        throw "not valid JSON: $($_.Exception.Message)"
    }

    $errors = New-Object System.Collections.Generic.List[string]
    if ($catalog.schema -ne 1) { $errors.Add("schema must be 1, got '$($catalog.schema)'") }
    if (-not ($catalog.updated -is [string]) -or -not $catalog.updated) {
        $errors.Add("updated must be a non-empty string")
    }

    $regions = @($catalog.regions | Where-Object { $null -ne $_ })
    if ($regions.Count -lt 1) { $errors.Add("regions must be a non-empty array") }
    if ($regions.Count -gt 64) { $errors.Add("at most 64 regions, got $($regions.Count)") }

    $seen = @{}
    $index = 0
    foreach ($region in $regions) {
        $index += 1
        $label = "regions[$index]"
        $id = [string]$region.id
        if ($id -notmatch '^[a-z0-9-]{1,64}$') {
            $errors.Add("$label id '$id' is not [a-z0-9-]{1,64}")
        } elseif ($seen.ContainsKey($id)) {
            $errors.Add("$label id '$id' is duplicated")
        } else {
            $seen[$id] = $true
        }

        $bbox = @($region.bbox | Where-Object { $null -ne $_ })
        if ($bbox.Count -ne 4 -or @($bbox | Where-Object { -not (Test-FiniteNumber $_) }).Count -gt 0) {
            $errors.Add("$label bbox must be 4 finite numbers")
        }

        $files = @($region.files | Where-Object { $null -ne $_ })
        if ($files.Count -lt 1) { $errors.Add("$label has no files") }
        if ($files.Count -gt 32) { $errors.Add("$label has $($files.Count) files, max 32") }

        $roles = @{}
        foreach ($file in $files) {
            $role = [string]$file.role
            if ($role -notin @("basemap", "graph")) {
                $errors.Add("$label file role '$role' is not basemap/graph")
            } else {
                $roles[$role] = $true
            }
            foreach ($nameField in @("transfer_name", "installed_name")) {
                $name = [string]$file.$nameField
                if (-not $name -or $name -match '[\\/]') {
                    $errors.Add("$label $role $nameField must be a plain file name")
                }
            }
            $url = [string]$file.url
            if ($url -notmatch '^https?://\S+$') {
                $errors.Add("$label $role url '$url' is not http(s)")
            }
            foreach ($sizeField in @("transfer_bytes", "installed_bytes")) {
                if (-not (Test-PositiveInteger $file.$sizeField)) {
                    $errors.Add("$label $role $sizeField must be a positive integer")
                }
            }
            foreach ($hashField in @("transfer_sha256", "installed_sha256")) {
                if ([string]$file.$hashField -cnotmatch '^[0-9a-f]{64}$') {
                    $errors.Add("$label $role $hashField must be 64 lowercase hex chars")
                }
            }
            if ([string]$file.compression -notin @("identity", "gzip")) {
                $errors.Add("$label $role compression must be identity or gzip")
            }
        }
        foreach ($needed in @("basemap", "graph")) {
            if (-not $roles.ContainsKey($needed)) { $errors.Add("$label has no $needed file") }
        }
    }

    if ($errors.Count -gt 0) {
        throw ("{0} problem(s): {1}" -f $errors.Count, (($errors | Select-Object -First 5) -join "; "))
    }
    Write-Host ("catalog ok: {0} region(s)" -f $regions.Count)
}

if (-not $env:NODE_PATH) {
    $candidate = Join-Path $env:TEMP "opencode\node_modules"
    if (Test-Path $candidate) {
        $env:NODE_PATH = $candidate
    }
}

if (-not $SkipWeb) {
    $catalogs = @(
        @{ Label = "web/catalog.json"; Path = (Join-Path $root "web/catalog.json") },
        @{ Label = "catalog.json"; Path = (Join-Path $root "catalog.json") },
        @{ Label = "pinpoint-maps/catalog.json"; Path = (Join-Path $dataRepo "catalog.json") }
    )
    $found = 0
    foreach ($catalog in $catalogs) {
        $catalogPath = $catalog.Path
        if (-not (Test-Path $catalogPath)) { continue }
        $found += 1
        Invoke-GateStep $catalog.Label { Test-CatalogFile $catalogPath }
    }
    if ($found -eq 0) {
        Add-Skip "catalog.json" "no schema-1 catalog in web/, the repo root or pinpoint-maps"
    }
}

if ($SkipRust) {
    Add-Skip "Rust global-map tests" "-SkipRust"
} else {
    foreach ($filter in @("region_catalog", "offline_install")) {
        $name = "Rust $filter tests"
        $sourceHits = @(Get-ChildItem (Join-Path $root "src") -Recurse -Filter "*.rs" -File |
            Select-String -Pattern $filter -SimpleMatch -List)
        if ($sourceHits.Count -eq 0) {
            Add-Skip $name "src/ has no $filter code yet"
            continue
        }
        $list = @()
        try {
            $list = @(Invoke-Cargo $root @("test", "--locked", "-j", "2", "--lib", $filter, "--", "--list"))
        } catch {
            $script:failed += $name
            Write-Host ("FAIL  {0}  ({1})" -f $name, $_.Exception.Message) -ForegroundColor Red
            continue
        }
        if ($LASTEXITCODE -ne 0) {
            $script:failed += $name
            Write-Host ("FAIL  {0}  (cargo test --list exited {1})" -f $name, $LASTEXITCODE) -ForegroundColor Red
            continue
        }
        if (@($list | Where-Object { $_ -match ': test$' }).Count -eq 0) {
            Add-Skip $name "no $filter tests in the lib yet"
            continue
        }
        Invoke-GateStep $name { Invoke-Cargo $root @("test", "--locked", "-j", "2", "--lib", $filter) } -Native
    }
}

if ($SkipWeb) {
    Add-Skip "web/map tests" "-SkipWeb"
} elseif (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    Add-Skip "web/map tests" "node is not installed"
} else {
    $smoke = Join-Path $root "web/map/test/smoke.mjs"
    if (Test-Path $smoke) {
        Invoke-GateStep "web/map/test/smoke.mjs" { Invoke-NodeTest $smoke } -Native
    } else {
        Add-Skip "web/map/test/smoke.mjs" "file is absent"
    }

    $catalogTests = @(Get-ChildItem (Join-Path $root "web/map/test") -Filter "*catalog*.mjs" -File -ErrorAction SilentlyContinue |
        Sort-Object Name)
    if ($catalogTests.Count -eq 0) {
        Add-Skip "web/map catalog test" "no catalog test in web/map/test yet"
    }
    foreach ($file in $catalogTests) {
        $testPath = $file.FullName
        Invoke-GateStep "web/map/test/$($file.Name)" { Invoke-NodeTest $testPath } -Native
    }
}

$workflow = Join-Path $dataRepo ".github/workflows/build-region.yml"
if (-not (Test-Path $dataRepo)) {
    Add-Skip "data repo build-region.yml" "pinpoint-maps is absent at $dataRepo"
} elseif (-not (Test-Path $workflow)) {
    Add-Skip "data repo build-region.yml" "no .github/workflows/build-region.yml yet"
} else {
    $workflowText = Get-Content -LiteralPath $workflow -Raw
    $facts = $null

    $python = Get-Command python -ErrorAction SilentlyContinue
    if ($python) {
        $pythonCode = @'
import json, sys
import yaml
with open(sys.argv[1], encoding='utf-8') as handle:
    doc = yaml.safe_load(handle)
triggers = doc.get(True) or doc.get('on') or {}
dispatch = triggers.get('workflow_dispatch') if isinstance(triggers, dict) else None
permissions = doc.get('permissions') or {}
print(json.dumps({
    'inputs': sorted(((dispatch or {}).get('inputs') or {}).keys()),
    'contents': permissions.get('contents') if isinstance(permissions, dict) else None,
    'jobs': sorted((doc.get('jobs') or {}).keys()),
}))
'@
        $output = Invoke-Captured { & python -c $pythonCode $workflow 2>$null }
        if ($LASTEXITCODE -eq 0 -and $output) {
            try { $facts = ($output | ConvertFrom-Json) } catch { $facts = $null }
        }
    }

    if (-not $facts -and (Get-Command node -ErrorAction SilentlyContinue)) {
        $nodeCode = @'
const fs = require('fs');
let parse;
try { parse = require('yaml').parse; } catch (error) { parse = require('js-yaml').load; }
const doc = parse(fs.readFileSync(process.argv[1], 'utf8'));
const triggers = doc[true] || doc.on || {};
const dispatch = triggers && triggers.workflow_dispatch ? triggers.workflow_dispatch : {};
const permissions = doc.permissions || {};
console.log(JSON.stringify({
  inputs: Object.keys((dispatch && dispatch.inputs) || {}).sort(),
  contents: permissions && permissions.contents ? permissions.contents : null,
  jobs: Object.keys(doc.jobs || {}).sort()
}));
'@
        $output = Invoke-Captured { & node -e $nodeCode $workflow 2>$null }
        if ($LASTEXITCODE -eq 0 -and $output) {
            try { $facts = ($output | ConvertFrom-Json) } catch { $facts = $null }
        }
    }

    if (-not $facts) {
        Add-Skip "build-region.yml parse" "no YAML parser could load it (python PyYAML or node yaml)"
    }
    $workflowStepName = "data repo build-region.yml"
    if (-not $facts) { $workflowStepName = "data repo build-region.yml (text checks)" }
    Invoke-GateStep $workflowStepName {
        $errors = New-Object System.Collections.Generic.List[string]
        if ($facts) {
            foreach ($inputName in @("region", "app_ref")) {
                if ($facts.inputs -notcontains $inputName) {
                    $errors.Add("workflow_dispatch is missing the '$inputName' input")
                }
            }
            if ($facts.contents -ne "write") {
                $errors.Add("permissions must grant contents: write")
            }
            if (@($facts.jobs).Count -eq 0) {
                $errors.Add("workflow has no jobs")
            }
        } else {
            if ($workflowText -notmatch '(?m)^\s*workflow_dispatch:') {
                $errors.Add("no workflow_dispatch trigger")
            }
            foreach ($inputName in @("region", "app_ref")) {
                if ($workflowText -notmatch "(?m)^\s+${inputName}:") {
                    $errors.Add("workflow_dispatch is missing the '$inputName' input")
                }
            }
            if ($workflowText -notmatch '(?m)^\s*contents:\s*write\s*$') {
                $errors.Add("permissions must grant contents: write")
            }
        }
        if (-not $workflowText.Contains("catalog.json")) {
            $errors.Add("workflow never regenerates catalog.json")
        }
        if ($workflowText -match '\$\{\{\s*secrets\.') {
            $errors.Add("workflow references secrets; the design allows none")
        }
        if ($errors.Count -gt 0) {
            throw (($errors | Select-Object -First 5) -join "; ")
        }
    }
}

$docs = @(
    @{ Label = "docs/adr/0021-global-map-catalog.md"; Path = (Join-Path $root "docs/adr/0021-global-map-catalog.md") },
    @{ Label = "docs/superpowers/specs/2026-11-17-global-maps-design.md"; Path = (Join-Path $root "docs/superpowers/specs/2026-11-17-global-maps-design.md") }
)
foreach ($doc in $docs) {
    $docPath = $doc.Path
    Invoke-GateStep $doc.Label {
        if (-not (Test-Path $docPath)) { throw "missing" }
        if ((Get-Item -LiteralPath $docPath).Length -eq 0) { throw "file is empty" }
    }
}

Write-Host ""
Write-Host "=== global maps v1 check summary ==="
Write-Host ("passed:  {0}" -f $script:passed.Count)
Write-Host ("skipped: {0}" -f $script:skipped.Count)
Write-Host ("failed:  {0}" -f $script:failed.Count)
if ($script:skipped.Count -gt 0) {
    $script:skipped | ForEach-Object { Write-Host "  skip  $_" -ForegroundColor Yellow }
}
if ($script:failed.Count -gt 0) {
    $script:failed | ForEach-Object { Write-Host "  fail  $_" -ForegroundColor Red }
    exit $script:failed.Count
}
Write-Host "global maps v1 check passed"
exit 0

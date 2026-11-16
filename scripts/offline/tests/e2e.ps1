$ErrorActionPreference = "Stop"
& "$PSScriptRoot/../build.ps1" -Region moldova
if ($LASTEXITCODE -ne 0) { throw "build failed" }
& "$PSScriptRoot/../verify.ps1" -PackDir "$PSScriptRoot/../../../dist/offline/moldova"
if ($LASTEXITCODE -ne 0) { throw "verify failed" }
Write-Host "e2e ok"

# Docker Desktop WSL disk maintenance.
#
# Rust Docker builds accumulate BuildKit cache quickly. Pruning removes the
# cache inside the WSL filesystem, but Windows does not shrink docker_data.vhdx
# automatically. This script prunes old cache, stops Docker/WSL, compacts the
# disk and starts Docker again.
#
# Usage (Administrator PowerShell):
#   ./scripts/docker-maintenance.ps1
#   ./scripts/docker-maintenance.ps1 -KeepCacheGB 2
#
# Run from the repository root. Existing images, containers and volumes are
# kept; only BuildKit cache beyond the requested size is removed.

param(
    [int]$KeepCacheGB = 5
)

$ErrorActionPreference = "Stop"

function Require-Command($Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found."
    }
}

Require-Command docker

Write-Host "Docker disk before:"
docker system df

Write-Host ""
Write-Host "Pruning build cache, keeping the newest $KeepCacheGB GB..."
docker builder prune --keep-storage "${KeepCacheGB}GB" -f

$vhdx = Join-Path $env:LOCALAPPDATA "Docker\wsl\disk\docker_data.vhdx"
if (-not (Test-Path $vhdx)) {
    Write-Host "No Docker data disk found at $vhdx; nothing to compact."
    exit 0
}

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $isAdmin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Warning "Run this script as Administrator to compact the WSL disk."
    Write-Host "Build cache was pruned; the VHDX will stay at its current size until compacted."
    exit 1
}

Require-Command Optimize-VHD

$before = (Get-Item $vhdx).Length
Write-Host ""
Write-Host "Stopping Docker Desktop and WSL..."
docker desktop stop
wsl --shutdown
Start-Sleep -Seconds 3

Write-Host "Compacting $vhdx (this can take several minutes)..."
Optimize-VHD -Path $vhdx -Mode Full

$after = (Get-Item $vhdx).Length
$freed = [Math]::Round(($before - $after) / 1GB, 2)
Write-Host ("Compacted: {0:N2} GB -> {1:N2} GB (freed {2:N2} GB)" -f `
    ($before / 1GB), ($after / 1GB), $freed)

Write-Host "Starting Docker Desktop..."
docker desktop start

Write-Host ""
Write-Host "Docker disk after:"
docker system df

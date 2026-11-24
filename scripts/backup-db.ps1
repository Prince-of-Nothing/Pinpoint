# scripts/backup-db.ps1 - encrypted logical backup of the compose Postgres.
#
# Streams `pg_dump --format=custom` from the db container to a temporary file,
# encrypts it with AES-256-CBC + PBKDF2 (openssl), verifies a decrypt roundtrip
# against the dump hash, and only then deletes the plaintext temporary file.
# The passphrase comes from PINPOINT_BACKUP_PASSPHRASE and is never written to
# disk or the repository.
#
# Run:
#   $env:PINPOINT_BACKUP_PASSPHRASE = "a long passphrase kept outside the repo"
#   powershell -ExecutionPolicy Bypass -File scripts/backup-db.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/backup-db.ps1 -SelfTest
#
# Exit code 0 on success, non-zero on any failure (including a missing
# passphrase; this script never writes an unencrypted backup).

[CmdletBinding()]
param(
    [string]$ComposeFile = "docker-compose.yml",
    [string]$OutDir = "backups",
    [int]$Keep = 24,
    [string]$DbUser = "pinpoint",
    [string]$DbName = "pinpoint",
    [string]$Service = "db",
    [string]$OpenSsl = "",
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"
$script:passEnv = "PINPOINT_BACKUP_PASSPHRASE"

function Resolve-OpenSsl([string]$explicit) {
    if (-not [string]::IsNullOrWhiteSpace($explicit)) { return $explicit }
    $cmd = Get-Command openssl -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $git = Join-Path $env:ProgramFiles "Git\usr\bin\openssl.exe"
    if (Test-Path $git) { return $git }
    throw "openssl was not found; install it or pass -OpenSsl <path>"
}

function Protect-Backup([string]$openssl, [string]$inputPath, [string]$outputPath) {
    & $openssl enc -aes-256-cbc -pbkdf2 -iter 200000 -salt `
        -in $inputPath -out $outputPath -pass "env:$script:passEnv"
    if ($LASTEXITCODE -ne 0) { throw "openssl encryption failed (exit $LASTEXITCODE)" }
}

function Unprotect-Backup([string]$openssl, [string]$inputPath, [string]$outputPath) {
    & $openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 -salt `
        -in $inputPath -out $outputPath -pass "env:$script:passEnv"
    if ($LASTEXITCODE -ne 0) { throw "openssl decryption failed (exit $LASTEXITCODE)" }
}

function Get-Sha256([string]$path) {
    return (Get-FileHash -Algorithm SHA256 -Path $path).Hash
}

function Assert-Roundtrip([string]$openssl, [string]$plainPath, [string]$encryptedPath) {
    $verify = "$plainPath.verify"
    try {
        Unprotect-Backup $openssl $encryptedPath $verify
        if ((Get-Sha256 $plainPath) -ne (Get-Sha256 $verify)) {
            throw "backup verification failed: decrypted hash does not match the dump"
        }
    } finally {
        Remove-Item $verify -Force -ErrorAction SilentlyContinue
    }
}

function New-SelfTestBytes {
    $bytes = New-Object byte[] 8192
    (New-Object System.Random 42).NextBytes($bytes)
    return , $bytes
}

function Invoke-SelfTest([string]$openssl) {
    $restore = $null
    if ([string]::IsNullOrWhiteSpace($env:PINPOINT_BACKUP_PASSPHRASE)) {
        $restore = $true
        $env:PINPOINT_BACKUP_PASSPHRASE = [guid]::NewGuid().ToString("N")
    }
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) ("pinpoint-backup-selftest-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $dir | Out-Null
    try {
        $plain = Join-Path $dir "sample.dump"
        [System.IO.File]::WriteAllBytes($plain, (New-SelfTestBytes))
        $encrypted = "$plain.enc"
        Protect-Backup $openssl $plain $encrypted
        Assert-Roundtrip $openssl $plain $encrypted
        $size = (Get-Item $encrypted).Length
        Write-Host "SELFTEST OK  encrypted=$size bytes  roundtrip=identical"
    } finally {
        Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
        if ($restore) { Remove-Item Env:PINPOINT_BACKUP_PASSPHRASE -ErrorAction SilentlyContinue }
    }
}

$opensslPath = Resolve-OpenSsl $OpenSsl

if ($SelfTest) {
    Invoke-SelfTest $opensslPath
    exit 0
}

if ([string]::IsNullOrWhiteSpace($env:PINPOINT_BACKUP_PASSPHRASE)) {
    throw "Set $($script:passEnv) for this process; the script refuses to write a plaintext backup."
}
if (-not (Test-Path -LiteralPath $ComposeFile)) {
    throw "compose file not found: $ComposeFile"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$stamp = Get-Date -Format "yyyyMMddTHHmmss"
$temp = Join-Path ([System.IO.Path]::GetTempPath()) "pinpoint-$stamp.dump"
$encrypted = Join-Path $OutDir "pinpoint-$stamp.dump.enc"
try {
    $command = "compose -f `"$ComposeFile`" exec -T $Service pg_dump -U $DbUser -d $DbName --format=custom > `"$temp`""
    & cmd.exe /c $command
    if ($LASTEXITCODE -ne 0) {
        throw "pg_dump failed (exit $LASTEXITCODE); is the compose stack running?"
    }
    if (-not (Test-Path -LiteralPath $temp) -or (Get-Item $temp).Length -eq 0) {
        throw "pg_dump produced no output"
    }
    Protect-Backup $opensslPath $temp $encrypted
    Assert-Roundtrip $opensslPath $temp $encrypted
    $kb = [math]::Round((Get-Item $encrypted).Length / 1KB, 1)
    Write-Host "backup ok: $encrypted ($kb KB)"
    Get-ChildItem -Path $OutDir -Filter "pinpoint-*.dump.enc" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -Skip $Keep |
        Remove-Item -Force
} finally {
    Remove-Item $temp -Force -ErrorAction SilentlyContinue
}

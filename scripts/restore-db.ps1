# scripts/restore-db.ps1 - restore an encrypted backup into the compose Postgres.
#
# Decrypts a `pinpoint-*.dump.enc` produced by scripts/backup-db.ps1, checks the
# pg_dump custom-format header, and runs `pg_restore --clean --if-exists
# --no-owner` inside the db container. The restore REPLACES the current data,
# so it requires -ConfirmRestore.
#
# Run:
#   $env:PINPOINT_BACKUP_PASSPHRASE = "the same passphrase used for the backup"
#   powershell -ExecutionPolicy Bypass -File scripts/restore-db.ps1 `
#     -File backups/pinpoint-20260101T120000.dump.enc -ConfirmRestore
#
# Exit codes: 0 success, 2 refused without -ConfirmRestore, non-zero on failure.

[CmdletBinding()]
param(
    [string]$File = "",
    [string]$ComposeFile = "docker-compose.yml",
    [string]$DbUser = "pinpoint",
    [string]$DbName = "pinpoint",
    [string]$Service = "db",
    [string]$OpenSsl = "",
    [switch]$ConfirmRestore,
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

function Assert-PgDumpHeader([string]$path) {
    $stream = [System.IO.File]::OpenRead($path)
    try {
        $header = New-Object byte[] 5
        if ($stream.Read($header, 0, 5) -ne 5) { throw "decrypted file is too short to be a pg_dump archive" }
        $magic = [System.Text.Encoding]::ASCII.GetString($header)
        if ($magic -ne "PGDMP") {
            throw "decrypted file is not a pg_dump custom archive (header '$magic')"
        }
    } finally {
        $stream.Dispose()
    }
}

function Invoke-SelfTest([string]$openssl) {
    $restore = $null
    if ([string]::IsNullOrWhiteSpace($env:PINPOINT_BACKUP_PASSPHRASE)) {
        $restore = $true
        $env:PINPOINT_BACKUP_PASSPHRASE = [guid]::NewGuid().ToString("N")
    }
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) ("pinpoint-restore-selftest-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $dir | Out-Null
    try {
        $plain = Join-Path $dir "sample.dump"
        $bytes = [System.Text.Encoding]::ASCII.GetBytes("PGDMP" + ("x" * 4096))
        [System.IO.File]::WriteAllBytes($plain, $bytes)
        $encrypted = "$plain.enc"
        Protect-Backup $openssl $plain $encrypted
        $decrypted = "$plain.dec"
        Unprotect-Backup $openssl $encrypted $decrypted
        Assert-PgDumpHeader $decrypted
        if ((Get-FileHash $plain).Hash -ne (Get-FileHash $decrypted).Hash) {
            throw "restore self-test roundtrip hash mismatch"
        }
        Write-Host "SELFTEST OK  decrypt=identical  header=PGDMP"
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

if ([string]::IsNullOrWhiteSpace($File)) { throw "-File is required (the .dump.enc to restore)" }
if (-not (Test-Path -LiteralPath $File)) { throw "backup not found: $File" }
if ([string]::IsNullOrWhiteSpace($env:PINPOINT_BACKUP_PASSPHRASE)) {
    throw "Set $($script:passEnv) for this process (the passphrase used by the backup)."
}
if (-not $ConfirmRestore) {
    Write-Host "Refusing to replace $DbName without -ConfirmRestore."
    Write-Host "This would drop and recreate the objects in the current database."
    exit 2
}
if (-not (Test-Path -LiteralPath $ComposeFile)) { throw "compose file not found: $ComposeFile" }

$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("pinpoint-restore-" + [guid]::NewGuid().ToString("N") + ".dump")
try {
    Unprotect-Backup $opensslPath $File $temp
    Assert-PgDumpHeader $temp
    $command = "compose -f `"$ComposeFile`" exec -T $Service pg_restore -U $DbUser -d $DbName --clean --if-exists --no-owner < `"$temp`""
    & cmd.exe /c $command
    if ($LASTEXITCODE -ne 0) {
        throw "pg_restore failed (exit $LASTEXITCODE); the database may be partially restored, inspect the output"
    }
    Write-Host "restore ok: $File -> $DbName"
} finally {
    Remove-Item $temp -Force -ErrorAction SilentlyContinue
}

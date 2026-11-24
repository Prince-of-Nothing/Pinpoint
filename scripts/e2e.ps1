# scripts/e2e.ps1 — Full-feature automation for the Pinpoint stack.
#
# Tests every live functionality through the gateway (127.0.0.1:8000).
# Requires: docker compose stack up; node (via docker image) for TOTP codes.
# Run: powershell -ExecutionPolicy Bypass -File scripts/e2e.ps1
# Exit code = number of failed checks (0 = all green).
#
# NOTE: this script predates the session-cookie auth of the Rust server; its
# requests carry no cookie or CSRF token, so the authenticated checks return
# 401. scripts/smoke.ps1 is the cookie-aware suite for the single binary.

param([string]$Base = "http://127.0.0.1:8000")

$script:pass = 0
$script:fail = 0
$script:skip = 0

function Check($name, [scriptblock]$fn, $expect) {
    try {
        $r = & $fn
        $actual = if ($r -is [string]) { $r } else { ($r | ConvertTo-Json -Depth 3 -Compress) }
        if ("$actual" -eq "$expect") {
            $script:pass++; Write-Host "  PASS  $name"
        } else {
            $script:fail++; Write-Host "  FAIL  $name (expected '$expect', got '$actual')"
        }
    } catch {
        $msg = "$($_.Exception.Message)"; if ($_.ErrorDetails.Message) { $msg += " " + $_.ErrorDetails.Message }
        if ("$msg" -eq "$expect") {
            $script:pass++; Write-Host "  PASS  $name"
        } else {
            $script:fail++; Write-Host "  FAIL  $name (exception: $msg)"
        }
    }
}

function NodeTotp($secretBase64, [int]$offsetSeconds = 0) {
    $code = docker run --rm node:alpine node -e @"
const c=require('crypto');
const k=Buffer.from('$secretBase64','base64');
const now=Math.floor(Date.now()/1000)+$offsetSeconds;
const m=Buffer.alloc(8);m.writeBigUInt64BE(BigInt(Math.floor(now/30)));
const h=c.createHmac('sha256',k).update(m).digest();
const o=h[31]&0xf;
console.log(String((h.readUInt32BE(o)&0x7fffffff)%1000000).padStart(6,'0'))
"@ 2>$null
    return "$code".Trim()
}

Write-Host "== Gateway & headers =="
Check "GET /health" { (Invoke-RestMethod "$Base/health").status } "healthy"
Check "GET /registry lists 4 services" { @( (Invoke-RestMethod "$Base/registry").services ).Count } "4"
Check "X-Upstream present (any-replica proof)" {
    $h = (Invoke-WebRequest -Uri "$Base/api/v1/users/" -UseBasicParsing).Headers
    ((($h['X-Upstream'] -join ',') -match ':\d+').ToString())
} "True"
Check "gateway headers" {
    $h = (Invoke-WebRequest -Uri "$Base/api/v1/users/" -UseBasicParsing).Headers
    $nosniff = ($h['X-Content-Type-Options'] -join ',') -match 'nosniff'
    $deny = ($h['X-Frame-Options'] -join ',') -match 'DENY'
    $csp = ($h['Content-Security-Policy'] -join ',') -match 'default-src'
    $ver = ($h['Server'] -join ',') -notmatch 'nginx/'
    "$nosniff|$deny|$csp|$ver"
} "True|True|True|True"

Write-Host "== Auth =="
# MFA disable now requires a current TOTP code (ADR-0022). A fresh database or
# an in-app disable is the only way to clear a previous run's enrolment.
$l = Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/login" -ContentType "application/json" -Body '{"email":"alice@example.com","password":"DemoPass123!"}'
Check "login alice (id=2)" { $l.user.id } "2"
Check "login wrong password 401" {
    try { Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/login" -ContentType "application/json" -Body '{"email":"alice@example.com","password":"wrong"}' | ConvertTo-Json -Compress } catch { $_.ErrorDetails.Message | ConvertFrom-Json | ForEach-Object { $_.error.code } }
} "INVALID_CREDENTIALS"
Check "me" { (Invoke-RestMethod "$Base/api/v1/auth/me").email } "alice@example.com"
Check "register dave (new or duplicate=exists)" {
    try {
        $r = Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/register" -ContentType "application/json" -Body '{"email":"dave@example.com","password":"DavePass123!","full_name":"Dave"}'
        $r.user.email
    } catch {
        # Duplicate rejection proves the account exists (cross-service
        # visibility: users service does not see auth-registered rows).
        if ($_.ErrorDetails.Message -match 'already registered') { "dave@example.com" } else { throw }
    }
} "dave@example.com"
Check "mfa enroll" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/mfa/enroll" -ContentType "application/json").secret.Length -gt 10 } "True"
$e = Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/mfa/enroll" -ContentType "application/json"
$code = NodeTotp $e.secret
Check "mfa verify (node code)" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/mfa/verify" -ContentType "application/json" -Body "{`"code`":`"$code`"}").mfa_enabled } "True"
Check "mfa status" { (Invoke-RestMethod "$Base/api/v1/auth/mfa/status").mfa_enabled } "True"
Check "login MFA_REQUIRED no-totp" {
    try { Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/login" -ContentType "application/json" -Body '{"email":"alice@example.com","password":"DemoPass123!"}' | ConvertTo-Json -Compress } catch { $_.ErrorDetails.Message | ConvertFrom-Json | ForEach-Object { $_.error.code } }
} "MFA_REQUIRED"
$code2 = NodeTotp $e.secret
Check "mfa replay rejected" {
    try { Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/mfa/verify" -ContentType "application/json" -Body "{`"code`":`"$code`"} " | ConvertTo-Json -Compress } catch { $_.ErrorDetails.Message | ConvertFrom-Json | ForEach-Object { $_.error.code } }
} "INVALID_MFA_CODE"
$code3 = NodeTotp $e.secret 30
Check "mfa disable needs a code" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/auth/mfa/disable" -ContentType "application/json" -Body "{`"code`":`"$code3`"}").mfa_enabled } "False"

Write-Host "== Users & friendships =="
Check "list users = seeded 3 (cross-service eventual)" {
    $raw = curl.exe -s "$Base/api/v1/users/"
    $has = @("admin@example.com", "alice@example.com", "bob@example.com") | ForEach-Object { $raw -match [regex]::Escape($_) }
    "$($has -join ',')"
} "True,True,True"
Check "get user 2 = Alice" { (Invoke-RestMethod "$Base/api/v1/users/2").email } "alice@example.com"
Check "get unknown user 400" {
    try { Invoke-RestMethod "$Base/api/v1/users/99999" | ConvertTo-Json -Compress } catch { ($_.Exception.Response.StatusCode.value__) }
} "400"
Check "search alice" { (Invoke-RestMethod "$Base/api/v1/users/search?q=alice").Count } "1"
Check "create user frank" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/users/" -ContentType "application/json" -Body '{"email":"frank@example.com","full_name":"Frank","role":"resident","password":"FrankPass123!"}').email } "frank@example.com"
$frank = (Invoke-RestMethod "$Base/api/v1/users/search?q=frank") | Select-Object -First 1
Check "patch user frank" { (Invoke-RestMethod -Method Patch -Uri "$Base/api/v1/users/$($frank.id)" -ContentType "application/json" -Body '{"full_name":"Frank Renamed"}' | ConvertTo-Json -Compress).Contains('Frank Renamed') } "True"
Check "delete user frank" { (Invoke-RestMethod -Method Delete -Uri "$Base/api/v1/users/$($frank.id)").message } "User deleted."
Check "friendships list has seeded accepted" { (@(Invoke-RestMethod "$Base/api/v1/friendships/").Count -ge 1).ToString() } "True"
$fs = Invoke-RestMethod -Method Post -Uri "$Base/api/v1/friendships/" -ContentType "application/json" -Body '{"receiver_email":"bob@example.com"}'
Check "send friendship bob" { $fs.status } "pending"
Check "respond accept" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/friendships/$($fs.id)/respond" -ContentType "application/json" -Body '{"action":"accept"}').status } "accepted"
Check "delete friendship" { (Invoke-RestMethod -Method Delete -Uri "$Base/api/v1/friendships/$($fs.id)").message } "Friendship removed."

Write-Host "== Locations, shares, check-ins, emergency =="
$seedFs = @(Invoke-RestMethod "$Base/api/v1/friendships/") | Where-Object { $_.status -eq "accepted" } | Select-Object -First 1
$s = Invoke-RestMethod -Method Post -Uri "$Base/api/v1/locations/shares" -ContentType "application/json" -Body "{`"friendship_id`":$($seedFs.id),`"direction`":`"mutual`",`"precision_level`":`"exact`",`"duration_hours`":24,`"label`":`"e2e`"}"
Check "create share (expiry+label)" { "$($s.is_active)/$($s.expires_at -ne $null)/$($s.label)" } "True/True/e2e"
Check "patch share pause" { (Invoke-RestMethod -Method Patch -Uri "$Base/api/v1/locations/shares/$($s.id)" -ContentType "application/json" -Body '{"is_active":false}').is_active } "False"
Check "patch share resume" { (Invoke-RestMethod -Method Patch -Uri "$Base/api/v1/locations/shares/$($s.id)" -ContentType "application/json" -Body '{"is_active":true}').is_active } "True"
Check "publish location" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/locations/me" -ContentType "application/json" -Body '{"latitude":41.8902,"longitude":-87.6323,"accuracy":10}').message } "Location updated."
Check "get my location" { (Invoke-RestMethod "$Base/api/v1/locations/me").precision_level } "exact"
Check "list friend locations >=1" { (@(Invoke-RestMethod "$Base/api/v1/locations/friends").Count -ge 1).ToString() } "True"
Check "create check-in" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/locations/check-ins" -ContentType "application/json" -Body '{"latitude":41.89,"longitude":-87.63,"note":"e2e","kind":"manual"}').kind } "manual"
Check "list check-ins >=1" { (@(Invoke-RestMethod "$Base/api/v1/locations/check-ins").Count -ge 1).ToString() } "True"
Check "trigger emergency" { (Invoke-RestMethod -Method Post -Uri "$Base/api/v1/locations/emergency" -ContentType "application/json" -Body '{"latitude":41.89,"longitude":-87.63}').resolved } "False"
Check "list emergencies >=1" { (@(Invoke-RestMethod "$Base/api/v1/locations/emergency").Count -ge 1).ToString() } "True"
Check "delete share" { (Invoke-RestMethod -Method Delete -Uri "$Base/api/v1/locations/shares/$($s.id)").message } "Share deleted."

Write-Host "== Map =="
Check "map health" { (Invoke-RestMethod "$Base/map/health").status } "healthy"
Check "map attribution" { (Invoke-RestMethod "$Base/map/attribution").data } "© OpenStreetMap contributors"
Check "tile proxy (200 keyed / 502 guidance keyless)" {
    $r = try { (Invoke-WebRequest -Uri "$Base/map/tiles/14/1000/5000" -UseBasicParsing).StatusCode } catch { [int]$_.Exception.Response.StatusCode }
    $ok = $r -in 200, 502
    [string]$ok
} "True"

Write-Host ""
Write-Host "RESULT: $script:pass passed, $script:fail failed, $script:skip skipped"
exit $script:fail

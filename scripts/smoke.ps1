# scripts/smoke.ps1 - end-to-end checks against a running pinpoint-server.
#
# Covers the validation and consent rules that are easy to regress: auth input
# rules, friendship rejection, location accuracy, share labels, notes,
# check-ins, the search bound and the offline region catalog. Every request is
# authenticated with a real
# session cookie + CSRF token, so the checks exercise the same path a client
# uses.
#
# Run:
#   $env:BIND_ADDR = "0.0.0.0:8080"
#   Remove-Item Env:DATABASE_URL -ErrorAction SilentlyContinue
#   .\target\debug\pinpoint-server.exe        # in-memory mode
#   powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1
#
# Exit code = number of failed checks (0 = all green).

param([string]$Base = "http://127.0.0.1:8080")

$ErrorActionPreference = "Stop"
$script:pass = 0
$script:fail = 0
$stamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()

function Check($Name, $Actual, $Expected) {
    if ("$Actual" -eq "$Expected") {
        $script:pass++
        Write-Host "  PASS  $Name"
    } else {
        $script:fail++
        Write-Host "  FAIL  $Name (expected '$Expected', got '$Actual')"
    }
}

function Json($Value) { $Value | ConvertTo-Json -Depth 6 -Compress }

# Public read without a session: the offline region catalog needs no login.
function Get-Public($Path) {
    try {
        $resp = Invoke-WebRequest -UseBasicParsing -Method Get -Uri "$Base$Path" -TimeoutSec 10
        @{ Status = [int]$resp.StatusCode; Body = ($resp.Content | ConvertFrom-Json) }
    } catch {
        $r = $_.Exception.Response
        @{ Status = $(if ($r) { [int]$r.StatusCode } else { 0 }); Body = $null }
    }
}

function New-Client($Email, $Password, $Name) {
    $body = Json @{ email = $Email; password = $Password; full_name = $Name }
    $resp = Invoke-WebRequest -UseBasicParsing -Method Post `
        -Uri "$Base/api/v1/auth/register" -ContentType "application/json; charset=utf-8" `
        -Body $body -SessionVariable session
    $json = $resp.Content | ConvertFrom-Json
    @{ Session = $session; Csrf = $json.csrf_token; User = $json.user }
}

function Api($Client, $Method, $Path, $Body) {
    $params = @{
        UseBasicParsing = $true
        Method          = $Method
        Uri             = "$Base$Path"
        WebSession      = $Client.Session
        Headers         = @{ "X-CSRF-Token" = $Client.Csrf }
        ContentType     = "application/json; charset=utf-8"
    }
    if ($null -ne $Body) { $params.Body = (Json $Body) }
    try {
        $resp = Invoke-WebRequest @params
        @{ Status = [int]$resp.StatusCode; Body = ($resp.Content | ConvertFrom-Json) }
    } catch {
        $r = $_.Exception.Response
        @{ Status = $(if ($r) { [int]$r.StatusCode } else { 0 }); Body = $null }
    }
}

function Raw-Register($Email, $Password, $Name) {
    try {
        $resp = Invoke-WebRequest -UseBasicParsing -Method Post `
            -Uri "$Base/api/v1/auth/register" -ContentType "application/json; charset=utf-8" `
            -Body (Json @{ email = $Email; password = $Password; full_name = $Name })
        [int]$resp.StatusCode
    } catch {
        $r = $_.Exception.Response
        $(if ($r) { [int]$r.StatusCode } else { 0 })
    }
}

$bidi = [char]0x202E
$name60 = [string]::new([char]0x00C4, 60)
$name101 = [string]::new([char]0x61, 101)
$password8 = [string]::new([char]0x00E9, 8)
$password7 = [string]::new([char]0x00E9, 7)

Write-Host "== Auth input rules =="
Check "bidi email rejected" (Raw-Register "bidi${bidi}user-$stamp@example.invalid" "StrongPass!2026" "Bidi User") 400
Check "unicode email accepted" (Raw-Register "caf$([char]0x00E9)-$stamp@example.invalid" "StrongPass!2026" "Cafe User") 201
Check "short password rejected" (Raw-Register "short-$stamp@example.invalid" "short" "Short Pass") 400
Check "bidi name rejected" (Raw-Register "name-$stamp@example.invalid" "StrongPass!2026" "Bad${bidi}Name") 400
Check "60-character multibyte name accepted" (Raw-Register "long-$stamp@example.invalid" "StrongPass!2026" $name60) 201
Check "101-character name rejected" (Raw-Register "longer-$stamp@example.invalid" "StrongPass!2026" $name101) 400
Check "8-character multibyte password accepted" (Raw-Register "mb8-$stamp@example.invalid" $password8 "Multibyte Eight") 201
Check "7-character multibyte password rejected" (Raw-Register "mb7-$stamp@example.invalid" $password7 "Multibyte Seven") 400

Write-Host "== Friendships =="
$alice = New-Client "alice-$stamp@example.invalid" "StrongPass!2026" "Alice"
$bob = New-Client "bob-$stamp@example.invalid" "StrongPass!2026" "Bob"

$request = Api $alice "POST" "/api/v1/friendships/" @{ receiver_username = $bob.User.username }
Check "friend request created" $request.Status 201
$friendId = $request.Body.id
Check "receiver rejects" (Api $bob "POST" "/api/v1/friendships/$friendId/respond" @{ action = "reject" }).Status 200
Check "rejected request cannot be accepted" (Api $bob "POST" "/api/v1/friendships/$friendId/respond" @{ action = "accept" }).Status 400
$resend = Api $alice "POST" "/api/v1/friendships/" @{ receiver_username = $bob.User.username }
Check "sender sends a fresh request" $resend.Status 201
Check "fresh request accepted" (Api $bob "POST" "/api/v1/friendships/$($resend.Body.id)/respond" @{ action = "accept" }).Status 200

Write-Host "== Location accuracy =="
Check "negative accuracy rejected" (Api $alice "POST" "/api/v1/locations/me" @{ latitude = 47.01; longitude = 28.86; accuracy = -1 }).Status 400
Check "absurd accuracy rejected" (Api $alice "POST" "/api/v1/locations/me" @{ latitude = 47.01; longitude = 28.86; accuracy = 10000001 }).Status 400
Check "plausible accuracy accepted" (Api $alice "POST" "/api/v1/locations/me" @{ latitude = 47.01; longitude = 28.86; accuracy = 10 }).Status 200

Write-Host "== Share labels =="
Check "bidi label rejected" (Api $alice "POST" "/api/v1/locations/shares" @{ friendship_id = $resend.Body.id; label = "Trip${bidi}Work" }).Status 400
Check "newline label rejected" (Api $alice "POST" "/api/v1/locations/shares" @{ friendship_id = $resend.Body.id; label = "Trip`nWork" }).Status 400
$share = Api $alice "POST" "/api/v1/locations/shares" @{ friendship_id = $resend.Body.id; label = "Work trip"; duration_hours = 24 }
Check "valid label accepted" $share.Status 201
Check "label listed" (@(Api $alice "GET" "/api/v1/locations/shares" $null).Body.label -contains "Work trip") "True"
Check "label cleared" (Api $alice "PATCH" "/api/v1/locations/shares/$($share.Body.id)" @{ label = "" }).Status 200

Write-Host "== Notes and check-ins =="
Check "bidi note label rejected" (Api $alice "POST" "/api/v1/locations/notes" @{ latitude = 47.01; longitude = 28.86; label = "Cafe${bidi}"; message = "hello" }).Status 400
Check "bidi note message rejected" (Api $alice "POST" "/api/v1/locations/notes" @{ latitude = 47.01; longitude = 28.86; message = "hi${bidi}there" }).Status 400
Check "newline note message accepted" (Api $alice "POST" "/api/v1/locations/notes" @{ latitude = 47.01; longitude = 28.86; message = "line one`nline two" }).Status 201
$longNote = [string]::new([char]0x78, 2001)
Check "2001-character check-in note rejected" (Api $alice "POST" "/api/v1/locations/check-ins" @{ latitude = 47.01; longitude = 28.86; note = $longNote; kind = "manual" }).Status 400
Check "bidi check-in note rejected" (Api $alice "POST" "/api/v1/locations/check-ins" @{ latitude = 47.01; longitude = 28.86; note = "hi${bidi}"; kind = "manual" }).Status 400
Check "valid check-in accepted" (Api $alice "POST" "/api/v1/locations/check-ins" @{ latitude = 47.01; longitude = 28.86; note = "ok"; kind = "manual" }).Status 201

Write-Host "== Search bound =="
$q120 = [string]::new([char]0x61, 120)
$q121 = [string]::new([char]0x61, 121)
Check "120-character query accepted" (Api $alice "GET" "/map/search?q=$q120" $null).Status 200
Check "121-character query rejected" (Api $alice "GET" "/map/search?q=$q121" $null).Status 400

Write-Host "== Offline map catalog =="
$catalogResp = Get-Public "/map/catalog"
Check "catalog responds 200" $catalogResp.Status 200
Check "catalog has a non-empty regions array" (@($catalogResp.Body.regions).Count -gt 0) "True"
Check "every catalog region has id and name" (@($catalogResp.Body.regions | Where-Object { -not $_.id -or -not $_.name }).Count) 0

Write-Host ""
Write-Host "$($script:pass) passed, $($script:fail) failed"
exit $script:fail

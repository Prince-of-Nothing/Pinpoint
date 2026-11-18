# Live security and accessibility audit (Midterm 2)

**Target:** https://pinpoint-uj4y.onrender.com
**Server version:** `2.6.1` (from `/health`)
**Branch audited from:** `docs/midterm2`
**Date:** 18 November 2026, 08:47-08:54 UTC
**Tools:** `curl` (Windows 11), Puppeteer + Chrome 156.0.8078.47 (headless), Lighthouse 12.8.2
**Scope:** live deployment only. No source changes were made; this file is the only write. No commit.

## Verdict summary

| # | Check | Result |
|---|-------|--------|
| 1 | `GET /health` returns healthy + PostgreSQL | **PASS** |
| 2 | Security headers on `/` | **PASS** |
| 3 | Security headers on `/api/v1` | **FAIL** (no route, no headers; sampled API JSON responses also carry none) |
| 4 | CSP present and effective | **PASS** (with two permitted relaxations, see below) |
| 5 | `Strict-Transport-Security` | **FAIL** (absent) |
| 6 | Cookie `HttpOnly` | **PASS** |
| 7 | Cookie `SameSite=Lax` | **PASS** |
| 8 | Cookie `Secure` | **FAIL** (absent on HTTPS deployment) |
| 9 | Unauthenticated check-ins return 401 | **PASS** |
| 10 | Unauthenticated emergencies return 401 | **PASS** |
| 11 | Admin API unauthenticated returns 401 | **PASS** |
| 12 | Admin API as resident returns 403 | **PASS** |
| 13 | Lighthouse accessibility, login page (mobile) | **PASS - 100/100** |
| 14 | Lighthouse best practices, login page (mobile) | **PASS - 96/100** |
| 15 | Lighthouse accessibility / best practices (desktop) | **PASS - 100 / 96** |
| 16 | HTTP to HTTPS redirect | **PASS** (301) |
| 17 | Throwaway account deleted and session invalidated | **PASS** |

The failures above are deployment hardening gaps, not functional breakage:
security headers are attached per response in `src/server/web_handlers.rs`
(`secure()`), the session cookie builder in `src/server/auth_handlers.rs` has no
`Secure` flag in any environment, and no layer adds HSTS.

## 1. Health endpoint - PASS

```http
HTTP/1.1 200 OK
Content-Type: application/json
```

```json
{"database":"connected","mode":"postgres","status":"healthy","version":"2.6.1"}
```

Matches the report's v2.6.1 / PostgreSQL claim.

## 2. Security headers

### 2.1 `/` - PASS

```http
HTTP/1.1 200 OK
Content-Type: text/html; charset=utf-8
Cache-Control: no-store
content-security-policy: default-src 'self'; img-src 'self' data: https://tiles.openfreemap.org; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self'; connect-src 'self' https://tiles.openfreemap.org; base-uri 'none'; frame-ancestors 'none'; form-action 'self'; object-src 'none'; manifest-src 'self'
cross-origin-embedder-policy: require-corp
cross-origin-opener-policy: same-origin
cross-origin-resource-policy: same-origin
permissions-policy: geolocation=(self), camera=(), microphone=(), payment=(), usb=()
referrer-policy: no-referrer
x-content-type-options: nosniff
x-frame-options: DENY
```

`/privacy` returns the same set with `Cache-Control: no-store`. This is a strong
baseline: clickjacking blocked twice (`frame-ancestors 'none'` + `X-Frame-Options:
DENY`), MIME sniffing off, no referrer leakage, no third-party script origins,
and geolocation restricted to self.

### 2.2 `/api/v1` - FAIL

```
GET /api/v1        -> 404, Content-Length: 0, no security headers
GET /api/v1/        -> 404, no security headers
GET /api/v1/sync-status        -> 200 application/json, no security headers
GET /api/v1/locations/check-ins -> 401 application/json, no security headers
```

Example 401 response headers in full (Cloudflare ephemerals omitted):

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json
cf-cache-status: DYNAMIC
Server: cloudflare
x-render-origin-server: Render
```

No CSP, `X-Content-Type-Options`, `X-Frame-Options`, `Referrer-Policy` or
`Permissions-Policy` leaves on any sampled API response, error or success. The
headers are not middleware; `secure()` is called by the HTML/asset handlers
only. The `/api/v1` prefix itself is not a route, so its 404 is expected, but
the absence on real JSON endpoints is the actual finding.

### 2.3 CSP assessment - PASS

The policy is present and same-origin by default, which is what the report
claims. Two relaxations are real and should be stated rather than hidden:

- `style-src 'self' 'unsafe-inline'` - needed by MapLibre inline styles.
- `script-src 'self' 'wasm-unsafe-eval'` - needed for the SQLite WASM build.

There is no `report-uri`/`report-to`, so violations are not collected. That is a
monitoring gap, not a bypass.

### 2.4 HSTS - FAIL

`strict-transport-security` is absent on `/`, `/privacy` and all API responses.
Lighthouse's `has-hsts` audit reports an item of severity `High`: "No HSTS
header found" (the audit is `informative`, so it does not lower the score).
The site does redirect `http://` to `https://` with `301`, so a first visit is
upgraded, but there is no protection after that against a downgrade on a later
request.

## 3. Session cookie flags

A throwaway account was registered with a unique address
(`midterm2-audit-20261118@example.com`). Registration response:

```http
HTTP/1.1 201 Created
Content-Type: application/json
Set-Cookie: session_id=<64 hex chars>; Path=/; HttpOnly; SameSite=Lax
x-csrf-token: <64 hex chars>
```

```json
{"access_token":"","csrf_token":"<redacted>","expires_in":43200,"token_type":"session","user":{"id":6054,"role":"resident","username":"cosmic-star-6054"}}
```

Token values are redacted here; the session was revoked during cleanup.

| Attribute | Observed | Verdict |
|-----------|----------|---------|
| `HttpOnly` | present | PASS |
| `SameSite` | `Lax` | PASS |
| `Path` | `/` | PASS |
| `Secure` | **absent** | **FAIL** |
| `__Host-` prefix | absent | hardening gap |
| `Max-Age` / `Expires` | absent (browser-session cookie) | informational |

The `Secure` omission is unconditional in the source, not environment-specific:
`session_cookie()` in `src/server/auth_handlers.rs` is documented as a loopback
dev convenience ("no `Secure` flag so the browser stores it"), but the same
string is what the live HTTPS deployment sends. Two consequences worth stating:

1. A network attacker who can inject an HTTP response for the host can write or
   overwrite `session_id`; there is no `__Host-` prefix to prevent it.
2. On `*.onrender.com`, which is shared hosting, a cookie without the
   `__Host-` prefix can be set with `Domain=onrender.com` by another tenant,
   enabling cookie tossing/session fixation against this app. Not demonstrated
   here, but the mitigation is two lines: add `Secure` and use
   `__Host-session_id`.

Logout/deletion clears the cookie with the same missing flag:
`session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0`.

## 4. Unauthenticated private endpoints - PASS

| Request | Status | Body |
|---------|--------|------|
| `GET /api/v1/locations/check-ins` | **401** | `{"error":{"code":"UNAUTHENTICATED","message":"Sign in required."}}` |
| `POST /api/v1/locations/check-ins` | **401** | same envelope |
| `GET /api/v1/locations/emergency` | **401** | same envelope |
| `POST /api/v1/locations/emergency` | **401** | same envelope |

This confirms the report's claim that check-ins and emergencies require
authentication, and that the 401 is returned before any body validation or
side effect.

## 5. Admin API authorization - PASS

| Request | Status | Body |
|---------|--------|------|
| `GET /api/v1/admin/overview` (no cookie) | **401** | `UNAUTHENTICATED` |
| `GET /api/v1/admin/users` (no cookie) | **401** | `UNAUTHENTICATED` |
| `GET /api/v1/admin/audit` (no cookie) | **401** | `UNAUTHENTICATED` |
| `GET /api/v1/admin/overview` (resident cookie) | **403** | `{"error":{"code":"FORBIDDEN","message":"Administrator role required."}}` |

The resident session was a real server-side session created by registration, so
the 403 is an authorization decision, not an authentication failure. Server-side
role enforcement is working.

## 6. Lighthouse on the login page

**URL:** `https://pinpoint-uj4y.onrender.com/`
**Lighthouse:** 12.8.2, Chrome 156.0.8078.47 headless, mobile emulation with
simulated throttling (default preset), fetch time `2026-11-18T08:50:15Z`. Desktop
preset run separately.

| Category | Mobile | Desktop |
|----------|--------|---------|
| Accessibility | **100** (1.00) | **100** (1.00) |
| Best practices | **96** (0.96) | **96** (0.96) |

Accessibility: all 13 weighted audits pass, including `color-contrast`,
`html-has-lang`, `document-title`, `label`, `button-name`, `link-name`,
`target-size` and `meta-viewport`; `aria-allowed-attr` and `image-alt` are not
applicable on this page. The login form is correctly marked up
with wrapping `<label>` elements for Email, Password and the optional
authenticator code; inputs use `autocomplete="username"`,
`autocomplete="current-password"` and `autocomplete="one-time-code"`.

Best practices: the only failing weighted audit is `errors-in-console`
(weight 1 of 28, hence 27/28 = 96). The single console error is a **401 from
`GET /api/v1/auth/me`**, which is the expected logged-out probe the page makes
to decide whether to show the session view. It is cosmetic for scoring, not a
defect, but it does cost the 4 points; suppressing the probe on the login route
or returning 204 would recover them.

Score caveats:

- `csp-xss` is `notApplicable` with no findings, so the strong CSP is **not**
  reflected in the 96; Lighthouse did not surface any CSP weakness.
- `has-hsts` is `informative` and scores 1 even though it reports "No HSTS
  header found" at severity High. The 96 therefore does **not** penalize the
  missing HSTS header.
- Chrome also logs two `verbose` DOM advisories that Lighthouse does not score:
  "Password field is not contained in a form" and "Password forms should have
  (optionally hidden) username fields". The first is real but limited to the
  hidden delete-account screen (`#delete-password` has no ancestor `<form>`);
  the visible login password is inside `#login-form`. The second is a
  Chrome-specific advisory; the form does provide `autocomplete="username"`.

## 7. Test account cleanup - PASS

- Registered `midterm2-audit-20261118@example.com` (user id `6054`, role
  `resident`).
- `DELETE /api/v1/users/me` with the session cookie, `X-CSRF-Token` and body
  `{"confirm":"DELETE","current_password":"..."}` returned:
  `HTTP/1.1 200 OK` and `{"message":"Account deleted."}`, with a clearing
  `Set-Cookie`.
- Post-deletion checks: `GET /api/v1/auth/me` with the old cookie -> **401**;
  `POST /api/v1/auth/login` with the same address -> **401**. The account and its
  session are gone.

## 8. Claims in the Midterm 2 report that need qualifying

| Report location | Claim as written | Observed | Suggested qualification |
|-----------------|------------------|----------|-------------------------|
| `Midterm2-Report.md:118` | "Every request goes through the same middleware order: security headers, session validation, ..." | Security headers are not middleware and are absent from all JSON/API responses. Only HTML and static assets get them via `web_handlers::secure()`. | "HTML and static asset responses are built with the security-header set; API responses use the same CSP/transport baseline but do not repeat the headers." Or add a real layer so the claim becomes true. |
| `Midterm2-Report.md:161` | "`HttpOnly` + `SameSite=Lax` + `Secure` cookies in production" | Live `Set-Cookie` is `session_id=...; Path=/; HttpOnly; SameSite=Lax` - no `Secure`, on HTTPS. The source has no production branch. | "`HttpOnly` + `SameSite=Lax` cookies; adding `Secure` and the `__Host-` prefix is outstanding production hardening." |
| `Midterm2-Report.md:173` | "a strict CSP" | Policy is strong but allows `style-src 'unsafe-inline'` (MapLibre) and `script-src 'wasm-unsafe-eval'` (SQLite WASM); no violation reporting. | Keep "strict CSP" but add the two intended relaxations and note there is no `report-to` yet. |
| `Midterm2-Report.md:241` | "unauthenticated check-ins/emergencies return `401`" | Confirmed exactly, GET and POST, both resources, same `UNAUTHENTICATED` envelope. | No change needed. |
| `Midterm2-Report.md:265` | "Live `/health`: version `2.6.1`, PostgreSQL connected" | Confirmed exactly. | No change needed. |
| `Midterm2-Report.md:303` | Evidence row "Security headers, CSP, same-origin assets -> `tests/assets.rs`, `web_handlers.rs`" | Accurate for assets and HTML; the tests do not cover API responses because no headers are emitted there. | Note the coverage boundary: asset/HTML only. |
| `Checklist.md:42-44` | Login page loads as `assets/01-login.png` | Page loads, title `Pinpoint — private offline map`, `#login-form` with Email/Password/Authenticator fields. | No change needed. |

Not claimed anywhere in the docs but relevant to the same section: there is no
HSTS header. If a reviewer asks "is transport locked down", the honest answer
today is "HTTPS + redirect, but no HSTS yet".

## 9. Reproduction

```bash
curl -i https://pinpoint-uj4y.onrender.com/health
curl -I https://pinpoint-uj4y.onrender.com/
curl -i https://pinpoint-uj4y.onrender.com/api/v1
curl -i https://pinpoint-uj4y.onrender.com/api/v1/sync-status
curl -i https://pinpoint-uj4y.onrender.com/api/v1/locations/check-ins
curl -i -X POST https://pinpoint-uj4y.onrender.com/api/v1/locations/emergency
curl -i https://pinpoint-uj4y.onrender.com/api/v1/admin/overview
```

Registration used a JSON body (`email`, `password`, `full_name`), then the
returned cookie and `X-CSRF-Token` were used for the resident 403 check and for
`DELETE /api/v1/users/me` with `{"confirm":"DELETE","current_password":"..."}`.

Lighthouse:

```powershell
$env:CHROME_PATH="C:\Users\Dinusauuuur\.cache\puppeteer\chrome\win64-156.0.8078.47\chrome-win64\chrome.exe"
npx.cmd --yes lighthouse@12 https://pinpoint-uj4y.onrender.com/ `
  --only-categories=accessibility,best-practices `
  --output=json --output-path=lh-login.json `
  --chrome-flags="--headless=new --no-sandbox --disable-gpu" --quiet
```

Add `--preset=desktop` for the desktop numbers. Raw reports were kept in the
session temp directory, not committed.

## 10. Not checked

- CSRF rejection on an unsafe request **without** a token, rate limiting, and
  lockout behavior (outside the requested scope; the successful deletion did
  prove a valid token is required and honored).
- TLS version/cipher suite and certificate chain (headers only were audited).
- Real screen-reader and keyboard-only passes; Lighthouse a11y is automated
  coverage only.
- Load or multi-replica behavior.

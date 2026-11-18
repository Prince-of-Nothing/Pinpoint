# Report evidence fact-check - `Midterm2-Report.md`

**Checked:** 2026-11-18, branch `docs/midterm2` (HEAD `5285bc6b`).
**Report revision checked:** working tree at 10:58 (19,405 B), after an edit that landed while this review was running.
**Code under test:** `345aa37f` (v2.6.1). `git diff 345aa37f..HEAD -- src tests Cargo.toml Cargo.lock android web` is empty, so the built test harness and the source line numbers below apply to the report's tree.
**Live system:** https://pinpoint-uj4y.onrender.com
**Method:** read-only source inspection, GitHub REST API, live unauthenticated GETs, and the already-built Rust test harness. No accounts were registered and the full `cargo test --locked` gate was not re-run; CI job steps and the built harness were used instead.

**Status key**

- **PASS** - the claim is supported by the cited evidence.
- **WRONG** - the claim is contradicted by the cited evidence; replace it.
- **OVERSTATED** - partly true but broader or more precise than the evidence, or it conflates two things.
- **UNVERIFIED** - no repository or live artifact can establish it either way (typically a self-reported manual run). Not evidence of failure, but not evidence of the claim either.

Tally: **45 PASS, 3 WRONG, 4 OVERSTATED, 5 UNVERIFIED** across 57 checked claims.

> **Changes during review.** The report was edited at 10:58 while this review ran. The earlier draft claimed `Secure` cookies and login/registration rate limiting; the current text (lines 161-167 and 327-332) correctly says the server does **not** set `Secure`, has no HSTS or `__Host-` prefix and has no per-IP server-side rate limit. Those two issues are therefore **fixed in the current text** and are not counted as errors below. Re-check the affected rows if the report changes again.

---

## 1. Version, release and APK

| # | Claim (report line) | Evidence | Verdict | Correction |
|---|---|---|---|---|
| 1 | Version 2.6.1 and release `v2.6.1` (lines 4, 9, 30) | `Cargo.toml:3` `version = "2.6.1"`; GitHub release `v2.6.1` exists, published `2026-11-18T08:28:49Z`, not draft/prerelease | PASS | - |
| 2 | Live `/health` reports 2.6.1 and PostgreSQL connected (lines 270, 388-389) | Live body: `{"database":"connected","mode":"postgres","status":"healthy","version":"2.6.1"}` | PASS | - |
| 3 | APK `pinpoint-android-2.6.1.apk`, 13,632,525 B (line 266) | GitHub release asset exactly 13,632,525 B; `docs/midterm2/reviews/artifact.md:32-41` confirms byte-identity and SHA-256 `402b5c76...abc2e` | PASS | - |
| 4 | `versionName` 2.6.1, `versionCode` 20601 (line 267) | `aapt dump badging` in `artifact.md:51`: `versionCode='20601' versionName='2.6.1'`; formula at `android/app/build.gradle.kts:14-17` gives `2*10000+6*100+1` | PASS | - |
| 5 | `arm64-v8a` + `armeabi-v7a` (lines 85, 267) | `artifact.md:56` native-code line and `artifact.md:63-72` ZIP listing show exactly those two ABIs and no x86/x86_64 | PASS | - |
| 6 | Signed with the Pinpoint release key, cert SHA-256 `2b326ca1...e7e49` (lines 268-269) | `artifact.md:74-98`: `apksigner verify` v2 and v3 true, exit 0, signer certificate SHA-256 exactly `2b326ca140f5d34ac61672fb3a5ec2596ced91f9a2fd618d50b52fe89dea7e49` | PASS (wording) | The quoted digest is the **certificate** digest, not the RSA key digest. Say "release certificate (SHA-256 ...)"; the public-key digest is `621a338d...` (`artifact.md:124-128`). |
| 7 | Screenshots are from deployed v2.6.0/v2.6.1; native shot is the v2.6.0 theme (lines 394-395) | Images exist, and `docs/midterm2/reviews/deck-qa.md:114-121` confirms the 6 referenced images load, but PNGs carry no version metadata | UNVERIFIED | Attach capture metadata, or change to "Screenshots from the demo environment; version not embedded in the images." |

## 2. Feature matrix, architecture and security claims

| # | Claim (report line) | Evidence | Verdict | Correction |
|---|---|---|---|---|
| 8 | Identity: registration, login, TOTP, opaque sessions, sign-out everywhere, password change, deletion (line 75) | Routes `src/server/mod.rs:254-281`; Argon2id `src/security/password.rs:1-9`; 32-byte session ids `src/security/session.rs:237-240`; tests `tests/contract.rs`, `tests/auth_password.rs`, `tests/account_delete.rs` | PASS | - |
| 9 | Friends by exact handle; "emails are not searchable" (line 76) | `lookup_user` is exact username and requires auth (`src/server/users_handlers.rs:173-198`). The admin-only `GET /api/v1/users/search` matches username, **email** and full name (`src/server/users_handlers.rs:409-430`) | OVERSTATED | Replace with "Request by exact handle; handles are public, and emails are not searchable by other residents." |
| 10 | Sharing consent sheet with friend, precision, duration, summary, active/paused/expired/revoked (line 77) | `tests/location_consent.rs` 10 tests; `web/app/test/sharing-ui.mjs` passes 17/17 (run below); `src/ui/share_sheet.rs` | PASS | - |
| 11 | Publish manual or GPS; friend reads only an active, unexpired, owner-created share (line 78) | Web geolocation `web/app/map.js:391-395`; native `GpsProvider` `src/platform.rs:160-230`; friend filter `src/server/locations_handlers.rs:576+`; `tests/location_consent.rs:79` | PASS | - |
| 12 | Private 24-hour trail; 7-day server retention (lines 79, 195) | 24 h cutoff `src/server/locations_handlers.rs:361-370`; `DELETE ... recorded_at < now-7d` runs on each publish `src/server/state/locations.rs:269-274` | PASS (qualified) | Retention is lazy, not a scheduled purge. Say "pruned to 7 days on the next publish"; an inactive account keeps older rows until it publishes again or is deleted. |
| 13 | Notes: JPEG/PNG, downscaled in browser, bounded to 675 KB, owner-only (line 80) | Server accepts data URLs up to 900,000 base64 chars (about 675,000 decoded bytes) `src/server/locations_handlers.rs:450-465`; browser caps 8 MB input, 1024 px and JPEG 0.82 `web/app/app.js:498-512`; owner-only tests `tests/today.rs` | PASS (qualified) | The 675 KB limit is server-enforced and decoded-size based; the browser downsizes but does not verify the encoded size. Say "server-enforced, about 675 KB decoded". |
| 14 | Map: OpenFreeMap basemap, OSM search, OSRM driving directions, saved/recent places and routes (line 81) | `web/app/map.js:3` OpenFreeMap Liberty; live `/map/search?q=Chisinau&limit=1` returns Chișinău; live `/map/route?...mode=driving` returns 200 with a 138.1 km route; `/map/capabilities` returns `{"modes":["driving"],...}`; `web/app/nav.js:92-223` saved/recent places and recent routes | PASS | - |
| 15 | "Moldova pack (139 MB transfer, 204 MB installed)" in the same row as OPFS install (line 82) | Native region catalog `web/catalog.json`: 93,172,399 + 45,338,586 = **138,510,985 B** transfer (138.5 MB) and 93,172,399 + 110,858,815 = **204,031,214 B** installed (204.0 MB). Live web OPFS manifest `/offline/packs/moldova/manifest.json`: 93,172,399 + 6,770,688 + 120,319,488 = **220,262,575 B** transfer (220.3 MB) | OVERSTATED | The row conflates the native pack with the web pack. Replace with: "Moldova native pack: 138.5 MB transfer / 204.0 MB installed; web OPFS pack: 220.3 MB (93.2 MB PMTiles + 6.8 MB search SQLite + 120.3 MB Valhalla tar)." |
| 16 | "Romania pack in the catalog" (line 82); "Moldova and Romania are published from pinpoint-maps" (lines 25-26) | Romania **is** published upstream (`data-romania-20261117-44139439624` release exists). But `web/catalog.json` has only Moldova; `web/app/offline-catalog.js` has Monaco + Moldova only; live `/map/catalog` returns only Moldova | WRONG | Replace with: "Romania data is published from `pinpoint-maps` but is not yet in either client catalog (`web/catalog.json`, `web/app/offline-catalog.js`); only Moldova and the Monaco test fixture are listed." |
| 17 | Account: active sessions with revoke, display name, diagnostics, delete (line 83) | Session routes `src/server/mod.rs:263-268`; display name `web/app/app.js:194+`; diagnostics `src/server/diagnostic_handlers.rs`; deletion test `tests/account_delete.rs` | PASS | - |
| 18 | Admin: role-gated overview/users/diagnostics/append-only audit; demo needs DB promotion (line 84) | Admin router `src/server/mod.rs:303-330`; `require_admin` in handlers; no update/delete audit route; `pinpoint-admin --promote-email` `src/bin/admin.rs:14-32,115-130` | PASS | - |
| 19 | Clients: web, desktop, Android APK, native map server, v2.6.1 (line 85) | Bins in `Cargo.toml`; APK verified above; native map server `src/map_server.rs:121-163` and `src/mobile/map_server.rs:162-279` | PASS | - |
| 20 | Native Today note list labels a photo attachment and points to web (lines 87-89) | `src/ui/today.rs:155-160`: "Photo attached (view it in the web client)." | PASS | - |
| 21 | "No demo accounts and no seeded pins are shipped; every account is registered through the public flow" (lines 34-35) | No seeded users/pins: `Store::new` starts empty and migrations have no user inserts. But an admin-only `POST /api/v1/users/` can create accounts (`src/server/users_handlers.rs:57-63`) | OVERSTATED (minor) | Replace "every account is registered through the public flow" with "no seeded accounts; demo accounts are registered through the public flow". |
| 22 | "One Rust codebase with three client surfaces: a web client, a desktop client, and an Android build" (lines 21-22) | The shipped web UI is vanilla JS/MapLibre/PMTiles (`web/app/index.html`, `web/app/app.js`, `web/app/map.js`). The Rust `pinpoint-web*` bins are native stubs: `src/bin/pinpoint-web.rs:1-25` prints "pinpoint-web (native stub)" and the wasm entry is empty. The report's own architecture diagram says "vanilla ES modules + MapLibre/PMTiles" | OVERSTATED | Replace with: "One repository: a Rust/Axum API and Rust native client (eframe/egui, also built for Android), plus a vanilla-JS/MapLibre/PMTiles web client served by the same Rust server. The Rust `pinpoint-web*` bins are native stubs for the wasm compile check, not the shipped web UI." |
| 23 | Argon2id hashes, opaque 256-bit sessions (lines 161-162) | `src/security/password.rs:1-9`; 32 random bytes `src/security/session.rs:237-240` | PASS | - |
| 24 | Current text: no `Secure`, no `__Host-`, no HSTS (lines 165-167) | `src/server/auth_handlers.rs:27-31` sets only `Path=/; HttpOnly; SameSite=Lax`; `session_id` has no `__Host-` prefix; live `/` sends no `Strict-Transport-Security` header | PASS (fixed in 10:58 edit) | - |
| 25 | CSRF token bound to the session, echoed in `X-CSRF-Token` on unsafe methods (lines 162-163) | `src/server/mod.rs:76-111`; `tests/contract.rs:88-114` (`unsafe_requests_send_csrf_double_submit`) | PASS | - |
| 26 | Current text: login locks after five failures; no per-IP server-side rate limit (lines 164-167) | `src/server/auth_handlers.rs:95-124` (5 failures -> 15 min lock); `RateLimiter` is explicitly **client-side** (`src/security/rate_limit.rs:1-4`) and appears nowhere under `src/server/` | PASS (fixed in 10:58 edit) | - |
| 27 | Friend coordinates never sent to Nominatim/OSRM/Overpass; only query/route endpoints (lines 171-173) | Policy comment `src/osm.rs:14`; the route UI sends only route endpoints (`web/app/nav.js:453`); no friend id or share data is sent | PASS (qualified) | The route endpoints themselves are coordinates. If a friend's pin can ever become an endpoint, the first clause is false. Keep the existing "only the query or the route endpoints" qualifier. |
| 28 | Transport/CSP: strict CSP, `frame-ancestors 'none'`, documented inline-style/wasm relaxations, HSTS absent, JSON API headers differ (lines 117-121, 175-179) | Live `/` sends `Content-Security-Policy: default-src 'self'; ... style-src 'self' 'unsafe-inline'; script-src 'self' 'wasm-unsafe-eval'; ... frame-ancestors 'none'`, `X-Frame-Options: DENY`, no HSTS. Live `/health` and `/map/catalog` return 200 without CSP/XFO, matching "JSON API responses currently do not carry the same header set" | PASS | - |

## 3. Test counts and live checks

| # | Claim (report line) | Evidence | Verdict | Correction |
|---|---|---|---|---|
| 29 | Library has 328 unit tests (line 220) | Built lib harness `target/debug/deps/pinpoint_desktop-43be06fe031c27d0.exe --list` reports exactly **328** tests; code is unchanged since `345aa37f` | PASS | - |
| 30 | 24 test binaries (line 220) | `cargo metadata`: 13 bins + 10 integration tests + 1 lib = 24 test-capable targets. Under `cargo test --locked`, `pinpoint-offline-index` is skipped (requires `offline-build`) and the doc-test target brings the executed sections to 24 (12 bins + 10 tests + lib + doc-tests) | PASS (composition note) | If challenged, spell out the composition: "24 test targets (13 bins + 10 integration + lib), one bin feature-gated." |
| 31 | `tests/location_consent.rs` 10 passed (line 225) | 10 `#[tokio::test]` functions in the file; names cover consent, precision, pause/revoke, expiry, invalid values, check-ins/emergencies | PASS | - |
| 32 | `tests/today.rs` 2 passed (line 226) | 2 tests: `trail_is_owner_only_and_records_each_publish`, `place_notes_are_owner_only_and_validated` | PASS | - |
| 33 | `tests/contract.rs` 11 passed (line 227) | 11 `#[tokio::test]` functions, including CSRF, numeric ids, and `today_endpoints_are_owner_paths_with_csrf_on_writes` | PASS | - |
| 34 | `tests/assets.rs` 11 passed (line 228) | 11 tests, including `today_and_notes_assets_are_wired`, CSP, no-third-party-host and service-worker no-API-cache checks | PASS | - |
| 35 | `tests/router_probe.rs` 17 passed (line 229) | 17 test functions, including health, session revoke, admin gates, diagnostics/audit, map routes, privacy | PASS | - |
| 36 | `tests/account_delete.rs` 1 passed (line 230) | 1 test | PASS | - |
| 37 | `tests/auth_password.rs` 1 passed (line 231) | 1 test | PASS | - |
| 38 | "Latest result: pass" for fmt/clippy/build/test (line 220) | CI run 44265766973 and the tag run 44265793804 both show `Format check`, `Clippy`, `Build (locked)` and `Tests (locked)` success on Linux and macOS at `345aa37f` | PASS | The four-command gate passes, but the `rust` job still fails at `cargo-audit`; see rows 48-49. |
| 39 | Latest live run on v2.6.1: 14/14 functional checks (lines 249-253) | No committed log or script. The bullet list at lines 237-247 enumerates 10 checks, not 14. No live-check script exists in `scripts/`; `scripts/v2/check.ps1` is an offline build gate. `tests/live_api.rs` is `#[ignore]`d and still targets a removed Go API | UNVERIFIED | Attach the run transcript, or change to "manual live check, transcript not archived" and reconcile 14 with the 10 listed checks. |
| 40 | Earlier v2.5.2 run passed 24/24 (line 253) | No artifact anywhere in the repo; only the report/handout repeat the number | UNVERIFIED | Attach the v2.5.2 log, or drop the 24/24 claim. |
| 41 | Live web UI 9/9 (lines 257-259) | No run log. The parenthetical lists 7 checks (empty-map prompt, online map, two-point trail, trail toggle, photo preview, note save/marker, mobile overflow), not 9 | UNVERIFIED | Attach the browser-run transcript, or label the checks manual and reconcile 9 with the 7 listed. |
| 42 | Offline web 7/7 (lines 260-261) | No run log. The parenthetical lists 4 checks (install, reload, map, search), not 7 | UNVERIFIED | Attach the offline-run transcript, or reconcile 7 with the 4 listed. |
| 43 | Sharing UI unit checks 17/17 (line 262) | `node web/app/test/sharing-ui.mjs` prints `sharing UI checks passed: 17` and exits 0 | PASS | - |
| 44 | Unauthenticated check-ins/emergencies return 401 (line 246) | Live `GET /api/v1/locations/check-ins` -> 401, `/emergency` -> 401, `/history` -> 401 | PASS | - |
| 45 | Online search/routing works on the live system (lines 235-248, 286) | Live `/map/search?q=Chisinau&limit=1` -> 200 with a Chișinău result; live `/map/route?...mode=driving` -> 200, 138.1 km, 7621 s, via Tiraspol-Bălți/M5 | PASS | - |

## 4. Data, CI, docs and endpoints

| # | Claim (report line) | Evidence | Verdict | Correction |
|---|---|---|---|---|
| 46 | Migration 16 adds `pinpoint_location_history` and `pinpoint_place_notes` (line 183) | `src/server/migrations.rs:133-140` | PASS | - |
| 47 | Direct writes protect users/sessions/friendships/shares/location/trail/notes; snapshot no longer overwrites (lines 201-205) | `persist()` writes the JSONB snapshot plus normalized tables `src/server/state/mod.rs:245-305`; `init_postgres`/`refresh_from_db` call `load_normalized` first and only fall back to the snapshot `src/server/state/mod.rs:192-241`; history/notes are `#[serde(skip)]` `src/server/state/store.rs:37-44` | PASS | - |
| 48 | "CI: Rust, wasm, map QA and offline data checks pass" (lines 271-272) | CI run **44265766973** (main, `345aa37f`) and tag run **44265793804** (`v2.6.1`) both conclude **failure**. Jobs: `wasm` success, `client-macos` success, `release-windows` success; `rust` fails at step **Dependency vulnerability audit (cargo-audit)** after fmt/clippy/build/tests succeed | WRONG | Replace with: "CI is red on v2.6.1. `wasm`, `client-macos`, `release-windows`, Map QA and Offline data pass; the `rust` job passes fmt/clippy/build/`cargo test --locked` but fails `Dependency vulnerability audit (cargo-audit)` (runs 44265766973 and 44265793804)." |
| 49 | "The macOS job has an unrelated flaky `offline_install` test" (lines 271-272, 333-335) | Both v2.6.1 CI runs show `client-macos` **success**; the only red job is `rust` at `cargo-audit`. No macOS `offline_install` failure appears in these runs | WRONG | Replace with: "**CI red at the dependency audit.** The v2.6.1 macOS job passes; the `rust` job fails `cargo-audit`. Resolve or pin the advisory before claiming a green CI." |
| 50 | Offline data checks pass (line 271) | Workflow `Offline data` succeeded on PR run `3ddb1a10`; it is path-filtered and did not run on `345aa37f` | PASS (qualified) | Say "Offline data passed on the PR; it is not part of the v2.6.1 commit run." |
| 51 | Release `v2.6.1` artifact published (lines 9, 266) | GitHub Release workflow conclusion success for `v2.6.1`; release is latest, not draft/prerelease | PASS | - |
| 52 | Presentation is a 13-slide deck (line 279) | `docs/midterm2/Presentation.md` has `## Slide 1` through `## Slide 13`; `deck-qa.md:5,12` confirms 13/13 render and 13 PDF pages | PASS | - |
| 53 | 8-minute run (line 286) | `Demo-Script.md:3` says "6-8 minutes" and `:40` says "6-8 min" | PASS | - |
| 54 | Screenshot set exists (line 283) | `docs/midterm2/assets/01-login.png` ... `08-route.png` all exist; `deck-qa.md:114-117` confirms the 6 referenced images load | PASS | - |
| 55 | Demo uses real accounts, not seeded rows (lines 289-290) | No seeded users/pins in migrations or `Store::new`; registration is the public route `src/server/auth_handlers.rs:174+` | PASS (qualified) | Add "demo accounts" rather than "every account"; admin `POST /api/v1/users/` also exists (row 21). |
| 56 | Appendix A endpoint list (lines 359-373) | All listed paths exist: auth `src/server/mod.rs:254-281`, friendships/users `src/server/mod.rs:332-365`, locations `src/server/mod.rs:366-412`, diagnostics/admin `src/server/mod.rs:303-330`, diagnostics report `src/server/mod.rs:324-329` | PASS | - |
| 57 | Appendix B health JSON (lines 388-389) | Live body matches exactly (see row 2) | PASS | - |

---

## 5. Blocking corrections, copy-paste ready

1. **Abstract, lines 21-22.** Replace:
   > The project is implemented as one Rust codebase with three client surfaces: a web client, a desktop client (eframe/egui), and an Android build of the same client.

   with:
   > The project is one repository: a Rust/Axum API and a Rust native client (eframe/egui, also built for Android), plus a vanilla-JavaScript/MapLibre/PMTiles web client served by the same Rust server. The Rust `pinpoint-web*` binaries are native stubs for the wasm compile check, not the shipped web UI.

2. **Offline row, line 82.** Replace:
   > Moldova pack (139 MB transfer, 204 MB installed), OPFS install, service-worker range serving, offline search; Romania pack in the catalog

   with:
   > Moldova native pack: 138.5 MB transfer / 204.0 MB installed; web OPFS pack: 220.3 MB (93.2 MB PMTiles + 6.8 MB search SQLite + 120.3 MB Valhalla tar); OPFS install, service-worker range serving, offline search. Romania data is published from `pinpoint-maps` but is not yet in either client catalog.

3. **CI, lines 271-272 and 333-335.** Replace:
   > CI: Rust, wasm, map QA and offline data checks pass. The macOS job has an unrelated flaky `offline_install` test; it is not part of the demo path.

   with:
   > CI is red on v2.6.1. `wasm`, `client-macos`, `release-windows`, Map QA and Offline data pass; the `rust` job passes fmt/clippy/build/`cargo test --locked` but fails `Dependency vulnerability audit (cargo-audit)` (runs 44265766973 and 44265793804). There is no macOS `offline_install` failure in these runs.

   Replace the limitation at lines 333-335 with:
   > **CI red at the dependency audit.** The v2.6.1 `rust` job fails `cargo-audit`; fmt/clippy/build/tests and the `client-macos` job pass. Resolve or pin the advisory before claiming a green CI.

4. **Live run, lines 249-253.** Either attach the transcript, or replace with:
   > The two-account live check is a manual run; no transcript is archived. `scripts/v2/check.ps1` is an offline build gate that never contacts the live service, and `tests/live_api.rs` is `#[ignore]`d and still targets a removed Go API.

   The 14/14, 24/24 and "expected 2.6.0 version string" details are not reproducible from the repository: the report's own 14/14 bullet list has 10 items, and no committed script expects `2.6.0`.

5. **Browser checks, lines 257-262.** Either attach the runs, or change to:
   > Manual checks (transcript not archived): 9/9 live UI, 7/7 offline. The sharing UI check is reproducible: `node web/app/test/sharing-ui.mjs` prints `sharing UI checks passed: 17`.

   The 9/9 parenthetical lists 7 checks and the 7/7 parenthetical lists 4 checks.

6. **Screenshot footer, lines 394-395.** Replace:
   > All screenshots are from the deployed v2.6.0/v2.6.1 system; the native screenshot is the v2.6.0 theme build.

   with:
   > Screenshots are from the demo environment; the PNGs carry no embedded version. The native screenshot is the v2.6.0 theme build.

## 6. Minor precision fixes (not blocking)

- **Line 76:** "emails are not searchable" -> "emails are not searchable by other residents" (admin search matches email).
- **Line 35:** "every account is registered through the public flow" -> "no seeded accounts; demo accounts are registered through the public flow" (admin create path exists).
- **Line 79 / 195:** "7-day server retention" -> "pruned to 7 days on each publish" (no scheduled purge).
- **Line 80:** "bounded to 675 KB" -> "server-enforced at about 675 KB decoded; the browser downsizes but does not verify the encoded size".
- **Lines 268-269:** "release key (SHA-256 ...)" -> "release certificate (SHA-256 ...)"; the public key digest is different (`artifact.md:124-128`).
- **Line 101 (architecture diagram):** the API box says "session + CSRF + rate limits"; §4 correctly says there is no per-IP server-side rate limit and only account lockout. Change to "session + CSRF + account lockout" or "client-side rate limits".
- **Line 117:** "security-header middleware" is loose wording: headers are applied by `web_handlers::secure()` in each HTML/static handler, not a router layer. The following sentence about JSON API responses is confirmed live.

## 7. What remains unverified

- 14/14 on v2.6.1 and 24/24 on v2.5.2: no log, script or captured output in the repo.
- 9/9 live web UI and 7/7 offline: no browser-run transcript; the checked-in harnesses are automated/mocked, not live-run logs.
- The "expected 2.6.0" failure detail: no committed script expects 2.6.0; the server and all repo version strings are 2.6.1.
- Screenshot provenance (v2.6.0/v2.6.1).
- Physical-device install/launch of the APK (the APK itself is verified by `artifact.md`).

## 8. Other repository observations

- `tests/live_api.rs` is an ignored legacy test whose header still says "real Go API" and uses seeded `alice@example.com` accounts. The Go backend is gone and the live system has no seeds, so this file cannot be used as live-run evidence and is stale documentation.
- `scripts/v2/check.ps1` is a local/offline gate (Rust + offline pipeline + Node assets). It does not contact `https://pinpoint-uj4y.onrender.com`; there is no committed live-check script.
- The live `/map/catalog` and both client catalogs list only Moldova (plus the Monaco fixture); Romania exists only as a `pinpoint-maps` data release.
- The 10:58 report edit fixed the cookie/rate/middleware text; the remaining code-versus-report gaps are the offline catalog, the CI status and the unarchived live/browser runs.

## 9. Method limits

- No accounts were registered and no authenticated live write was attempted (per instructions), so registration/login, friend accept, share visibility, pause/resume/revoke and account deletion were checked against code/tests, not against the live service.
- The full `cargo test --locked` was not re-run. CI records the four gate steps as passing at `345aa37f`; the already-built lib harness lists 328 tests, and the checked-in test source supplies the suite counts.
- APK manifest/signature evidence is from the pre-existing review `docs/midterm2/reviews/artifact.md` and the GitHub asset metadata, not from a fresh APK download.
- GitHub Actions job logs require authentication (the log endpoint returned 403); job/step conclusions were read from the public API.
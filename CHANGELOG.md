# Changelog

All notable changes to this project are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
Semantic Versioning. 1.0.0 is the thesis demo release; production hardening is
tracked in the [wiki roadmap](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Roadmap).

Release flags: `[FEATURE]` new capability · `[FIX]` bug fix ·
`[SECURITY]` hardening · `[DOCS]` documentation · `[INFRA]` build/deploy.
The per-item flagged list for this release is in
[`RELEASE_NOTES.md`](./RELEASE_NOTES.md).

## [Unreleased]

## [3.0.0] - 2026-11-25
### Changed
- [DOCS] Release: v3.0.0 now publishes the signed universal `pinpoint-android-3.0.0.apk` (`arm64-v8a` + `armeabi-v7a`, 14,005,317 B); the evidence and deployment pages point at it and the duplicate debug release is gone.
- [DOCS] Version audit: the evidence index and deployment guide now name the 3.0.0 crate and `Pinpoint-mobile-3.0.0-debug.apk` (30000); the Midterm 2 package is explicitly marked as the v2.6.1 snapshot.
- [RELEASE] Pinpoint 3.0.0 unifies the desktop, Android/iOS and web version on the same Rust crate. API v1, the PostgreSQL schema, cookies and encrypted message/note formats are unchanged, so there is no data migration.
- [INFRA] The web service worker cache is bumped to `pinpoint-offline-v10` so installed clients refresh their precached shell on this major release.

## [2.7.1] - 2026-11-25
### Fixed
- [FIX] Native screenshot mode (`--screenshot`) now captures the real egui window with a seeded map screen and demo pins, instead of writing a gradient placeholder; a backend without readback support exits cleanly after a bounded wait.
- [FIX] Native map: friend labels now sit in a dark pill so they stay readable on light tiles; the desktop map opens the route panel for map actions and uses the window height, and screenshot mode can use the local backend for real tiles (plus `PINPOINT_SCREENSHOT_ROUTE=1` for a seeded route).

## [2.7.0] - 2026-11-25
### Changed
- [DOCS] API contract: a new "Response headers and throttling" section records the baseline headers, the `no-store` rule for API and map-query answers, the tile cache header and the `429` + `Retry-After` behavior.
- [DOCS] Evidence: a single index now links the gate, live smoke, SAST/DAST reports, backup self-tests, threat model, control register, traceability, models and artifact details for the report.
- [DOCS] Reference: the technology-stack, design-pattern and domain-data pages now describe the Rust system (crates, `src/patterns/`, real tables) instead of the removed Flutter/FastAPI design.
- [DOCS] Evidence: the security control register and requirements traceability matrix now describe the as-built Rust system (SC-01..SC-09, REQ-01..REQ-09) with current tests, scans and limitations instead of the removed Python backend.
- [INFRA] Backups: a host-side wrapper encrypts `pg_dump` with AES-256-CBC/PBKDF2 (openssl), verifies a decrypt roundtrip before deleting the plaintext temp file, and ships a restore script with a header check and overwrite guard; the passphrase stays outside the repository.
- [INFRA] DAST: a weekly ZAP baseline workflow starts the in-memory server and scans it, uploading the HTML/JSON report as an artifact; the latest report is committed under `docs/evidence/`.
- [INFRA] CI now runs CodeQL (Rust) as a SAST gate and Dependabot keeps Cargo and GitHub Actions dependencies current, complementing the existing `cargo audit` allowlist.
- [DOCS] Security architecture: the threat model is now STRIDE-tagged and scored (impact × likelihood) with a prioritised table, trust boundaries and an attack tree for location disclosure; the weeks 7–15 roadmap page the ADRs link to now exists with the as-built evidence index and open decisions.
- [INFRA] Native password forms now share pure, testable validators for registration and password change, covering 4-emoji rejection, 8-emoji acceptance, a missing current password, forced rotation and confirmation mismatch; the security-architecture test evidence matches the 373-test suite.
- [FEATURE] Tooling: `scripts/smoke.ps1` drives real sessions against a running server and checks the auth input rules, friendship rejection, location accuracy, share labels, notes, check-ins and the search bound (29 checks; exit code = failures).
- [DOCS] The testing guide now documents the Rust test layers, CI jobs and commands instead of the removed Go backend and its `thesis_content.md` reference.
- [DOCS] The threat-model attack tree now says a stolen phone exposes owner-only bookmark JSON, matching the code, instead of claiming AES-GCM bookmarks at rest.
- [INFRA] CI now runs the pure Node web UI checks (sharing consent and admin form wiring), so client regressions are gated like the Rust suite.
- [DOCS] The as-built API contract now matches the shipped routes: admin-only user endpoints, `receiver_username`, share labels/expiry/precision, history/notes/check-in fields, the FOSSGIS-only tile fallback and the real session lifetime.
- [DOCS] The authentication page now says the recovery code is stored as a SHA-256 hash, clarifies the admin audit table, and describes the admin-issued temporary password instead of "no password reset".
- [DOCS] Troubleshooting, threat-model, incident-response, domain-model and control-register pages now describe the owner-only 7-day trail and session revocation instead of "no history" and `SECRET_KEY` rotation.
- [DOCS] Authentication docs now record the shipped anonymous/recovery, admin password reset, session management and per-client rate limits instead of marking them pending.
- [DOCS] The deployment guide now describes the Rust stack (single binary, PostgreSQL, Compose, production checklist, desktop/Android/iOS) instead of the archived Python/Go/Kotlin steps; the Docker instructions use `db/init/03-fks.sql`.
- [DOCS] README now points at the 2.6.1 release and the actual Windows CI artifact instead of the old 1.2.1 bundle name.
- [DOCS] The security architecture page now describes the Rust server, the per-client throttles, the PostgreSQL session authority and the owner-only trail instead of the archived Go backend and its env vars.
- [DOCS] The security status and checklist now reflect PostgreSQL persistence, the queryable admin audit table, the implemented auth throttles and the `__Host-` cookie.
- [DOCS] The reference share-expiry snippet now enforces the seven-day cap instead of 24 hours.
- [DOCS] The public privacy policy now states the 7-day trail is pruned hourly, lists encrypted messages, explains the keyed-HMAC abuse limits, and drops the stale 1.5.0 build label.
- [INFRA] The convergence middleware only reloads the store for `/api/` requests, so loading a page no longer runs a full database refresh for every static asset.
- [DOCS] Threat model now records the per-client auth/write throttles and the owner-only 7-day trail instead of "no rate limits" and "latest-only".
- [DOCS] The security header list in `docs/SECURITY.md` now matches the server: HSTS, nosniff, XFO, Referrer-Policy, Permissions-Policy, COOP, CORP, X-Permitted-Cross-Domain-Policies and the web CSP.
- [DOCS] Security and privacy design notes now describe the real controls: per-account login lockout instead of per-IP limits, and the owner-only 7-day trail instead of "no history".
- [INFRA] Dependency audit: `audit.toml` records nine unmaintained or unsound transitive advisories with no fixed upgrade as accepted risk, so `cargo audit` passes with no open vulnerability.
### Added
- [INFRA] Release smoke: Linux launches the packaged binary under Xvfb and writes a screenshot, and macOS opens `Pinpoint.app` and checks the process stays alive before packaging.
- [INFRA] Desktop packaging: macOS now ships a `Pinpoint.app` bundle with Info.plist/icns and Linux ships `pinpoint.desktop` + icon, both validated in the release jobs.
- [INFRA] v3.0.0 release also attaches `pinpoint-desktop-macos-3.0.0.zip` (6,131,546 B); the tag workflow builds a macOS ZIP from now on.
- [INFRA] Release workflow: a macOS job builds `pinpoint-desktop`, checks `--help`, packages a README ZIP and the release job verifies and attaches it.
- [INFRA] v3.0.0 release also attaches `pinpoint-desktop-linux-x64-3.0.0.zip` (10,057,368 B); the tag workflow builds a Linux desktop ZIP from now on.
- [INFRA] Release workflow: a Linux x86_64 job builds `pinpoint-desktop`, checks `--help`, packages a README ZIP and the release job verifies and attaches it.
- [INFRA] Release job: the downloaded Windows ZIPs are extracted and checked for the expected `pinpoint-desktop.exe`/`pinpoint-server.exe` and README version before the draft is created.
- [INFRA] Release workflow: the Windows job checks `pinpoint-desktop.exe --help`, starts `pinpoint-server.exe`, and asserts `/health` reports the Cargo version and `/` returns HTML before packaging.
- [INFRA] Release workflow: a `windows-latest` job builds `pinpoint-desktop.exe` and `pinpoint-server.exe`, packages both ZIPs with dynamic READMEs, and the release job attaches them next to the APK.
- [INFRA] v3.0.0 release assets now include `pinpoint-desktop-windows-x64-3.0.0.zip` and `pinpoint-server-windows-x64-3.0.0.zip`; the server zip serves the embedded web client, so no separate web bundle is needed.
- [INFRA] Release dry run: `workflow_dispatch` now executes the release job's checkout, `aapt` install and downloaded-APK check, skipping only the draft creation.
- [INFRA] Release workflow: drafts now use `RELEASE_NOTES.md` as the body and re-run `check-release.ps1 -Apk` on the downloaded artifact before drafting, so a generic body or mismatched asset cannot ship.
- [INFRA] The tag release workflow now runs `scripts/check-release.ps1 -Apk` against the universal APK before drafting, so versionName/versionCode/ABI drift blocks the release.
- [INFRA] Release consistency: `scripts/check-release.ps1` compares Cargo, Android Gradle, iOS plist, the web cache marker, `/health` and an APK's versionName/versionCode; CI runs the metadata checks and the router tests assert `/health` matches the crate version.
- [FEATURE] Auth: GitHub OAuth 2.0 sign-in alongside Google, using the primary verified address from `/user/emails` and the same state/PKCE/no-auto-link rules.
- [FEATURE] Native map: right-click actions (directions from/to here, measure from here, add note here, copy coordinates) and a distance-measure HUD with numbered points, live total, undo and clear.
- [FEATURE] Web map: friend pins now show the friend's name above the precision circle; the labels are DOM markers, so they also render on the offline basemap that has no glyphs.
- [FEATURE] Web map: right-click/long-press actions (directions from/to here, measure from here, add note, copy coordinates) and a distance tool with numbered points, live total, undo and clear; `web/app/measure.js` is a pure module served same-origin.
- [FEATURE] Web: wide screens now keep the section cards in a right column with a full-bleed map and top-left search, MapLibre gains compass/pitch and fullscreen, and the zoom/fullscreen controls stay clear of the mobile tab bar; the Midterm 2 map, Today and notes screenshots were regenerated from this build.
- [FEATURE] Native: the friend list on the map screen now says how old each location is (`just now`, `5m ago`, `2d ago`), matching the web client and using the existing freshness thresholds.
- [FEATURE] Web: the friend-locations list now says how long ago each friend last shared a location (`just now`, `5m ago`, `2d ago`), so stale data is not mistaken for a live position.
- [FEATURE] Auth: optional Google OAuth 2.0 sign-in (authorization code + PKCE) with a single-use state bound to an HttpOnly cookie, a verified-email requirement, no automatic email linking, the same opaque session as password login, and an anonymous per-client throttle; disabled unless the client credentials are configured.
- [DOCS] Analysis: system models page with the topic, subject, objectives, functional/non-functional/CIA requirements, use-case, sequence, activity, state, ER, component and deployment diagrams, and the threat-to-use-case mapping for the Midterm 1 deliverables.
- [FEATURE] Web: the tab bar now opens real per-section screens (Friends, Sharing, Today, Chats, Account) instead of scrolling one long drawer, and the map has its own basemap (Offline/Streets) and layer (Friends/Trail/Notes) control.
- [DOCS] API contract: the as-built page now records the accuracy bound, the registration and anonymous-sign-up validation rules, the character-count convention, and that only a pending friend request can be answered.
- [FEATURE] Admin: both consoles can require or clear a password rotation without resetting the password; the admin user PATCH accepts `must_change_password` and audits the change.
- [FEATURE] Web admin: resetting a password now opens a masked form with a user picker and a clear-MFA checkbox instead of a browser prompt.
- [FEATURE] Native admin: the users list flags accounts that still need to change a temporary password and accounts with MFA enabled.
- [FEATURE] Native admin: create a user with an email, display name, role and temporary password; the account must change the password at first sign-in.
- [FEATURE] Native sharing: name a share with an optional label and see it in the active-shares list; editing an existing share keeps the label.
- [FEATURE] Web sharing: an optional label names a share (for example "Work trip") and appears in the shares list; the server already stored the field.
- [FEATURE] Native admin: issue a temporary password for another account (optionally clearing MFA); the reset revokes the account's sessions and forces a rotation at next sign-in.
- [FEATURE] Friends: the web list can cancel an outgoing request or remove an accepted friend, and native sent requests have a Cancel button; the server already allowed either participant to end the friendship.
- [FEATURE] Map: long-press a point to start a private note there; the Today form shows the pinned coordinates and falls back to the map centre when no point is pinned.
- [FEATURE] Admin console: the users table flags accounts that still need to change a temporary password.
- [FEATURE] Web: a same-origin app manifest (name, standalone display, theme colour, SVG icon) so the client can be installed to a home screen; no new third-party assets.
- [FEATURE] Admin console: create a user with an email, display name, role and temporary password; the account is forced to change that password at first sign-in.
- [FEATURE] Messages: keep up to eight previous peer public bundles locally (native keychain, web localStorage) and try them newest-first when verifying, so history stays readable after a friend changes device/key; the safety number still flags the change.
- [FEATURE] Native map: a 1-2-5 metric scale bar in the lower-left corner, derived from the Web Mercator metres-per-pixel at the current centre and zoom.
- [FEATURE] Messages: a dedicated `pinpoint-messages` service (port 8007) in the compose stack with a gateway route and a least-privilege `pinpoint_messages` database role; the combined `pinpoint-server` still mounts the same router.
- [FEATURE] Messages: compare and locally verify a friend's safety number on web and native; a changed fingerprint is flagged as Changed and asks for a new comparison. The record stays on the device (keychain/localStorage) and is never sent to the server.
- [FEATURE] Map: private note markers show their photo as a thumbnail (JPEG/PNG, owner-only), with the pencil glyph as fallback when a note has no photo. The service worker cache is now `pinpoint-offline-v8`.
- [FEATURE] E2E messages: 1:1 text between accepted friends with X25519 + HKDF-SHA256 + AES-256-GCM and Ed25519 signatures, a ciphertext-only server, safety number, unread counts and read receipts on web and native. This is an ECIES construction, not the Signal Protocol; see ADR-0023 and `docs/messaging.md`.
- [DOCS] Midterm 2 package: report and PDF, 13-slide deck and PDF, demo script, printable handout, pre-flight checklist, Q&A, admin runbook and evidence reviews under `docs/midterm2/`.
### Fixed
- [FIX] Distance labels: a folded zero (single-point measure) no longer renders as `-0 m`; the formatter clamps distances to zero.
- [FIX] Native: the change-password form counts characters instead of UTF-8 bytes, and the register form applies the server's password rule before sending, so a 4–7 emoji password is rejected in the form; a blank email now explains itself instead of doing nothing.
- [FIX] Web: the register, change-password and admin temporary-password forms count characters like the server instead of UTF-16 units, so a 4–7 emoji password is rejected before the request instead of by the server.
- [FIX] Web: the 5-second message poll now skips accounts with no friendships, so an idle friendless tab no longer sends two requests every 5 seconds; pending requests keep polling because the other side can accept.
- [FIX] Native sign-out: a session the server has already expired now signs out locally instead of reporting a failed logout, and a refresh that meets a 401 returns to the sign-in screen instead of leaving every action erroring.
- [FIX] Native admin: the reset-password button counts characters, not UTF-8 bytes, so a multibyte temporary password that meets the server's 8-character minimum is accepted.
- [FIX] Native: when a password change revokes every session, the client returns to the login screen instead of an empty map with a revoked session.
- [FIX] Admin: setting `must_change_password` through the user PATCH now persists to PostgreSQL, so the forced rotation survives a store refresh instead of reverting.
- [FIX] Native: the trip check-in and kiosk note fields stop at the 2000-character server limit.
- [FIX] Native: the message composer and both place-search fields stop at the documented limits (2000 and 120 characters).
- [FIX] Web admin: the create and reset password fields now stop at the server email, name and password limits.
- [FIX] Search: the shared query limit is 120 characters, matching the map proxy, so client validation and the server agree.
- [FIX] Clients stop accepting email, name, password, search and username input at the server limits, so oversized text is caught while typing instead of failing the request.
- [FIX] Validation: name, password and search limits now count characters instead of UTF-8 bytes, so non-Latin input is accepted at the documented limits.
- [FIX] Native admin: the temporary password is cleared from the reset field after a reset request, so it does not linger on screen.
- [FIX] The app-lock test no longer fails when the random Argon2id hash happens to contain the PIN digits; it parses the record and checks the PIN is only stored as a hash.
- [FIX] Auth: registration and login trim and case-fold email addresses, so a differently capitalized or padded address no longer creates a duplicate account or blocks sign-in.
- [FIX] Web sharing: clearing the label field now removes the stored label instead of silently keeping it.
- [FIX] Web: any authenticated 401 now returns to the login form immediately instead of leaving a signed-in shell full of failed actions.
- [FIX] Native account deletion now warns that the private trail, notes and messages are erased too.
- [FIX] Web: a temporary-password account skips the authenticated refresh and message polling, so the Account panel is not flooded with 403 responses while the password is rotated.
- [FIX] Web: message polling pauses while the tab is hidden, resumes on visibility, and never overlaps itself, so a background tab stops making requests every five seconds.
- [FIX] Web: the service worker is network-first for the app shell, so a deployment reaches installed clients on the next load instead of being pinned behind an unchanged worker; the cache is bumped to v9 and API calls bypass it entirely.
- [FIX] Native sharing: editing an existing share keeps its current expiry by default instead of extending a short share to 24 hours; an expired share still defaults to a fresh day.
- [FIX] Web: a legacy share whose expiry is beyond seven days no longer offers "Keep current" (the server cap rejects it); the form defaults to 24 hours.
- [FIX] Web: Escape closes the topmost place, sharing, route or chat sheet; the admin overlay and the sharing drawer keep priority.
- [FIX] Web: an account flagged for a temporary-password change now opens the Account panel with a prompt instead of failing every tab; the delete-account note lists the private trail, notes and messages it erases.
- [FIX] Web map: MapLibre zoom controls are 44x44 px and the search field is 40 px tall, meeting the mobile touch-target minimum.
- [FIX] Web: Escape closes the sharing drawer and returns focus to its toggle; the closed drawer stays out of the accessibility tree.
- [FIX] Messages: a native send failure now surfaces the error and restores the composer text instead of silently dropping the message.
- [FIX] Compose: a fresh Postgres volume no longer dies in `db/init/03-fks.sql` before the services create their tables; the FK script skips with a notice and is re-run with `02-grants.sql` after first boot.
- [FIX] Messages: native thread and inbox timestamps now render in the device's local time with the date for older messages instead of raw UTC; the web Chats tab shows the unread count.
- [FIX] Messages: the safety number is now a symmetric pair value (canonical user-id order plus both bundles), so both friends see the same string; old peer-only records show Changed and must be compared again.
- [FIX] Messages: the native thread polls while open so incoming messages appear without re-entering, the More tile shows the unread count, read state is written only when a friend's message is actually unread, and deleting a web account removes its local message identity.
- [FIX] Messages: first use in two web tabs now stores one identity transactionally instead of letting both tabs publish different bundles, which made one tab's messages undecryptable.
- [FIX] Native Today: action errors are surfaced, the note draft clears only after a successful POST and only if unchanged, refresh sections load independently, and a 401 clears private lists without letting an in-flight refresh cross accounts.
- [FIX] Location permission: Android now requests fine and coarse location in one dialog (Android 12+ ignores a fine-only request), asks on the first map view, retries the fix after a grant, and receives one-shot GPS updates through a broadcast receiver. The web client asks once after sign-in and reports the real geolocation error. Native map controls use a solid fill so their glyphs stay readable over the light basemap.
### Security
- [SECURITY] Split services: the auth, users, locations, map, messages, diagnostics and admin binaries now apply the same baseline headers and `no-store` policy as the combined router, so a split deployment cannot serve unprotected responses.
- [SECURITY] map: search, reverse-geocode and route answers now carry `Cache-Control: no-store` because they contain the query or coordinates; tiles keep their public one-hour cache.
- [SECURITY] rate limit: 429 responses now carry `Retry-After`, so a client that hits the auth, write or map throttle waits for the window instead of retrying in a tight loop.
- [SECURITY] API responses now carry `Cache-Control: no-store`, so a browser or intermediate cache on a shared device cannot replay another account's locations, notes or sessions; static assets keep their own cache headers.
- [SECURITY] Web: the CSP no longer allows inline styles; all CSS is same-origin or applied through the CSSOM, so `style-src` is `'self'` and the ZAP medium finding is gone.
- [SECURITY] Private note labels, messages and photos are AES-256-GCM encrypted at rest when `DATA_ENCRYPTION_KEY` is set, with legacy plaintext rows still readable and an invalid key refusing to start; ADR-0025.
- [SECURITY] Location publishes reject an accuracy outside 0 to 10,000,000 meters, matching the client validation; a crafted request can no longer store a negative or absurd precision.
- [SECURITY] Login and registration password/TOTP inputs are cleared on submit and after a successful sign-in, so credentials do not linger in hidden forms.
- [SECURITY] Dependencies: updated `cookie` to 0.18.3 for RUSTSEC-2026-0594 (private-cookie nonce reuse after fork); Pinpoint uses opaque session cookies, not the affected private-cookie API.
- [SECURITY] Registration now validates the email address (format, character count and display controls), matching the admin create path; malformed or bidi addresses are rejected.
- [SECURITY] Diagnostic app-version strings reject display controls and count characters, so a crafted version cannot reorder the admin diagnostics table; invalid values fall back to the server version.
- [SECURITY] A rejected friend request can no longer be flipped to accepted by the receiver; a fresh request from the sender is required.
- [SECURITY] Email validation counts characters and rejects Unicode bidi/control characters, so a crafted address cannot spoof the admin user list and multibyte addresses at the 254-character limit are accepted.
- [SECURITY] Recovery-code inputs are cleared on submit and cancel, so the one-time code does not linger in the web or native UI after a failed recovery.
- [SECURITY] Bookmark files are created owner-only (`0600`) on Unix and the app-data directory `0700`; the security docs now state the AES-GCM bookmark primitives are not wired by default instead of implying encryption at rest.
- [SECURITY] Anonymous sign-up validates the device name against the display-name rules and bounds the optional public key, so an anonymous account cannot store an oversized or spoofing name.
- [SECURITY] Diagnostic reports reject Unicode bidi controls as well as C0/C1, so a report cannot reorder the text an administrator reads in the diagnostics console.
- [SECURITY] Private-note labels reject control characters and messages reject Unicode bidi controls, so a crafted note cannot spoof the owner's note list or map labels; newlines remain allowed in messages.
- [SECURITY] Check-in notes are trimmed, capped at 2000 characters and reject control/bidi characters, so one request cannot store an arbitrarily large or spoofed note.
- [SECURITY] Display names and share labels reject Unicode bidi control characters, so a crafted name or label cannot visually reorder or spoof the text shown to friends.
- [SECURITY] Share labels reject control characters as well as the 120-character cap, so a label cannot inject line breaks or terminal escapes into the share lists.
- [SECURITY] Cached sessions are re-validated against PostgreSQL on every authenticated API request, so revocation on another replica (logout-all, password change, admin reset, deactivation) takes effect on the next request instead of waiting for the idle timeout; static assets skip the lookup.
- [SECURITY] Admins can issue a temporary password for another account (optionally clearing its MFA); the reset revokes every session, forces a rotation at next sign-in, and is recorded in the admin audit log.
- [SECURITY] The tile proxy rejects x/y coordinates outside the requested zoom level before contacting an upstream provider.
- [SECURITY] Display names are capped at 100 characters on registration, admin creation and profile updates, and share labels at 120, instead of accepting a body-sized string.
- [SECURITY] Admin user creation is recorded in the append-only admin audit log with the actor and created handle.
- [SECURITY] Deactivating an account revokes its sessions immediately, and the session middleware rejects a session whose account is inactive or missing, so a disabled user cannot keep using an old cookie.
- [SECURITY] HTTPS sessions use the `__Host-pinpoint_session` cookie (`Secure`, `Path=/`, no `Domain`), so a sibling subdomain cannot overwrite the session; loopback HTTP keeps `session_id` and logout clears both names.
- [SECURITY] Location history is pruned hourly by the locations service, so trail points older than the documented seven days are deleted even when the user stops publishing.
- [SECURITY] The public map proxy is capped per client (tiles 1200/min, search/reverse 60/min, route 120/min), protecting the upstream OSM providers from proxy abuse.
- [SECURITY] The bundled nginx gateway overwrites X-Forwarded-For with the direct client address, so a spoofed header cannot rotate per-client rate-limit keys.
- [SECURITY] Public diagnostic reports are capped at 10 per hour per client, closing the last unauthenticated public write endpoint.
- [SECURITY] Authenticated unsafe API requests are capped at 120 per minute per account (per replica), on top of the per-client auth throttles.
- [SECURITY] Login (30/5 min), registration (10/h) and anonymous/recovery (20/10 min) are throttled per client IP with a keyed HMAC; raw addresses are never stored, the deployment proxy must overwrite `X-Forwarded-For`, and the limiter is per replica.
- [SECURITY] Sessions also have an absolute 30-day ceiling (`PINPOINT_SESSION_MAX_DAYS`) on top of the sliding idle timeout, so a stolen cookie cannot live forever even if it is used regularly.
- [SECURITY] Expired session rows are pruned from PostgreSQL when a new session is created, so "until expiry" is a real deletion rather than only a rejected lookup.
- [SECURITY] Admin user creation now fails closed when PostgreSQL cannot persist the row and only accepts resident/admin/administrator roles, instead of returning an account that exists only in memory.
- [SECURITY] New location shares default to a 24-hour expiry on native and web (an existing "until off" share keeps its choice); the server rejects expiries in the past or beyond 7 days.
- [SECURITY] Admin-created accounts now write the forced password change to PostgreSQL with the row, so a restart or another replica cannot drop the flag and serve the temporary password as a normal account.
- [SECURITY] Every response now also carries `Permissions-Policy` (geolocation for this origin only; camera, microphone, payment and USB denied), `Cross-Origin-Opener-Policy` and `Cross-Origin-Resource-Policy` both `same-origin`, and `X-Permitted-Cross-Domain-Policies: none`.
- [SECURITY] Login performs the same Argon2 verification for unknown or inactive emails, so response timing no longer reveals whether an account exists.
- [SECURITY] Admin user creation no longer falls back to a shared default password: a password is required and the account is flagged `must_change_password` so the temporary credential must be rotated at first sign-in. Until it is rotated, the shared session middleware allows only password-change, logout, session and `/auth/me` routes.
- [SECURITY] Disabling MFA now requires a valid unused TOTP code through `POST /api/v1/auth/mfa/disable`; the cookie-only `DELETE /auth/mfa` route is removed, so a stolen session cannot remove the second factor.
- [FIX] `POST /api/v1/auth/anonymous` returns `409 ALREADY_SIGNED_IN` while a session is active, so repeated taps cannot create duplicate anonymous accounts or sessions.
- [SECURITY] Emergency alerts publish the latest point without reactivating paused shares; the point is written directly to PostgreSQL.
- [SECURITY] The session cookie is `Secure` on HTTPS, and every response (including API JSON and 401/403) now carries HSTS, `nosniff`, `X-Frame-Options: DENY` and `Referrer-Policy: no-referrer`.

## [2.6.1] - 2026-11-18
### Added
- [FEATURE] Native Today: the desktop/Android client now has a Today tab with the owner-only 24-hour trail list, the trail drawn on the map, a text-note composer at the map centre, and a private-notes list with delete. Photos remain available in the web client.

## [2.6.0] - 2026-11-18
### Added
- [FEATURE] Native UI: one shared dark theme from the web palette, a compact top bar with a scrollable screen tab row, filled/padded cards, 44 pt touch targets, and a map-first map screen shared by desktop and Android.
- [FEATURE] Today: a private 24-hour location trail recorded from your own published locations, shown as a list and a map line. Owner-only; no friend access.
- [FEATURE] Private notes: text and an optional JPEG/PNG photo attached to a place, stored on your account and shown as map markers. Owner-only; photo size is bounded and validated.
### Fixed
- [FIX] Web: the service worker no longer caches `/api/` or `/map/` GET responses, so shares, friend locations and the trail update immediately after a change instead of serving a stale cached reply.
- [FIX] Web: the app opens with a clear “Choose a map” prompt instead of a blank surface; the drawer is a solid, scrollable panel with the offline prompt inside it, and the mobile header no longer overflows.
 
## [2.5.2] - 2026-11-17
### Fixed
- [FIX] Friends: friend requests, accepts and removals now write directly to Postgres instead of a whole-store snapshot. On the multi-replica deployment a request could be acknowledged and then wiped before it was persisted, so the receiver never saw it.

## [2.5.1] - 2026-11-17
### Fixed
- [FIX] Web: the anonymous recovery code panel now sits above the fixed map shell, so the code and “I saved it” are visible and clickable on desktop and mobile.
- [FIX] Map: the local router resolves the catalog layout `packs/<region>/<region>.graph.bin` after an install (and still accepts the legacy root file), so `/map/route` works on a default map server without `PINPOINT_ROUTER_GRAPH`.
- [FIX] Sharing: exact shares keep full coordinates; create/update/delete write shares directly to Postgres instead of a whole-store snapshot, so a stale replica cannot resurrect a paused or revoked share.
- [FIX] Privacy: check-ins and emergency alerts require authentication and are owner-scoped; exact shares and the private-location paths now have regression tests.

## [2.5.0] - 2026-11-17
### Added
- [FEATURE] Web sharing: a deliberate consent sheet with a named/precision/timed summary and confirmation before granting or widening a share; the "Who can see me" list shows friend, precision, active/paused/expired state and expiry, with one-click revoke and a "Share again" path for expired shares; editing precision preserves the existing expiry.
- [FEATURE] Map: a "covers this view" hint in the Maps panel when the map center falls inside a catalog region that is not active, with Download (idle) or Show (installed).
### Fixed
- [FIX] Sharing: the server validates updates before mutating, rejects unknown precision/direction instead of falling through to exact coordinates, hides incoming `from_friend` rows from "Who can see me", and the consent tests cover owner-only PATCH/DELETE, expiry, invalid values and incoming-share filtering.

## [2.4.0] - 2026-11-17
### Added
- [FEATURE] Map data: a runtime region catalog (`catalog.json` in `pinpoint-maps`) and a manifest-driven multi-region installer replace the hardcoded Moldova pack, so a country can be added without an app release; GitHub Releases hosts assets (2 GiB cap, `maxzoom` reduced when needed), legacy root-level Moldova files migrate into `packs/moldova/`, and an opt-in OpenFreeMap basemap provides worldwide online viewing (ADR-0021).
- [FEATURE] Mobile/desktop map: a region picker lists catalog regions with transfer size and installed/update state, downloads one region at a time, serves `/packs/<region>/<file>` with Range requests, and shows the active pack from `/map/config`.
- [FEATURE] Data pipeline: `build.ps1` builds and gzips the native routing graph and records the source SHA-256/size; `catalog.ps1` merges schema-1 entries; `pinpoint-maps` publishes releases, `catalog.json`, a JSON Schema/validator and `build-region.yml` (weekly Moldova + Romania, Ukraine dispatch-only, `dry_run` support).
### Changed
- [FEATURE] Android WebView: the online tile cache is enabled (`LOAD_DEFAULT`) so OpenFreeMap's `max-age` works; loopback assets still revalidate.
### Fixed
- [FIX] Map data: catalog versions accept both `YYYYMMDD` and `YYYY-MM-DD`, the source SHA-256 stays in the manifest rather than the catalog, local builds publish `local:<filename>` instead of the build path, and the free-space check compiles cleanly on 64-bit Unix.

## [2.3.0] - 2026-11-17
### Added
- [FEATURE] UI: a cartographic forest/amber visual refresh — serif Pinpoint wordmark, a split auth field panel with coordinates and the "only shares when you do" line, a compact mobile auth sheet, consistent map-window header and route panel, and an amber route line.
### Changed
- [FEATURE] Mobile/desktop map: the routing graph is downloaded as `moldova.graph.bin.gz` (45,338,586 B) and decompressed while streaming, so the offline install transfers 139 MB instead of 204 MB while still installing the same 110,858,815-byte verified graph (204 MB on disk).
### Fixed
- [FIX] Offline demo: an install is published only after the map reopens and the pack list refreshes, and errors from a map that was already replaced are ignored. This removes the race that made the update/reopen smoke test flaky. The compressed-install progress test now holds the graph transfer open after its first chunk, so observing partial progress no longer depends on CI scheduling.

## [2.2.0] - 2026-11-17
### Added
- [FEATURE] Mobile/desktop map: the map window downloads the published Moldova PMTiles archive and routing graph into the app's `packs/` directory through a shared native installer; both artifacts are streamed, SHA-256 verified and atomically installed, `/map/config` points the page at `/packs/moldova.pmtiles` after install, both servers stream pack ranges, and the route panel uses the downloaded graph.

## [2.1.0] - 2026-11-17
### Added
- [FEATURE] Mobile/desktop map: local turn-by-turn routing panel over `/map/route`, backed by the on-device `native_router` graph cache (no online service or fallback).

## [2.0.1] - 2026-11-17
### Fixed
- [FIX] Registration: the user row is inserted into Postgres before the session is created, so a new account cannot exist only in a replica's memory and be dropped by a background refresh; a failed insert now returns an error instead of a false `201`.

## [2.0.0] - 2026-11-17
### Added
- [FEATURE] Offline map data pipeline and search CLI: `scripts/offline/` builds a versioned region pack (Planetiler/OpenMapTiles PMTiles basemap, `pinpoint-offline-index` SQLite FTS5 search index, Valhalla routing tiles, hashed manifest) and `pinpoint-offline-index` (`offline-build`) builds and verifies the search index.
- [FEATURE] Moldova offline pack: the 220 MB pack (93 MB PMTiles, 6.8 MB search index with 36,913 places, 120 MB Valhalla tar) is published as the non-latest `data-moldova-20261116` data release.
- [FEATURE] Web map foundation: vendored MapLibre GL JS 6.18.0 and PMTiles 4.5.0, a no-CDN `web/map/` module and preview, and the 94 KB ODbL Monaco fixture with a headless smoke test.
- [FEATURE] Web app map: `/map.js` is a thin adapter over the shared MapLibre + PMTiles foundation, with same-origin `/map-assets/*` and `/vendor/*` routes, byte-range fixture serving, visible OSM/OpenMapTiles/ODbL attribution and a headless adapter test. Leaflet was removed from the web app.
- [FEATURE] Offline web search: vendored SQLite WASM 3.54.1 with FTS5, a same-origin `web/map/search.js` API over the M1 `places`/`places_fts` index, the ODbL Bălți/Кишинёв fixture and an HTTP/Chromium smoke test.
- [FEATURE] Offline search UI component with debouncing, keyboard selection and accessible states.
- [FEATURE] Offline app search: `web/app/offline-search.js` uses the installed pack's SQLite FTS5 index through the service worker and falls back to the existing server search when no pack is installed.
- [FEATURE] Web offline packs: a same-origin pack catalog, manifest and SHA-256 verification, OPFS storage, a Range-capable service worker, download/update/delete UI, and a headless OPFS smoke test.
- [FEATURE] Same-origin pack proxy: `/offline/packs/moldova/manifest.json` rewrites file URLs to the server origin and `/offline/packs/moldova/<file>` streams the published release asset with `Range` passthrough, `Accept-Ranges` and CORS, because GitHub release assets send no CORS headers.
- [FEATURE] Route and turn-by-turn UI component with distance/time summary, maneuver banner, Start/Stop and Clear.
- [FEATURE] Local native routing: a Rust A* car router over the OSM extract with road names, 53 maneuvers on the Chișinău → Bălți route (134.1 km / 97.3 min), and a compact `graph.bin` cache (105.7 MiB, 0.265 s load).
- [FEATURE] Offline routing spike: a local-only Valhalla WASM harness, the Chișinău → Bălți route evidence and the web/MapLibre route API contract in `docs/offline/routing-spike.md`.
- [FEATURE] Desktop map window (opt-in `map-webview`): a loopback axum server with HTTP Range serves `web/map/` and `web/vendor/`, and a separate `wry` WebView process opens the same MapLibre page; the egui map stays as the fallback and a system-browser hand-off covers missing WebView2/WebKitGTK runtimes.
- [FEATURE] Android: the shared MapLibre page now runs in a full-screen WebView, served from a loopback-only Axum server with HTTP Range support, a loopback navigation allowlist and a click/share JavaScript bridge.
- [FEATURE] Admin bootstrap: `pinpoint-admin --promote-email <email>` promotes one existing active account to the admin role using `DATABASE_URL`, without creating or seeding a user.
### Changed
- [INFRA] CI gates the offline pipeline with a fixture-based `offline-checks` job (search-index tests, clippy, PowerShell script tests) on relevant PRs, keeps the manual Monaco pack smoke separate and records ADR-0020 for the MapLibre/PMTiles/Valhalla map subsystem.
- [INFRA] `scripts/v2/check.ps1` is the v2 release gate: Rust fmt/clippy/build/test, offline feature tests, the six offline-pipeline tests, native-router tests and every standalone web smoke test.
- [DOCS] `docs/offline/v2-release.md` records the integration, device and deployment checklist.
### Fixed
- [FIX] Offline packs build in a staging work directory, verify there, and only then replace the published five-file pack, so a failed run never publishes a manifest or partial artifacts; downloads and container images are hash/digest pinned and manifests carry publish URLs.
- [FIX] Search hardening: empty/truncated PBF rejection, punctuation-safe FTS queries, importance-weighted ranking and address-only nodes.
- [FIX] Android: the WebView now enables geolocation and requests fine/coarse location and notification permissions at runtime, so the system dialog appears and GPS works; denial shows a visible message instead of silently stopping the location service.
- [FIX] Offline pack fixture: pin `web/app/fixtures/offline-pack/LICENSE.txt` to LF so its 405 bytes and SHA-256 match the manifest on Windows checkouts; CRLF conversion previously made the real Monaco download fail verification.
- [FIX] Web CSP: the offline demo module is now an external same-origin script and `style-src` permits MapLibre's inline control styles, so the real server installs the pack and serves `206` ranges without CSP console errors.
## [1.5.3] - 2026-11-16
### Fixed
- [SECURITY] Account erasure: deleted user ids are tombstoned and filtered from every persist snapshot, so a concurrent background refresh can no longer upsert the account back after `DELETE FROM pinpoint_users` (1.5.2 removed the row, then the post-delete persist re-inserted it from a stale in-memory store).
- [FIX] `load_users` ignores tombstoned ids, so a stale normalized read cannot restore an erased account.

## [1.5.2] - 2026-11-16
### Fixed
- [SECURITY] Postgres account erasure: each cleanup statement now runs independently, so an append-only audit table without `DELETE` permission can no longer abort the transaction before the user row is removed. Audit entries are anonymised, sessions expired and reports detached when deletion is denied; a user row blocked by a foreign key is scrubbed and disabled.
- [FIX] Account deletion removes the legacy `pinpoint_state` JSONB rollback snapshot so a restart cannot restore an erased account.
- [FIX] Persistence no longer aborts when `pinpoint_user_id_seq` is missing; migrations 11/12 tolerate absent optional service roles and registration falls back to a direct insert.
- [FIX] Self-deletion no longer writes a new audit row after erasure.

## [1.5.1] - 2026-11-16
### Fixed
- [SECURITY] Locations: `/api/v1/locations/friends` now requires an accepted friendship and an active, non-expired share owned by the location owner. Removed the demo bypass that returned every latest location to any signed-in user.
- [FIX] Account deletion now deletes related shares, friendships, locations, check-ins, emergencies, diagnostics, sessions and audit rows in one transaction before the user row, so Postgres erasure actually completes.
- [FIX] Removed seeded demo accounts, the demo login hint and demo friend pins; docs now explain the real admin bootstrap.

## [1.5.0] - 2026-11-16
### Added
- [FEATURE] Public privacy policy at `/privacy`, linked from web sign-in and the Android About screen.
- [FEATURE] Self-service account deletion from Android and the web: typed `DELETE` confirmation, current-password proof for email accounts, session revocation and a full data cascade.
### Fixed
- [FIX] Android: anonymous sign-in is debounced while a request is in flight (partial #49).

## [1.4.2] - 2026-11-16
### Fixed
- [FIX] Android: a newly installed app version now performs one launch update check before the 24h throttle applies, so an upgrade no longer hides the new release.

## [1.4.1] - 2026-11-16
### Added
- [FEATURE] Android: release builds default to `https://pinpoint-uj4y.onrender.com`; Settings and the sign-in screen can change the server URL, with HTTPS enforcement and a forced sign-out/identity reset on change.

## [1.4.0] - 2026-11-16
### Added
- [FEATURE] Web account panel: edit the display name, change the password, manage active sessions and open the account area from the existing drawer.
- [FEATURE] Web crash capture: uncaught errors stay in memory and are sent only when the user presses **Send report**, with URLs and token-like fields redacted.
- [FEATURE] Web sign-in accepts an authenticator code for accounts with MFA enabled.

### Changed
- [DOCS] Moved 146 non-essential design, SDLC, UML, component, tutorial, project and thesis docs to the [GitHub wiki](https://github.com/Prince-of-Nothing/Pinpoint/wiki) as 150 structured pages. Build/setup/testing/deployment, ADR, security, privacy, OSM policy and API-contract docs remain in `docs/`; cross-links and Map QA were updated.

### Removed
- [INFRA] Removed the `archive/` tree (315 historical files) and its Docker/build ignore entries. The old Go/Python/Kotlin/web stacks remain available in git history and ADR-0007.

### Fixed
- [FIX] Auth: `POST /auth/password/change` now verifies the current password and rejects a missing or wrong value without revoking the session.
- [FIX] Web: anonymous sign-in ignores repeated clicks while a request is in flight (partial #49).

## [1.3.15] - 2026-11-16
### Added
- [FEATURE] Update dialog with the release notes for the new version, a direct **Download APK** button and the asset name/size. The GitHub Releases API supplies the real tag, release page URL, notes and assets; the Atom feed remains a fallback.
- [FEATURE] The updater selects the universal APK asset, so the app downloads the correct file without the user choosing between 32-bit and 64-bit builds.

### Fixed
- [FIX] Release builds are one universal APK containing both `arm64-v8a` and `armeabi-v7a` native libraries. This prevents 64-bit-only devices from accidentally installing the 32-bit asset and reporting a bad package.

## [1.3.14] - 2026-11-16
### Added
- [FEATURE] Read-only service status and discovery: `GET /status` (HTML) and `GET /api/v1/services` (JSON) in the combined server and every split binary. They report the version, service kind, database mode, component path prefixes and sanitized OSM/tile endpoints, with no secrets or user data (ADR-0016).
- [FEATURE] Android share-in restored with a small Java `PinpointActivity` bridge: cold-start and repeat `ACTION_SEND` text are delivered to the existing allowlisted deep-link parser instead of being silently ignored.
- [FEATURE] Android biometric unlock: optional Profile toggle, platform `BiometricPrompt` on API 28+, PIN fallback, persisted opt-in and no biometric data stored.
- [FEATURE] Android background location: user-started, non-exported foreground service with a persistent notification, 15–600 s interval, explicit stop control, background-location/notification permission flow, and publishing through the same Rust session and consent checks.

### Changed
- [INFRA] CI: every pull request now fails unless its body links an issue with `Closes`/`Fixes`/`Resolves`/`Refs #N`; the `skip-issue-link` label is the escape hatch for release/tooling PRs.
- [INFRA] Android APKs are packaged by a Gradle module under `android/` (Java bridge, DEX, service manifest, resources) while cargo-apk builds the Rust `.so`; release workflow zipaligns and signs the Gradle output. Requires JDK 21 for AGP 8.7 (ADR-0017).
- [DOCS] Tracker: every issue now has a native GitHub type, a `PREFIX-n` title ID, one `severity:*` and one `priority:*` label. Adds `docs/ISSUE_TAXONOMY.md`, `scripts/new-issue.ps1`, a task issue template, and prunes the redundant `feature`/`fix`/`docs`/`release`/`go` labels.

## [1.3.13] - 2026-11-15
### Fixed
- [FIX] Updates: launching an older build now checks GitHub at most once a day, shows a banner when a newer release exists, and links to it. The check can be disabled in Profile & settings; the manual About check remains. Adds `prefs.json` persistence (opt-out, dismissed tag, last check) and ADR-0015.

## [1.3.12] - 2026-11-15
### Added
- [FEATURE] Saved places: import a GeoJSON `FeatureCollection` from the Saved screen. Points are validated (finite, in range), non-Point geometries and malformed features are counted, duplicates against existing places and within the batch are skipped, names/notes/categories are preserved, one import is capped at 500 places, and map pins refresh immediately.

### Changed
- [INFRA] Every pull request must link an issue with `Closes #N` or `Refs #N`; documented in AGENTS/CONTRIBUTING and the PR template. The enforcing workflow is tracked in #29 pending the `workflow` token scope.

### Fixed
- [FIX] GeoJSON import writes once at the end instead of rewriting `bookmarks.json` per feature.

## [1.3.11] - 2026-11-15
### Security
- [SECURITY] Unix diagnostics: crash-report and log directories are created `0700` and files `0600`, so panic details, backtrace paths and log tails are not readable by other local users. Covered by Unix CI tests.

### Changed
- [DOCS] Process: every change now goes through a feature branch and pull request; `main` is never committed or pushed directly. Documented in `AGENTS.md`, `CONTRIBUTING.md`, the PR template and the commit helper guard.

## [1.3.10] - 2026-11-15
### Security
- [SECURITY] `platform::open_url` now allowlists only `mailto:`, `tel:` and `geo:`. HTTP/HTTPS, `file:`, `content:`, `intent:`, `javascript:` and `pinpoint:` are rejected and cannot launch arbitrary OS handlers; whitespace/control characters are rejected.

### Added
- [FEATURE] About → Troubleshooting shows the configured backend URL (userinfo redacted) and the number of pending crash reports; reports marked seen are kept but not counted.
- [INFRA] Docker: `scripts/docker-maintenance.ps1` prunes BuildKit cache and compacts Docker Desktop's WSL data disk. README documents why the VHDX does not shrink by itself after pruning.

### Fixed
- [FIX] Crash diagnostics count only `crash-*.txt` files, matching the pending-report logic.

## [1.3.9] - 2026-11-15
### Added
- [FEATURE] Map: tapping a saved-place pin opens its place sheet (Directions, Set as origin, Save, Copy, Share, map apps) without a network round-trip. The map centres on the bookmark at the current zoom and suppresses the duplicate pin label while it is selected.
- [FIX] Lock screen: the persisted lockout countdown is visually verified on the API 37 emulator (108s → 80s, Unlock disabled).

## [1.3.8] - 2026-11-15
### Fixed
- [SECURITY] PIN lock: the wrong-attempt count and lockout deadline are persisted in `lock.json`, so force-stopping the app no longer resets the retry delay. The remaining lockout is enforced at startup; a correct PIN clears the state; the lock screen repaints the countdown.
- [FIX] Lock state writes stay atomic and fail closed on a corrupt file; tests cover reload persistence, lockout expiry, failure accumulation and the startup deadline.

## [1.3.7] - 2026-11-15
### Added
- [FEATURE] Map: saved places render as blue pins with their names, under friend pins, and survive login/logout resets.
- [FEATURE] Saved places: each bookmark gains **Map** (centre the map there) and **Share** (`geo:` link) actions; the list refreshes map pins immediately after add/remove.

## [1.3.6] - 2026-11-14
### Removed
- [FIX] Android: remove the `ACTION_SEND` text-share target. Repeat shares were silently ignored while Pinpoint was alive because Android delivers them to `onNewIntent`, which the Rust-only `NativeActivity` cannot observe. Share-out (places/kiosk), `pinpoint://` and `geo:` deep links remain. `launchMode` stays `singleTop`; `standard` caused android-activity lifecycle errors and elevated threads. See ADR-0014.

## [1.3.5] - 2026-11-14
### Added
- [FEATURE] Android: `pinpoint://publish` (also `publish-gps`/`share-location`) publishes the current location on launch, waiting for a restored session before it does.
- [FEATURE] Android: `Use GPS` now actively requests a location when no provider has a recent last-known fix (8 s window, GPS → fused → network), then removes the request.

### Fixed
- [FIX] GPS: the publish action re-checks the fix after the active request and reports success in the device log; verified on the API 37 emulator against a local backend (`POST /api/v1/locations/me`).
- [FIX] Android debug builds can opt into screenshots for UI automation via `files/allow_screenshots`; release builds ignore the marker and keep FLAG_SECURE.

## [1.3.4] - 2026-11-14
### Added
- [FEATURE] Android: one-shot GPS. **Use GPS** checks the runtime location permission, asks for it when missing, then publishes the last known GPS or network fix (approximate grants included).
- [FEATURE] Android: share intents. Map links and coordinates shared from Maps, OSM, Apple Maps or chat apps open in Pinpoint; place and kiosk links gain a **Share** action that opens the system share sheet (desktop copies the link).
- [FEATURE] Privacy: optional on-device PIN app lock (Argon2id hash only). It locks on launch and on returning from the background, with a lockout after repeated wrong PINs.
- [FEATURE] Map: pinch-to-zoom on touch screens, accumulated into whole zoom levels.
- [FEATURE] Offline: the current-area pack now caches three zoom levels (out, current, in) instead of one, so the map stays usable when zooming offline.

### Fixed
- [FIX] Android: JNI now targets the live `NativeActivity`, not the Application context, so FLAG_SECURE, launch deep links, runtime permission requests and the share sheet actually run. Shared `ACTION_SEND` text opens the linked location.
- [FIX] Share/deep links: a linked coordinate now survives the first map render (zoom is initialized), so the map actually opens at the shared place. Repeat shares while Pinpoint is already running remain a documented Rust-only limitation.
- [FIX] GPS: only fixes newer than 10 minutes and with accuracy ≤5 km are published; providers are tried gps → fused → network → passive and one failing provider no longer hides the rest.
- [FIX] Lock: an unreadable `lock.json` fails closed, PIN changes use an atomic temp-file write with fsync, and existing Unix lock files are tightened to 0600.
- [FIX] Share parser: accepts map URLs surrounded by prose/trailing punctuation and `destination=` links; shared text can only open locations/kiosks, never `pinpoint://admin`.
- [FIX] Pinch zoom no longer pans with the first finger and no longer builds a zoom backlog at the 2/18 limits.

## [1.3.3] - 2026-11-14
### Added
- [FEATURE] Android: register and route `pinpoint://` and `geo:` launch intents, including kiosk token links and map-centre hand-off.
- [FEATURE] Kiosk: render a scannable QR for `pinpoint://kiosk?token=...` links.

### Security
- [SECURITY] Android: FLAG_SECURE blocks screenshots and screen recording of location data.
- [SECURITY] A release with no configured backend disables session restore and sign-in instead of sending credentials or a persisted session to the shared loopback port.

### Fixed
- [FIX] Android: report and log copies use the system `ClipboardManager`, so the no-email fallback leaves the full report on the real clipboard.
- [FIX] Android: reserve a bottom inset so the edge-to-edge gesture bar does not cover the mobile tab bar.
- [FIX] Diagnostics: redact Authorization/Bearer headers, JSON values, suffixed tokens and quoted coordinates; mailto re-redacts before encoding.
- [FIX] Logs: Clear log truncates the open writer, so subsequent events remain visible.
- [FIX] Crash prompt is marked seen after Email or Close, so it does not repeat on every launch.

## [1.3.2] - 2026-11-14
### Added
- [FEATURE] Crash reporting: Rust panics and `eframe::run_native` startup errors write a redacted app-private report; the next launch shows an editable dialog with Email/Copy/Discard/Close, and About/Profile expose a manual Report a problem flow.
- [FEATURE] Rolling native log under the app data directory, with 1 MiB rotation, copy and clear actions.
- [FEATURE] Troubleshooting card in About with device/page-size summary, log path, pending report access and an Offline badge on mobile.

### Fixed
- [FIX] Android release: a rejected default backend URL now opens the app with the loopback offline fallback and a visible Offline badge instead of panicking before the UI.
- [FIX] Updates: opening About no longer contacts GitHub automatically; the release check runs only when the user presses Check for updates.
- [FIX] WASM: keep the web stub compiling by stubbing the native URL opener.

## [1.3.1] - 2026-11-14
### Fixed
- [FIX] Android: align the arm64 native library to 16 KB so the APK loads on Android 15+ devices configured with 16 KB pages.

## [1.3.0] - 2026-11-14
### Added
- [FEATURE] Native map proxy: search, reverse geocoding and routing now call the Pinpoint backend with map-centre bias, stops, avoid classes, alternatives and capability gating.
- [FEATURE] Native route planner: place-based A/B, stops, avoid classes, swap, car/walk/bike, alternatives, step list, route sharing and KML/GeoJSON export.
- [FEATURE] Per-friend sharing sheet: existing-share lookup, enable/pause, precision, duration presets, incoming status and revoke; accepted friends are listed with Sharing/Remove actions.
- [FEATURE] Native route following: locate/self marker, 1 Hz fix polling, next-stop advancement and exit navigation.
- [FIX] Mobile locate no longer publishes a location; the publish sheet is reachable from the drawer and the drawer now has a refresh action.
- [FEATURE] Account lifecycle: anonymous sign-in, recovery code/recovery, keychain-backed session resume, active-session list/revoke and sign out everywhere.
- [FEATURE] Profile utilities: PII-free diagnostics report and map label/icon size sliders.
- [FEATURE] Admin console parity: overview counters, user search/role/active/delete, audit log and diagnostics triage; accepts `admin` and `administrator` roles.
- [FEATURE] ArtCraft-inspired mobile Tools sheet: colored tool tiles for map, friends, sharing, saved, trips, emergency, profile, updates, admin, kiosk and password, plus publish/refresh actions.
- [FEATURE] About & updates screen: version/build identity, user-initiated GitHub release check, semver comparison, release notes and release-page hand-off.
- [INFRA] Release workflow: `v*` tags build signed arm64/armv7 APKs, verify their certificates and create a GitHub release with the source archives.
- [FEATURE] mobile: Port webapp Kiosk check-in mode & Admin console to Android app and eframe UI, with deep links `pinpoint://kiosk?token=...` and `pinpoint://admin`.
- [FEATURE] Android APK: cargo-apk 0.10 build with NativeActivity/`android_main`, eframe `android-native-activity`, API 35 manifest metadata and launcher icons; runs on the API 37 x86_64 emulator.
- [FEATURE] Mobile map-first shell: web-loyal full-bleed dark map with floating search, zoom/locate/menu controls, bottom drawer, publish/place sheets, touch-sized controls, compact login/register forms, live tiles and scrollable locations.
- [FEATURE] Mobile map app polish: clean light OSM map by default with a light/dark toggle, category chips, bottom tab bar (Map/Friends/Share/Saved/Me), and a route polyline with A/B markers.
- [FEATURE] On-device tile cache/prefetch through `/map/tiles` under the Android app data dir.

### Changed
- [INFRA] Repository cleanup: untrack the archived MSVC/Windows SDK, downloaded installers, Go toolchain and `archive/app/target`; `.gitignore` now covers them plus Android signing material.

### Fixed
- [FIX] Auth: native login accepts an optional TOTP code, so MFA-enabled accounts can sign in instead of always receiving `MFA_REQUIRED`.
- [FIX] Admin navigation is hidden for residents and shown for `admin`/`administrator` only.
- [FIX] Mobile route sheet sits above the bottom tab bar and no longer clips its actions.
- [FIX] GitHub update check forces IPv4, matching the tile proxy, so it works on Android emulators with a broken IPv6 path.
- [FIX] CI: scope sqlx to Postgres-only (removing unused SQLite/MySQL advisories), refresh the lockfile to mio 1.2.5, and keep axum/sqlx/tokio out of the wasm stub build.
- [FIX] Tiles: force IPv4 for proxy/download requests and avoid the Docker gateway on 8000 by using `10.0.2.2:8080` for Android debug builds.

## [1.2.1] - 2026-11-14
### Security
- [SECURITY] **ZAP baseline hardening**: `Permissions-Policy`,
  `Cross-Origin-Embedder-Policy`, `Cross-Origin-Opener-Policy` and
  `Cross-Origin-Resource-Policy` are set on every web response. Re-scan is
  0 high / 0 medium / 0 low; raw evidence in
  `docs/evidence/zap-baseline.*` and the summary in
  `docs/evidence/zap-baseline-1.2.1.md`.
- [SECURITY] Static JS/CSS now use `Cache-Control: no-cache` (revalidate)
  while HTML keeps `no-store`.

### Fixed
- [FIX] OS-keychain tests skip cleanly when Windows returns
  `ERROR_NO_SUCH_LOGON_SESSION` (headless/sessionless runs) instead of
  failing the suite.

## [1.2.0] - 2026-11-13
### Added
- [FEATURE] **Headless Alice/Bob demo clients**: `pinpoint-demo-client` signs in
  through the API and publishes locations on an interval, so the demo runs the
  two accounts as real client containers (`docker compose --profile demo up
  alice bob`) instead of server-side seed rows. Adds an explicit exact-host
  `PINPOINT_ALLOW_PLAINTEXT_HOSTS` opt-in for internal container DNS; HTTPS
  remains the default.
- [FEATURE] **Diagnostics and audit split ports**: `pinpoint-diag` on 8005
  ingests PII-free, user-initiated reports (category + message + public
  handle only), and `pinpoint-admin` on 8006 serves the role-gated admin API.
  Migration 15 adds `pinpoint_diagnostic_reports`, append-only
  `pinpoint_admin_audit` and `users.created_at`; the admin console now has
  real Diagnostics (triage) and Audit modules instead of placeholders.
- [FEATURE] **Generated public usernames and private profiles**: every account
  gets a stable handle such as `calm-otter-42`; friends add each other by exact
  username, pending requests show handles only, and real names/emails are
  revealed to accepted friends. Numeric user IDs are no longer walkable and
  free-text user search is admin-only.

### Fixed
- [FIX] **DB bootstrap on Windows checkouts**: `db/init/01-roles.sh` is forced
  to LF (`*.sh text eol=lf`), so the container entrypoint no longer fails with
  `set: illegal option -` on a fresh volume.
- [FIX] **Real timestamps on publish**: location updates, emergency alerts,
  check-ins and friend requests use the current UTC clock instead of the seed
  date; friends see a truthful `updated_at`.
- [FIX] Duplicate friend requests are rejected; unknown/pending profiles no
  longer leak email or legal names.

### Infrastructure
- [INFRA] **Postgres verified**: migrations 1–15 applied against live Postgres
  16; a new user and location survived a restart; 8005 ingestion and 8006
  triage shared sessions and the report/audit tables.
- [INFRA] New binaries: `pinpoint-diag`, `pinpoint-admin`,
  `pinpoint-demo-client`; compose adds diag/admin services, gateway routes and
  the `demo` profile.

## [1.1.0] - 2026-11-13
### Added
- [FEATURE] **Export and hand-off to other maps**: place/route links for
  Google Maps and Organic Maps, Magic Earth `magicearth://` links
  (destination-only; multi-stop falls back to KML), plus KML and GeoJSON
  exports alongside GPX. All are user-initiated; no third-party scripts or
  tracking.
- [FEATURE] **Saved-place filters**: category chips derived from saved places
  (All / cafe / pharmacy / …) with a category badge per row.
- [FEATURE] **Per-account route history**: the last 20 routes (origin,
  destination, stops, mode, distance/time) are stored on-device under
  `pinpoint.v1.<userId>.routes`; click to reopen, clear separately from
  places. Route result lists use a delegated click handler so a fresh search
  result cannot lose its listener.
- [FEATURE] **Route panel drag/collapse**: drag the handle to reposition on
  desktop, or collapse to a compact summary; the map stays visible.
- [FEATURE] **Live next-stop ETA**: during Navigate, the next stop advances
  when within 60 m, completed nodes mark as done, and the remaining
  distance/time is recalculated from the current fix.
- [FEATURE] **Map appearance settings**: label size (10–18 px) and icon size
  (18–40 px) sliders, persisted on the device; pins, labels, tooltips and
  route markers follow the setting.
- [FEATURE] **Route progress and next-stop ETA**: A → numbered stops → B
  progress bar, next stop highlighted, and per-leg distance/time from OSRM
  (`legs` in the route JSON); route sharing copies a summary or uses the Web
  Share API, with GPX export as the fallback.
- [FEATURE] **Route options**: multi-stop routing (add stops by clicking the
  map, numbered markers, remove), GPX 1.1 export from the route panel, and
  avoid-motorway/toll/ferry controls. Avoid needs a self-hosted OSRM profile;
  `/map/capabilities` reports `avoid` and the UI disables the controls when
  the public demo cannot honour them.
- [FEATURE] **Per-friend sharing controls**: every accepted friend gets a
  “Sharing” sheet with an on/off switch, precision (`exact` / `approximate` /
  `city_only`) and duration presets (1 h, 8 h, 24 h, 7 days, until turned
  off); creating is an upsert per friendship+direction, and “Stop sharing”
  takes effect immediately.
- [FEATURE] **Modular admin console**: role-gated Admin tab in the web client
  with a module registry (Overview, Users, Diagnostics/Audit placeholders),
  `/api/v1/admin/overview|users|users/:id`, safe purge that revokes sessions
  and cascades friendships/shares/locations/check-ins/emergencies, and
  self-demotion/self-deletion guards. Extension guide:
  `docs/admin-console.md`.
- [FEATURE] **Web place search and A→B routing**: same-origin
  `/map/search`, `/map/reverse`, `/map/route` proxies over `src/osm.rs`
  (validation, LRU cache, 2-permit route semaphore, `alternatives=true`),
  with OSRM foot/bike profiles enabled only when
  `PINPOINT_OSRM_FOOT_URL` / `PINPOINT_OSRM_BIKE_URL` are configured
  (`/map/capabilities` reports what is available).
- [FEATURE] **Map-first web UI with locally vendored Leaflet 1.9.4**
  (BSD-2, served from `/vendor/`, no CDN): full-viewport map, dark tile
  treatment, friend pins, place sheet, route panel with alternatives and
  step list, scale bar and ODbL attribution.
- [FEATURE] **Search UX**: submit-on-Enter by default (Nominatim policy),
  opt-in 300 ms autocomplete with `PINPOINT_SEARCH_AUTOCOMPLETE=1` for a
  self-hosted Nominatim, category chips, distance-sorted results,
  “What’s here?” reverse geocoding on map click.
- [FEATURE] **Saved places and recent places** on-device, scoped per account
  (`pinpoint.v1.<userId>.savedPlaces|recents`), with clear-saved and
  clear-history actions.
- [FEATURE] **Locate me** (explicit button, browser permission only) and
  route follow mode using `watchPosition` (preview, no voice).
### Fixed
- [FIX] **Share expiry used a hardcoded seed date** (`2026-11-08`), so a share
  created for 1 hour never expired in the demo. Expiry now parses RFC 3339
  and compares against the real UTC clock; `duration_hours=0` means “until
  turned off”.
- [FIX] **Admin user list could be stale on a replica** started before new
  accounts were created elsewhere: `/api/v1/admin/overview` and
  `/api/v1/admin/users` refresh from Postgres before reading and label the
  view “all accounts, including anonymous and never-signed-in users”.
- [FIX] **User management was open to any signed-in account** (critical): the
  existing list/create/update/delete user endpoints did not check for an
  admin role, so a resident could raise their own role or delete accounts.
  They now require `admin`/`administrator`; self-role/active changes are
  refused and `/users/search` requires a session.
- [FIX] **Navigation Start failed when geolocation was blocked**: “Navigate”
  now always enters a route-preview banner (distance/time + first step) even
  without GPS, shows a precise blocked/unavailable/timeout message with an
  “Enable location” retry, and keeps the route drawn. Exit stops the watch and
  removes the self marker.
- [FIX] **No “you are here” marker after locating**: the locate button only
  flew the map to the GPS fix. It now draws a pulsing “You” marker plus a
  dashed accuracy circle (also updated during route follow mode) and clears
  it on logout.
- [FIX] **Cross-account saved-place flash**: switching accounts without a
  reload could briefly show the previous account’s saved places; the drawer
  is cleared on logout and re-rendered from the current account’s keys.
- [FIX] **Search/route coordinates in server logs**: `converge` now logs the
  path only (`loggable_uri`), so queries and coordinates never reach stderr.
### Security
- [SECURITY] Map endpoints accept no session/friend data, are cached by
  query/coords only, and redact query strings from logs.

## [1.0.0] - 2026-11-13
### Added
- Client platforms track (`docs/client-platforms.md`): Windows/Linux/macOS
  desktop via `src/main.rs` (`--help`, `$PINPOINT_BACKEND`, per-platform
  defaults in `platform.rs`), Linux `.deb`/`.desktop` + macOS `.app`
  metadata (`Cargo.toml`, `packaging/`), Android `cargo-apk` scaffold
  (`android/` incl. `network_security_config.xml`), iOS `cargo-xcodebuild`
  scaffold (`ios/Info.plist`), and a Rust/WASM web entry (`web/`,
  `src/bin/pinpoint-web.rs` via Trunk). Status is scaffolding-only for
  mobile/web — GUI verification needs NDK/Xcode/wasm toolchains or CI
  matrices on those OSes.
- OSM map parity (web + Android + Rust desktop): interactive Slippy web map
  with CARTO/Wikimedia layers, Nominatim search/reverse, OSRM routing
  (driving/foot/bike + GPX), Overpass POI browse (9 categories), on-device
  bookmarks (GeoJSON export); Android MapLibre with 3 OpenFreeMap styles,
  offline packs, Nominatim/OSRM/Overpass screens, DataStore bookmarks;
  Rust desktop OSM hand-off links fixed (OSM/directions/Organic Maps + ODbL credit)
- Full SDLC documentation set (`docs/sdlc/00-16`): BRD, FR/NFR/TRS, OSM
  integration guide (ODbL, Nominatim/OSRM/Overpass policies, self-hosting),
  architecture, security/privacy, QA, deployment, risks, traceability
- UML diagrams 12–15: map search/places, routing/navigation, offline/
  bookmarks, web Slippy engine (Mermaid, with FR traceability)
- Automatic QA: `scripts/qa.sh` + `scripts/qa.ps1` (tile policy, attribution,
  CSP sync, SW SHELL, Mermaid, notices) + CI `map-qa` job
- Rust desktop client (`pinpoint-desktop`, root crate): eframe/egui UI with
  stateful login/register/logout/password-change, `reqwest` API client bound to
  the server contract (session cookie + `X-CSRF-Token` double-submit), transport
  policy requiring HTTPS off-loopback (ADR-0006), and unit + API-contract tests
  (`tests/contract.rs`) covering fail-closed login/logout and CSRF placement
- CI `rust` job: `cargo fmt --check`, `clippy -D warnings`, locked build/test,
  and a `cargo-audit` dependency vulnerability audit
- CI `release` job (`windows-latest`): builds the `--release` binary, packages it
  into `pinpoint-desktop.zip`, and uploads it as a GitHub Actions artifact
- `docs/DEPLOYMENT.md` §4 Rust Desktop Deployment: build, run flags, ADR-0006
  transport policy, verified distributable artifact path, and demo credentials
- Kotlin/Jetpack Compose Android app scaffold (`mobile/`) — planned next
- `weba` admin console and `webk` kiosk check-in clients — planned
- Project governance docs: CONTRIBUTING, CODE_OF_CONDUCT, docs/{PROJECT, ROADMAP,
  ARCHITECTURE, SECURITY, PRIVACY, TESTING, DEPLOYMENT, GLOSSARY}, LICENSE template
- OS keychain integration (`src/security/keychain.rs`): unified `Keychain`
  trait with `OsKeychain` (Windows Credential Manager / macOS Keychain /
  Linux Secret Service via the `keyring` crate) and `FileKeychain`
  fallback. `KeyManager` now persists its active key through
  `Keychain::load_or_generate`; `PinpointApp::new` opens the
  best-available keychain via `keychain::open_default`. Closes the
  "OS keychain for bookmark keys" hardening item.
- Spiral execution plan (`docs/sdlc/17-…`): model statement, turns S0–S6
  (Foundation → OSM parity → Security → Reliability → Multiplatform/mobile →
  Verification → Deployment/report), four stages per turn (goal elaboration,
  risk analysis & prototyping, interim version, next-turn planning), exit
  criteria, pros/cons + mitigations, and the critical path to completion.
- Interactive Slippy tile canvas (`src/map.rs`, `src/tilepacks.rs`,
  `src/ui/map.rs`): Web Mercator projection (`latlon_to_world_px` /
  `world_px_to_latlon`), drag-to-pan, scroll/`+`/`-` zoom (`z2..=z18`),
  on-disk PNG tile texture rendering from `~/.pinpoint/tiles/{z}/{x}/{y}.png`,
  in-app **"Download current area pack"** button with multi-source ODbL
  fallback (CARTO → Wikimedia → CARTO Voyager), and clickable ODbL
  `https://www.openstreetmap.org/copyright` attribution.
- E2E location encryption + OS keychain publish wiring (`src/app.rs`):
  derives per-user `e2e_location_key` on login/register, persists it in the
  OS keychain, encrypts published locations with AES-256-GCM before sending,
  and wipes key material on logout.
- Real GPS publish + dual mobile apps (`src/platform.rs`, `src/mobile/mod.rs`,
  `mobile/`, `android/`, `ios/`): `EnvOrFileGpsProvider` (`PINPOINT_GPS_FIX`
  or `gps_fix.json`), **"📍 Use GPS"** button on the Map screen, `geo:` and
  `pinpoint://` mobile deep-link parser, `[package.metadata.android]` for
  `cargo-apk`, and the clean Kotlin/Jetpack Compose Android app in `mobile/`
  alongside the Rust mobile client.
- Bookmarks GeoJSON export/import (`src/bookmarks.rs`, `src/ui/bookmarks.rs`):
  RFC 7946 `FeatureCollection` export/import with fail-closed coordinate
  validation and one-click clipboard copy.
- Component documentation for **all architectures** (`docs/components/`):
  01 Go backend · 02 Python/FastAPI backend · 03 Rust/Axum backend ·
  04 Rust multiplatform client · 05 Kotlin Android app · 06 archived vanilla-JS
  web client — each standalone with stack, module map, API surface, data model,
  security controls, build/run/test, deployment, and cross-component relations.
- Competitive & similar-app analysis (`docs/components/07-competitive-analysis.md`):
  Google Maps LS · Apple Find My · Life360 · Glympse · WhatsApp/Telegram Live
  Location · Strava Beacon · Organic Maps · OsmAnd — feature matrix, consent/
  privacy model comparison, architecture patterns, gaps, and prioritised P1–P3
  recommendations (temporary share links, real-time streaming, battery-aware
  sampling, E2E bookmark sync). Referenced from `docs/sdlc/00-overview.md` and
  `docs/PRIVACY.md`; ROADMAP updated with the P2/P3 items.
- Dual authentication model (ADR-0009, **Accepted (partial)** +
  `docs/security/dual-auth-model.md`): anonymous device-bound accounts
  (no PII; one-time recovery code) alongside identity-linked accounts
  (email/OAuth/passkey), with an upgrade path and Sybil/abuse controls.
  Phases 1–2 implemented; OAuth/passkey/magic-link/link endpoints pending.
- **Session management decision + spec** (`docs/security/session-management.md`,
  ADR-0012): opaque 256-bit server-side sessions (`pinpoint_sessions`) shared
  across services, `HttpOnly; SameSite=Lax` cookie + session-bound CSRF
  double-submit, sliding idle timeout, immediate revocation on
  logout/logout-all/password-change/per-session revoke, `mfa_pending` gate,
  split-service sharing and in-memory fallback. The JWT + rotating-refresh
  design of ADR-0003 is superseded for transport (ADR-0012 keeps the password
  decision).
- **Authentication mechanisms reference** (`docs/security/auth-mechanisms.md`):
  TOTP (RFC 6238, HMAC-SHA256, ±1 step, replay-safe), OAuth2/OIDC + PKCE
  primitives, passkeys/magic-link designs, anonymous recovery codes, lockout
  and rate limiting, CSRF, audit, scraping detection, request signing, and a
  per-mechanism implementation-status table (TOTP + OAuth primitives
  implemented; passkeys/magic links/password reset pending).
- Auth/session **handlers landed**: `User` gains `auth_method` /
  `public_key` / `recovery_code_hash` / `identity_subject`; migration 14 adds
  the columns and `pinpoint_sessions`. The mock tokens are gone: login/register
  create a real session (HttpOnly cookie + CSRF), logout revokes it, password
  change revokes all sessions, and `session_auth` middleware validates the
  cookie + `X-CSRF-Token` on every request.
- **Session device management**: `GET /api/v1/auth/sessions` (8-hex prefix +
  `current` flag, full id never exposed), `DELETE /api/v1/auth/sessions/:id`
  (own sessions only, CSRF-protected), `POST /api/v1/auth/logout-all`; sliding
  `last_activity`, configurable idle timeout (`PINPOINT_SESSION_IDLE_MINUTES`,
  default 12 h demo), expired sessions deleted on use.
- **Anonymous accounts** (`POST /api/v1/auth/anonymous`,
  `/auth/anonymous/recover`): device-bound account with no PII, one-time
  160-bit recovery code shown once (SHA-256 stored, rotated on every
  recovery), same session/CSRF machinery; web UI adds "Continue anonymously",
  a recovery-code panel and a "Recover with code" form.
- **Web UI** (`web/app/`, served at `/`): login/register, anonymous sign-in,
  Slippy map via the server `/map/tiles` proxy, publish location, friends,
  shares, active-sessions card with revoke/sign-out-everywhere; CSP +
  security headers; ODbL attribution. Plain HTML/CSS/JS, no third-party
  scripts, no IP geolocation.
- **Tile proxy fix**: `tile.openstreetmap.de` (FOSSGIS) is the readable demo
  default with Wikimedia/CARTO fallbacks, an accepted User-Agent, one retry for
  rate-limited bursts, and a source-aware cache. Fixed a latent cache-key bug
  (`u8` truncated x/y at z ≥ 9 and could serve the wrong tile). `/map/tiles`
  returns 200 without a watermark; production self-hosts (ADR-0011).
- **Local-only web assets**: local `/favicon.svg` (no favicon service), CSP
  tightened to `style-src 'self'` and explicit `font-src 'self'` (the UI uses
  `system-ui`, so no font request is made at all), and `tests/assets.rs`
  guards against Google Fonts/CDN URLs or absolute subresources in the shipped
  HTML/CSS/JS.
- **Desktop tile packs via the backend**: `tilepacks::download_pack` now
  prefers `<backend>/map/tiles/{z}/{x}/{y}` before `PINPOINT_TILE_URL` and the
  public fallbacks, so desktop clients do not call a CDN directly when a
  Pinpoint server is configured.
- ADR-0009 status → **Accepted (partial)**; traceability matrix (15) rebuilt
  against the current Rust/Kotlin tree with FR-23…FR-30 auth/session/E2E/
  GPS/dual-mobile rows.
- **Database design** (`docs/database/schema.md`, ADR-0010, reference DDL
  `db/schema/00-target.sql`): normalized domain-owned tables (auth/social/
  location/audit), no single JSONB source of truth, identity columns instead
  of a counters table, application-level AES-256-GCM column encryption with
  `v1:` format + blind indexes (`email_hash`), one-way hashes for
  passwords/session ids/CSRF/recovery, per-service least-privilege roles, and an
  additive-modularity rulebook. Migration path documented (backfill →
  encrypt → retire `pinpoint_state`/`pinpoint_counters`).
- **Self-hosted OSM services** (ADR-0011, `docs/maps/self-hosted-osm.md`):
  `PINPOINT_NOMINATIM_URL`, `PINPOINT_OSRM_URL`, `PINPOINT_OVERPASS_URL`,
  `PINPOINT_TILE_URL` make every OSM endpoint self-host-first; public
  endpoints are demo fallbacks only. ODbL attribution stays mandatory
  (`openstreetmap.org/copyright` + `X-Data-Attribution`). Added `.env.example`
  with the OSM + security variables.
- **OSM parity quick wins** (`docs/maps/osm-feature-parity.md`): POI result
  list with opening hours/phone/website + Save/OSM buttons, full route-steps
  display, **GPX 1.1 export** for routes (`src/osm.rs::export_route_gpx`),
  and a Save button on search results.
### Fixed
- [FIX] **CSRF lost after reload** (`web/app/app.js`, `auth_handlers::me`): a
  reloaded page kept the `HttpOnly` session cookie but had no CSRF token, so
  the first publish/revoke returned `403 CSRF_FAILED`. `GET /auth/me` now
  returns `X-CSRF-Token` and the client re-arms it from any response.
- [FIX] **Session ids stored in plaintext** (`state/sessions.rs`): a database dump
  could have been replayed as cookies. Sessions are now persisted/kept as
  SHA-256 hashes; the raw 256-bit value exists only in the cookie.
- [FIX] **Tile cache key truncation** (`src/server/mod.rs`): `(u8,u8,u8)` wrapped
  x/y at z ≥ 9 and could serve a neighbouring tile; the key is now
  `(u32,u32,u32)` and cache hits report their `X-Tile-Source`.

### Changed
- [SECURITY] **Password hashing → Argon2id** (`src/security/password.rs`): new hashes are
  Argon2id PHC strings (`$argon2id$v=19$m=19456,t=2,p=1$…`, OWASP-aligned,
  memory-hard) via the `argon2` crate 0.6. Legacy `pbkdf2$…` hashes still
  verify and are transparently re-hashed to Argon2id on the next successful
  login (`needs_rehash`). `pbkdf2` remains only for legacy verification and
  E2E key derivation. Chosen over bcrypt (72-byte limit, recent
  RUSTSEC-2026-0199 DoS advisory).
- [INFRA] Rust desktop client: bumped `eframe` 0.30 → 0.32, moving the Linux
  accessibility stack off `quick-xml 0.30.0` (RUSTSEC-2026-0194,
  RUSTSEC-2026-0195, both high) to `0.41.0`; `cargo audit` now exits 0 with
  0 vulnerabilities (5 unmaintained warnings only)

## [Unreleased-redundancy]
### Added
- Client-side redundancy: `src/security/circuit_breaker.rs` (per-backend
  circuit breaker with Closed/HalfOpen/Open states), `src/backend/health.rs`
  (active health monitoring of backend URLs), `src/backend/failover.rs`
  (FailoverBackend wrapping multiple REST backends with automatic failover,
  circuit breakers, and exponential backoff retry), `FailoverV1`
  `BackendKind` + `PINPOINT_FAILOVER_URLS` env support
- Server reliability: `/health` verifies Postgres connectivity (returns
  `degraded` when DB unreachable); new `GET /api/v1/sync-status` endpoint
  reports replication freshness (`last_persist_at`, `last_persist_age_secs`,
  `fresh`) for monitoring replica health
- Client offline resilience: `PendingMutation` queue in `PinpointApp` — all
  mutation methods queue locally when all backends are unavailable;
  `sync_pending_mutations()` auto-replays queued mutations when connectivity
  returns; `pending_mutation_count()` / `pending_mutation_descriptions()`
  exposed for UI; offline mode auto-engaged on failure
- Map tile redundancy: `src/server/mod.rs` `map_handlers::tile` now tries
  multiple ODbL-compliant sources in order (CARTO with key → Wikimedia →
  OpenFreeMap) instead of failing closed at 502 when CARTO_API_KEY is missing.
  `X-Tile-Source` header reports which source served the tile. Attribution
  endpoint lists all configured sources.
- E2E encryption: `src/security/e2e.rs` provides AES-256-GCM end-to-end
  encryption for location data with PBKDF2-derived per-user keys. Location
  payloads are encrypted client-side before being sent to the server; the
  server stores ciphertext only. `derive_location_key`, `encrypt_location`,
  `decrypt_location` exported from `src/security/mod.rs`. `pbkdf2` made public
  in `src/security/password.rs`.

## [0.3.0] - 2026-10-13
### Added
- Docker deployment: `docker-compose.yml`, multi-stage `backend/Dockerfile` (non-root,
  healthcheck), `web/Dockerfile` + `web/nginx.conf` (SPA + `/api` reverse proxy)
- Idempotent demo seeding (`SEED_DEMO_DATA`): admin (forced password change), Alice & Bob
  with mutual shares and Paris locations
- Static file serving from the Go binary when `WEB_ROOT` is set (single-container option)
- `COOKIE_INSECURE` dev flag; `.env.example` documenting all settings
- Web SPA v1: login/CSRF flow, OSM map with friend pins, freshness/stale badges, precision
  badges, navigation export, location publishing, friend requests, directional share
  management, admin provisioning, forced-password-change UX

### Changed
- Mobile client decision: Flutter → **Kotlin (Android-first)** due to no macOS/iPhone
  development hardware; Android emulator covers the full test loop

## [0.2.0]
### Added
- Full Go backend feature parity: JWT + session auth, bcrypt, CSRF double-submit,
  rate limiting, lock/unlock, must-change-password gate
- Authorization + security test suite (`go test ./...` green, incl. IDOR fail-closed cases)

### Removed
- Entire legacy Python/FastAPI backend (refactor complete; no `.py` files remain)

## [0.1.0]
### Added
- Initial Python/FastAPI prototype (superseded by 0.2.0)
- Core domain model: users, friendships, directional share permissions, current location

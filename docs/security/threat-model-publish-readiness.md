# PINPOINT threat model: publish readiness

Date: 2026-11-27 · Status: draft for review

## Summary

Not ready to publish. The application-level controls are sound: Argon2id, opaque server-side sessions, fail-closed consent, and friend-only messages. The blockers sit at the deployment edge and in one Android deep link. Five findings should block publish (F-1 to F-5). They are small fixes, roughly 2 to 3 days of engineering in total.

**Scope:** the Rust client (egui, Android activity, loopback map server), the Docker stack (nginx gateway, auth, users, locations, messages, map, diag, admin, Postgres) and the CI and release workflows. **Method:** STRIDE per component, attacker goals, and evidence from the code on the checked-out tree on 27 Nov 2026. I read code and config. I did not run the stack, run a DAST scan, or audit vendored JavaScript. Items marked verify need a live check.

## Attacker goals

| Actor | Goal | Most likely route today |
|---|---|---|
| Stalker (former friend or someone with an old share) | Current location of one person | An old or forgotten share (T-I1 in the existing model). Not a code bug, but UX matters |
| Internet attacker | Database, accounts, sessions | Postgres port with default passwords (F-1) |
| Malicious app on the victim's phone | Trigger location publishes, tamper with offline packs | `pinpoint://publish` (F-4); loopback map server (F-7) |
| Network observer on public Wi-Fi | Session or admin cookie | HTTP default gateway (F-3); plain-HTTP admin (F-2) |
| Malicious or careless admin | Take over accounts | Admin password reset (F-9) |
| Supply-chain attacker | Poisoned map pack or routing graph | Unsigned offline catalog (F-8) |
| Harasser | Unwanted friend requests | Friend requests by exact handle; rate limits to verify |

## Findings

High = block publish. Medium = fix before launch. Low = track.

| ID | Sev | Finding | Evidence | Fix |
|---|---|---|---|---|
| F-1 | High | Postgres is published on host port 5432. Compose falls back to known passwords such as `pinpoint-auth-dev` when a `*_DB_PWD` variable is unset | `docker-compose.yml:66-67`, `:49-53`, `:112`, `:144`, `:176`, `:209`, `:271`, `:301` | Remove the db port mapping. Make each `*_DB_PWD` required with no default, and refuse to start without it |
| F-2 | High | Admin API (8006) and diagnostics (8005) are published on the host, on plain HTTP, bound to `0.0.0.0` by default. They skip the gateway, its TLS, its headers and its proxy-level limits | `docker-compose.yml:283-284`, `:313-314`; `src/bin/admin.rs:57`; `src/bin/diag.rs:4` | Remove the host ports. Serve admin only through the TLS gateway. Keep both on the internal network |
| F-3 | High | The default gateway publishes HTTP on all interfaces (`8000:80`). HTTPS and HSTS exist only in the optional HTTPS config | `docker-compose.yml:353-354`; `docker/nginx-gateway.conf`; `docker/nginx-gateway-https.conf:17` | Make HTTPS the default, redirect 80 to 443, send HSTS in every config. Check that `session_cookie(…, secure)` is called with `true` in every deployed config (verify) |
| F-4 | High | `pinpoint://publish` on the exported `PinpointActivity` starts a GPS publish as soon as the user is signed in. There is no confirmation. Any installed app can send the intent | `android/app/src/main/AndroidManifest.xml` (exported activity, custom scheme); `src/mobile/mod.rs:413-417`; `src/ui.rs:1076-1078`, `:2500-2507` | Remove the publish deep link, or open a confirmation sheet first. Add a test that the link does not publish without a tap |
| F-5 | High | No edge rate limiting. The gateway has no `limit_req`. App limits exist (login 30 per 5 min per client, 120 writes per minute per account), but direct admin and diag ports skip the gateway | `docker/nginx-gateway*.conf` (no `limit_req`); `src/server/state/rate_limit.rs:99-110`; `DIAGNOSTIC_LIMIT` exists, verify it is applied to diagnostics submit | Add `limit_req` zones for `/api/v1/auth/` and `/api/v1/diagnostics/`. Close the direct ports (F-2) |
| F-6 | Medium | Registration reveals whether an email exists (`email already registered`). Login is safe: it returns a generic error and runs a dummy Argon2 check | `src/server/auth_handlers.rs:271`; `src/server/users_handlers.rs:100` | Give the same response for new and existing emails. Send the existing address a notice |
| F-7 | Medium | The loopback map server has no token or Origin check. It serves `POST /map/offline/download`, and on Android `DELETE /map/offline/{region}`. Any app on the device can reach 127.0.0.1 | `src/map_server.rs:163-175`; `src/mobile/map_server.rs:260-281` | Use a random per-launch secret in the WebView path. Check Host and Origin. Allow mutating routes only from the app's own WebView |
| F-8 | Medium | The offline pack catalog is unsigned. Its hashes come from the same catalog. A signed catalog is explicitly deferred. Pack data is parsed by native code | `docs/offline/catalog-security.md:48`, `:101`; `src/offline_install.rs` (SHA-256 checks present) | Ed25519-signed manifest with a pinned key, before offline packs ship in the store build |
| F-9 | Medium | Admins can set any user's password. The admin UI calls `PATCH /api/v1/admin/users/{id}/password`. One admin can take over an account without the user knowing | `web/app/admin/users.js:245`; `src/server/admin_handlers.rs:35` (`require_admin`) | Notify the user. Require a second admin or the user's own reset. Treat password reset as a separate, audited permission |
| F-10 | Medium | The file keychain fallback stores its master key in `dir/master.key`, next to the encrypted entries. Anyone who reads the app-data folder gets both | `src/security/keychain.rs:203-227` | Keep the master key in Android Keystore and Windows DPAPI. Fail closed when neither is available |
| F-11 | Medium | Message key trust. The server hands out public bundles. Safety numbers help, but a changed key showing a warning is not confirmed (verify) | `src/server/messages_handlers.rs:113-130`; `src/security/message_crypto.rs` | Show a `key changed` banner in the thread and require re-verification |
| F-12 | Low | `GET /` and `GET /registry` on the gateway list internal upstream names and ports | `docker/nginx-gateway.conf:138`, `:155` | Return a fixed body without upstream details |
| F-13 | Low | Idle session timeout is 12 hours (`PINPOINT_SESSION_IDLE_MINUTES=720`), with a 30-day ceiling. Long for a location app | `.env.example` | Consider 2 hours idle. Re-authenticate before share changes |
| F-14 | Low | `SECRET_KEY` and `JWT_SECRET` in `.env.example` are not read anywhere in `src`, `db` or `docker` | `.env.example` | Remove them, so operators do not set secrets that do nothing |
| F-15 | Low | The map page has an inline script, which blocks a strict CSP. The innerHTML sites sampled escape their values, but the review was partial | `web/map/index.html`; `web/app/*.js` | Move the script to a file. Add a strict CSP. Finish the DOM-sink review |
| F-16 | Low | `web/map/fixtures/balti-search.sqlite` is tracked in git. `audit.toml` ignores 9 advisories | repository | Confirm the fixture holds no sensitive data. Re-check the advisories before release |
| F-17 | Low | `src/security/e2e.rs` describes a location key derived from the password. Verify whether it is wired in | `src/security/e2e.rs:8`, `:28` | Resolved: the location cipher was never read, so it was removed and the docs now say locations are not E2E |

## Controls that hold (checked in code)

- Argon2id password hashing, with a dummy hash for unknown emails (`src/security/password.rs`, `auth_handlers.rs:143-148`).
- Account lockout (`locked_until`, `auth_handlers.rs:155-191`).
- Opaque session cookies: HttpOnly, SameSite=Lax, and the `__Host-` prefix with Secure on HTTPS (`auth_handlers.rs:36-52`).
- CSRF double-submit on unsafe methods (`src/server/mod.rs:137-145`).
- Fail-closed consent. An accepted friendship alone grants nothing. A share must be active, unexpired, and at a set precision (`locations_handlers.rs:41-57`). Precision is rounded on the server. Expiry is capped at 7 days (`:19-28`).
- Messages only between accepted friends (`messages_handlers.rs:123`, `:158`, `:220`). Sends are rate limited.
- The gateway overwrites `X-Forwarded-For` and sends `nosniff`, `X-Frame-Options: DENY` and `default-src 'none'` on the API (`docker/nginx-gateway.conf:25-37`).
- Services run as non-root user `pinpoint` (`Dockerfile.service:49`). Service ports are `expose`-only, except db, diag and admin (F-1, F-2).
- Diagnostic reports carry no IP, email or coordinates. A unit test checks this (`diagnostic_handlers.rs:288`).
- Android: `allowBackup=false`, `usesCleartextTraffic=false`. The release network config allows cleartext only to loopback.
- Crash reports stay on the device (`src/crash.rs`).
- CI uses `pull_request`, not `pull_request_target`. It runs clippy with `-D warnings`, CodeQL and a ZAP baseline. Signing secrets appear only in the release workflow (`release.yml:87-143`). `.env` is git-ignored, and `.env.example` holds no real secrets.

## Threats by component

| Component | Main threats | Findings |
|---|---|---|
| Native and Android client | Any app triggers a publish. Loopback tampering. Plaintext key material on fallback platforms | F-4, F-7, F-10 |
| Gateway | HTTP sessions. Unthrottled auth. Topology disclosure | F-3, F-5, F-12 |
| Auth | Email enumeration at registration | F-6 |
| Users and friendships | Friend-request spam (verify the limit) | Not yet rated |
| Locations and shares | Stale or forgotten shares (UX). Consent holds in code | Controls hold |
| Messages | Key substitution by the server. Ciphertext only otherwise | F-11 |
| Admin | Plain HTTP access. Password reset as takeover | F-2, F-9 |
| Diagnostics | Anonymous ingestion, flooding | F-2, F-5 |
| Postgres | Default passwords, published port | F-1 |
| Offline packs | Unsigned catalog | F-8 |

## Publish blockers outside the code

- **Google Play background location.** The manifest requests `ACCESS_BACKGROUND_LOCATION` and `FOREGROUND_SERVICE_LOCATION`. Play requires a declaration and a demo video. Plan for that review.
- **Privacy policy and terms.** `docs/PRIVACY.md` exists. Check it against the real flows: OSM requests, the GitHub release check, and diagnostics.
- **Release integrity.** Publish SHA-256 sums for every artifact. The update check (`src/update.rs`) only links to GitHub and does not install anything. Keep it that way, or verify a signature before any auto-update.
- **Admin accounts.** Require MFA for any account with the admin role. TOTP exists (`src/security/mfa.rs`), so the check is a policy decision.

## Stale docs that contradict the code

- `docs/security/threat-model.md` sections 6 and 7 call MFA and E2E messages deferred, and say the server sees plaintext messages. TOTP (`mfa.rs`) and E2E messages (`message_crypto.rs`) exist.
- `SECURITY.md` says passwords use PBKDF2 (the code uses Argon2id). It says migrations v1 to v12 (README says 1 to 17). It lists E2E as a known limitation.
- The README lists GitHub OAuth under what works, and also under still pending.
- `.env.example` lists the unused `SECRET_KEY` and `JWT_SECRET`.

## Suggested order

1. **Before publish (blockers):** F-1 to F-5. Add a CI test that parses `docker-compose.yml` and fails if db, admin or diag has a host port, or if any `*_DB_PWD` has a default.
2. **Before launch:** F-6 to F-11. F-6 and F-9 need product decisions.
3. **Docs and low items:** refresh the threat model and SECURITY.md, then F-12 to F-17.

## Not checked

- The running stack, DAST results, the Windows release binary, vendored JavaScript (`web/vendor`), and every route.
- Whether `secure` is set on HTTP deployments (F-3, verify).
- Whether diagnostics submit is rate limited (F-5, verify).
- OAuth flow details beyond the state and PKCE helpers.

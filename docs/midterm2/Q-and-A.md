# Pinpoint - Q&A bank (Midterm 2, v2.6.1)

Short answers to the 20 most likely assessor questions. The labels keep the
evidence honest:

- **Verified** - checked in the repository or against the live service for this
  review.
- **Reported** - claimed in `Midterm2-Report.md`; not re-run here (live
  two-account run, browser/offline checks, APK signature).
- **Not built / not done** - a known gap, not a hidden feature.
- **Blocked** - cannot be demonstrated in this environment.

Do not upgrade a "reported" number to "verified" in the talk.

---

## 1. What problem does Pinpoint solve?

Most location-sharing products make friendship permanent access, show a
last-known dot as if it were live, and bury pause/revoke/precision. Pinpoint
makes each share explicit and directional, with a chosen precision and optional
expiry, and the server re-checks it on every read. The target is trusted-friend
safety, not public discovery or a social feed.

## 2. Why Rust?

The API and the native clients share one Rust core: typed models and one wire
contract, memory safety without a garbage collector, and one async HTTP service
(Axum). The desktop and Android clients reuse the same core through
eframe/egui. The **web client is deliberately vanilla ES modules**, not
Rust/WASM; MapLibre and PMTiles already do the browser-heavy work and the client
is served same-origin. Rust removes memory-safety classes of bugs; it does not
replace the explicit authorization checks.

## 3. How is consent enforced, not just hidden in the UI?

`GET /api/v1/locations/friends` calls `visible_share`, which requires all of:
an accepted friendship; a share owned by the location owner; `is_active`;
direction not `from_friend`; a known precision; and an unexpired `expires_at`.
Only then is the coordinate returned, and it is rounded by
`round_for_precision`. Pause, resume, edit and delete are owner-only; a foreign
or missing share returns the same not-found envelope. A modified client cannot
widen any of this because the check runs server-side on every read.

## 4. Can a friend see my location just because we are friends?

No. Friendship is a prerequisite, not consent. With no share, the friend
location query returns no row. `tests/location_consent.rs::friend_locations_require_an_active_share`
pins this, together with exact precision, pause/revoke ownership and expiry.

## 5. What happens when a share expires or is paused mid-session?

The next read recomputes `is_active` and `expires_at` and returns no location
immediately; there is no grace period or cached coordinate. The web list shows
the share as expired and offers "Share again". Pausing preserves the original
expiry and does not rewrite precision.

One consent edge worth knowing: triggering an **emergency alert** reactivates
the owner's existing shares, including paused ones (deleted shares stay gone).
The owner starts it, and the UI says alerts are visible to every accepted
friend; a stricter design would require per-share confirmation.

## 6. How is precision enforced? Can a client widen it?

Precision is stored on the share and applied on read: `exact` is full
precision, `approximate` rounds to 2 decimal places, and `city_only` rounds to
1 decimal place. Create and PATCH reject unknown levels instead of falling
through to exact; unknown values fail closed on read. The recipient only
receives the rounded coordinate, so a client-side change cannot recover the
original.

## 7. Is the Today trail background tracking?

No. It is the owner's list of explicit publishes (manual coordinates or a
single GPS fix) from the last 24 hours, newest first, drawn on the map. Postgres
keeps 7 days and prunes older rows on the next publish. There is no 24/7
background GPS and no background upload.

## 8. How private are the note photos?

The web client accepts JPEG/PNG up to 8 MB, downscales the longest side to
1024 px, re-encodes JPEG at quality 0.82, and sends a data URL. The server
accepts only `data:image/jpeg`/`data:image/png` and caps the string at 900,000
characters (about 675 KB decoded). The value is stored in
`pinpoint_place_notes.photo`; list and delete queries are filtered by
`user_id`. Friends and the admin UI do not receive notes.

Important limit: photos are **not end-to-end encrypted and not encrypted at
rest in the current runtime**. A database operator or backup can read them.
ADR-0010's application-level AES-256-GCM is an accepted design target, not
implemented. Do not claim "only I can ever read it".

## 9. How does account deletion work?

The user types `DELETE` and, for password accounts, re-enters the current
password. In Postgres mode the deletion path removes the user, sessions,
friendships, shares, latest location, trail, notes, check-ins, emergencies and
diagnostics, then clears the session cookie. `tests/account_delete.rs` and the
reported live 14/14 run cover this.

## 10. How do multiple replicas converge? Is it correct?

PostgreSQL is authoritative; the in-memory `Store` is a view. A `converge`
middleware refreshes from Postgres before every request and persists after
every non-GET. Security-sensitive rows (users, sessions, friendships, shares,
latest location, trail, notes) are written with direct single-row SQL/upserts,
so a stale snapshot cannot resurrect a revoked share. This gives serial
read-after-write behaviour at demo scale; concurrent writes from two replicas
are not serialized and there is no conflict-resolution layer.

Be honest about the limit: it is a whole-store refresh, not a distributed
protocol. There is no formal linearizability claim and no load test.
`/api/v1/sync-status` reports last-persist age. The code comment records the
pre-fix 2-replica e2e at 29/38; the current model is sized for the demo and
small trusted groups, not a global workload.

## 11. What breaks at scale?

The per-request full-store refresh and the in-memory mutex are the main costs:
every request becomes Postgres work, and every mutation persists a snapshot.
The native routing graph is about 106 MB and serialises A* queries. There is no
load test, no read-replica/cache layer and no queue. The modular monolith is
deliberate for the thesis and can run split binaries (auth, users, locations,
map, diagnostics, admin), but that is not a scaling proof. The next step is
targeted queries plus a shared session/rate-limit cache, then a two-replica
load test.

## 12. What happens if PostgreSQL is unavailable?

If `DATABASE_URL` is unset, in-memory mode is intentional for local demo. If it
is set but the connection fails, the current startup logs a warning and starts
in-memory ("data is not persisted") instead of failing closed. That is a
deployment risk, not a feature: a production build should fail fast, and the
multi-replica story assumes Postgres is up. No data-loss test covers this path.
Mark it as a known gap.

## 13. How do offline maps work, and can the web client route offline?

The web client downloads the Moldova pack (PMTiles basemap plus routing graph)
into OPFS, verifies SHA-256 per file, and a service worker serves pack bytes
with HTTP Range requests. The map and offline search work with the network off.
Web directions always call `/map/route` (OSRM through the server), so
**offline web routing is not built**. The native map server has `/map/route`
backed by `native_router::Graph` from `packs/moldova.graph.bin`; that path
routes locally. Privacy caveat: a user-initiated route to a friend's pin sends
that endpoint to the router; search queries never carry friend coordinates.

## 14. Why can't the native client add photos?

There is no photo picker or note-photo decode path wired into the egui build.
Native notes send text only; when a photo exists the list says "Photo attached
(view it in the web client)." The web client is the only photo path in v2.6.1.
A mobile picker plus image decode is next work, not a hidden feature.

## 15. Why is there no admin demo? How is the first admin created?

Admin routes are role-gated and there is no seeded admin or HTTP bootstrap. An
existing active account is promoted offline:

```powershell
$env:DATABASE_URL = "postgres://..."
cargo run --bin pinpoint-admin -- --promote-email owner@example.com
```

Then the user signs out and in again. The Users table contains real emails, so
it is not projected. The overview/audit UI is implemented and audit-logged, but
a live admin walkthrough is **blocked in this review** because no promoted
account or deployment database was available. This is an ops gap, not a
missing console.

## 16. What are the MFA gaps?

TOTP (RFC 6238, HMAC-SHA256) is implemented end-to-end: enroll/verify/disable
endpoints, a login gate for enrolled accounts, and an `mfa_last_step` replay
guard. The desktop client has an enrolment card; the web client only has the
login code field, so **web enrolment is not built**. Email accounts have no
recovery codes, no email password reset, and there is no WebAuthn/passkey
option; admin is not forced to enrol. "MFA login works" is true; "MFA is
complete" is not.

## 17. How did you test security and correctness?

**Verified in this review:** live `/health` reports `2.6.1` with PostgreSQL
connected; `cargo test --locked` passes with **328 library tests**;
`location_consent` 10, `today` 2, `contract` 11, `assets` 11, `router_probe`
17, plus account-delete/auth-password. The asset suite pins
CSP/same-origin/service-worker rules; the contract pins CSRF and the wire
envelope; CI runs `cargo-audit`. Three integration tests are opt-in and
ignored without env (live API, offline e2e, route e2e).

**Reported, not re-run here:** live two-account **14/14** functional checks on
v2.6.1 (one script assertion expected the old 2.6.0 string), UI 9/9, offline
7/7, signed APK. **Not done:** external penetration test, DAST/fuzzing, formal
threat-model validation, load test. Findings from this review: the `Set-Cookie`
omits the `Secure` attribute (the report records this as a hardening gap), HSTS
is absent on the live deployment, API JSON responses do not get the web security
headers, and the Axum server has no general rate limiter wired in - only the
per-account login lockout (5 failures, 15 minutes). Treat these as
pre-production fixes.

## 18. Do you verify email addresses or support password reset?

No email verification flow and no email-based password reset in v2.6.1. Email
is the login identifier, not proof of ownership. Anonymous accounts get a
one-time recovery code; email accounts do not. Login still requires the
Argon2id password (and TOTP if enrolled), so guessing an email does not grant
access - but a lost password is unrecoverable. This is a real production gap
and is listed as next work.

## 19. Is this production-ready? What is the biggest risk?

No. It is a verified demonstration/small-group build. The biggest risks are:

1. the demo-scale convergence model and hard dependency on Postgres;
2. sensitive columns, including note photos and locations, are plaintext at
   rest (ADR-0010 is a target);
3. missing admin bootstrap, web MFA enrolment, email verification/recovery, a
   general server-side rate limiter, and HSTS/Secure-cookie hardening;
4. no load test or external security test.

The consent model itself is the strongest part and is server-enforced. I would
not put real sensitive location data on this deployment without that ops and
security work.

## 20. What did you personally build, and what is third-party?

The application code in this repository: the Rust/Axum API, PostgreSQL
schema/migrations, consent/precision/share model, web client (vanilla ES
modules), desktop/Android clients, offline pack install/search, tests and
deployment wiring. Third-party: Axum, egui/eframe, MapLibre/PMTiles,
OpenFreeMap, Nominatim/OSRM, OSM data and the pack tooling.

Ownership note: the repository history has multiple authors (Pleșu Dinu,
Mordret, Andrei Cliucovschi and others). This answer describes the submission
you can walk through, not a claim to every commit; check `git shortlog` before
claiming a specific one. Edit this answer to match your actual contribution.

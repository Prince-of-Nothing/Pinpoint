# Pinpoint 1.5.3 - Release Notes

**Released:** 2026-11-16 - **Version:** `1.5.3` (from `1.5.2`) -
**Status:** Final account-erasure patch. Use this build for launch.

**Artifacts:** `pinpoint-android-1.5.3.apk` - **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.5.3` tag.

## The bug 1.5.2 missed

The live log showed `delete user 6: user rows=1`, then `CONVERGE persisted`,
then a successful login with the erased account. The SQL delete worked, but the
request's post-handler persist cloned a store that a concurrent refresh had
reloaded just before the delete committed. Its `ON CONFLICT DO UPDATE` upsert
re-inserted the account from that stale snapshot.

## Fix

- Tombstone every id passed to `sql_delete_user` before touching Postgres.
- Filter tombstoned users and all related rows (friendships, shares, locations,
  check-ins, emergencies, diagnostic reports and audit entries) out of every
  persist snapshot, including `pinpoint_state`, before any INSERT/upsert.
- `load_users` also ignores tombstoned ids, so a stale read cannot repopulate
  the in-memory store.
- Added a unit test covering all eight store domains.

## Verification

- Full gate: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build --locked`, `cargo test --locked` all pass.
- Live test after deploy: delete returns success, login returns `401`, and the
  row stays absent after multiple refresh cycles.

## Upgrade

Install over 1.5.2. The Android client is unchanged; the web app is fixed as
soon as Render deploys 1.5.3.

## Known issues

- #49 server-side anonymous session scoping remains open.
- #50 MFA disable does not require a code yet.
- #58 theme switcher not included; #56 OAuth2 not included.
- E2E ciphertext-only location delivery is not complete; the hosted build stores
  the latest location server-side, with no history.

---

# Pinpoint 1.5.2 - Release Notes

**Status:** Superseded by 1.5.3. 1.5.2 removed the account row, but a
concurrent persist could immediately re-insert it from a stale in-memory
snapshot. The 1.5.2 APK still works with the 1.5.3 server.

**Released:** 2026-11-16 - **Version:** `1.5.2` (from `1.5.1`).

**Artifacts:** `pinpoint-android-1.5.2.apk` - **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.5.2` tag.

## Fixes

- **Account erasure on Postgres:** each cleanup step now runs independently, so
  an append-only audit table (where the runtime role has `INSERT` but not
  `DELETE`) can no longer abort the transaction before the user row is removed.
  Audit entries are anonymised when they cannot be deleted, sessions are
  expired when they cannot be deleted, and diagnostic reports are detached from
  the account. If a foreign key still prevents physical removal, the row is
  scrubbed (email, name, password hash, MFA and recovery data cleared) and
  disabled instead of remaining usable.
- **Rollback snapshot:** the legacy `pinpoint_state` JSONB snapshot is removed
  on account deletion so a restart cannot restore an erased account.
- **Persistence:** databases without `pinpoint_user_id_seq` no longer abort
  every persist transaction. Migration 11/12 tolerates missing optional service
  roles, and registration falls back to a direct insert if the sequence is
  unavailable.
- **Audit:** self-deletion no longer writes a fresh audit row after erasure.

## Upgrade

Install over 1.5.1, or use 1.5.3 for the complete erasure fix.

## Known issues

- #49 server-side anonymous session scoping remains open.
- #50 MFA disable does not require a code yet.
- #58 theme switcher not included; #56 OAuth2 not included.
- E2E ciphertext-only location delivery is not complete; the hosted build stores
  the latest location server-side, with no history.

---

# Pinpoint 1.5.1 - Release Notes

**Status:** Superseded by 1.5.2. The 1.5.1 APK still works with the current
server, but its Postgres account deletion could leave the user row behind when
the audit table denied `DELETE`. Use 1.5.2 for launch.

**Released:** 2026-11-16 - **Version:** `1.5.1` (from `1.5.0`).

**Artifacts:** `pinpoint-android-1.5.1.apk` - **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.5.1` tag.

## Security fixes

- **Location consent:** `/api/v1/locations/friends` now requires an accepted
  friendship **and** an active, non-expired share owned by the location owner,
  with the share's precision applied. 1.5.0 returned every latest location to
  any signed-in user. Update immediately.
- **Erasure:** account deletion now deletes related shares, friendships,
  locations, check-ins, emergencies, diagnostics, sessions and audit rows before
  the user row. In 1.5.0 a user with related rows could remain in Postgres after
  the session was revoked. 1.5.2 completes this fix for append-only audit
  grants.

## Demo removal

- No demo accounts are seeded. The login hint, demo friend pins, demo Compose
  clients and demo credential docs are removed.
- The demo admin and Bob rows are gone. The legacy `alice@example.com` row has
  its demo password disabled; run the cleanup SQL in the 1.5.1 GitHub release
  body (or below) in Supabase to erase it and the stale state snapshot.
- To create an administrator, register a real account and run in Supabase SQL:
  `update pinpoint_users set role = 'administrator' where email = '<real email>';`

## Legacy demo data cleanup (run once in Supabase SQL)

```sql
do $$
declare ids bigint[];
begin
  select array_agg(id) into ids from pinpoint_users
   where email in ('alice@example.com','bob@example.com','admin@example.com');
  if ids is null then return; end if;
  delete from pinpoint_shares where owner_id = any(ids)
     or friendship_id in (select id from pinpoint_friendships where requester_id = any(ids) or receiver_id = any(ids));
  delete from pinpoint_friendships where requester_id = any(ids) or receiver_id = any(ids);
  delete from pinpoint_locations where user_id = any(ids);
  delete from pinpoint_checkins where user_id = any(ids);
  delete from pinpoint_emergencies where user_id = any(ids);
  delete from pinpoint_diagnostic_reports where user_id = any(ids);
  delete from pinpoint_sessions where user_id = any(ids);
  delete from pinpoint_admin_audit where actor_id = any(ids);
  delete from pinpoint_users where id = any(ids);
  delete from pinpoint_state where id = 1;
end $$;
```

## Upgrade note

Install the 1.5.2 APK over 1.5.0/1.5.1. Do not announce 1.5.0: it contains the
consent leak fixed in 1.5.1.

## Known issues

- #49 server-side anonymous session scoping remains open.
- #50 MFA disable does not require a code yet.
- #58 theme switcher not included; #56 OAuth2 not included.
- E2E ciphertext-only location delivery is not complete; the hosted build stores
  the latest location server-side, with no history.

---

# Pinpoint 1.5.0 - Release Notes

**Released:** 2026-11-16 - **Version:** `1.5.0` (from `1.4.2`) -
**Status:** Public launch readiness: privacy policy, account deletion, and the
server-URL and update-check fixes from 1.4.1/1.4.2.

**Artifacts:** `pinpoint-android-1.5.0.apk` - **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.5.0` tag.

Flags: `[FEATURE]` new capability - `[FIX]` bug fix - `[SECURITY]` hardening -
`[DOCS]` documentation - `[INFRA]` build/deploy.

---

## 1.5.0 launch features

- **Public privacy policy:** `/privacy` on the backend, linked from web sign-in
  and the Android About screen.
- **Self-service account deletion:** Android Profile & settings -> Danger zone
  and web Account -> Delete account. Type `DELETE`; email accounts also enter
  the current password. The server removes profile, friendships, shares owned
  by the user, current location, check-ins, emergencies and sessions, then
  clears the cookie. The client removes the local session and E2E key.
- **Android anonymous sign-in debounce:** a double tap cannot create two
  accounts while a request is in flight (partial #49).
- **Release default:** the APK connects to
  `https://pinpoint-uj4y.onrender.com`; Settings and the sign-in screen can
  change servers.
- **Update check:** a newly installed version checks once after an upgrade,
  then keeps the 24h throttle.

## Privacy accuracy

The current 1.5.0 hosted build stores the **latest** location server-side so it
can enforce shares and precision; there is no location history. The app derives
a per-user E2E key on the device, but ciphertext-only delivery is future work and
is not a 1.5.0 guarantee. The public policy at `/privacy` states this explicitly.

## Launch notes

- Distribution: GitHub Releases APK + the Render web app at
  `https://pinpoint-uj4y.onrender.com`. There is no Play Store listing yet.
- Render Free sleeps after about 15 minutes idle; the first request can take up
  to a minute. Upgrade the plan or add a keep-warm check before high traffic.
- Family sharing, the theme switcher, OAuth2 and the MFA-management UI remain
  post-launch.

## Known issues

- #49 server-side anonymous session scoping remains open; the client-side
  duplicate-click guard is included here.
- #50 MFA disable does not require a code yet.
- #58 theme switcher not included.
- #56 OAuth2/federated login not included.
- E2E ciphertext-only location delivery is not complete.

## Upgrade note

If a locally built/debug APK with a different signature is installed, uninstall
it once before installing this release.

---

# Pinpoint 1.4.2 - Release Notes

**Released:** 2026-11-16 - **Version:** `1.4.2` (from `1.4.1`) -
**Status:** Update check after an app upgrade.

**Artifacts:** `pinpoint-android-1.4.2.apk` - **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.4.2` tag.

Flags: `[FEATURE]` new capability - `[FIX]` bug fix - `[SECURITY]` hardening -
`[DOCS]` documentation - `[INFRA]` build/deploy.

---

## 1.4.2 fix

- **Upgrade check:** a newly installed app version performs one launch update
  check even if the previous version checked less than 24 hours ago. Repeated
  launches of the same version remain throttled to once a day.
- **Privacy:** the request is still one GET of the public releases feed with
  `Pinpoint/<version>` as the User-Agent; no account, location, friend or
  device data. The version-change exception is recorded in ADR-0018 and the
  privacy note.
- **Manual path unchanged:** **About -> Check for updates** still runs at any
  time.

## Known issues

- Theme switcher is not included; it remains tracked in #58.
- #49 server-side anonymous session scoping remains open.
- MFA management in the web UI is not included; MFA-enabled accounts can still
  sign in with a code.

## Upgrade note

If a locally built/debug APK with a different signature is installed, uninstall
it once before installing this release.

---

# Pinpoint 1.4.1 - Release Notes

**Released:** 2026-11-16 - **Version:** `1.4.1` (from `1.4.0`) -
**Status:** Android server URL setting; release default is the hosted Render backend.

**Artifacts:** `pinpoint-android-1.4.1.apk` - **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.4.1` tag.

Flags: `[FEATURE]` new capability - `[FIX]` bug fix - `[SECURITY]` hardening -
`[DOCS]` documentation - `[INFRA]` build/deploy.

---

## 1.4.1 features

- **Release default:** the Android release APK connects to
  `https://pinpoint-uj4y.onrender.com` on first launch. Debug builds keep
  `http://10.0.2.2:8080` for the emulator.
- **Change servers in Settings:** **Profile & settings -> Server** shows the
  current URL, a text field, **Save & reconnect**, and **Reset to default**.
- **Recovery from a bad URL:** the sign-in and offline screens also expose the
  Server section, so a custom URL cannot lock you out.
- **HTTPS enforcement:** non-loopback plain HTTP is rejected per ADR-0006.
- **Clean switch:** changing the server stops the old polling thread, clears the
  persisted session and E2E key, and returns to login, so a cookie from one
  server is never sent to another.
- **Persisted choice:** the URL is stored in `prefs.json`; empty means the
  platform default.

## Known issues

- Theme switcher is not included; it remains tracked in #58.
- #49 server-side anonymous session scoping remains open.
- MFA management in the web UI is not included; MFA-enabled accounts can still
  sign in with a code.

## Upgrade note

If a locally built/debug APK with a different signature is installed, uninstall
it once before installing this release.

---

# Pinpoint 1.4.0 - Release Notes

**Released:** 2026-11-16 - **Version:** `1.4.0` (from `1.3.15`) -
**Status:** Web account and security parity.

**Artifacts:** `pinpoint-android-1.4.0.apk` - **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.4.0` tag.

Flags: `[FEATURE]` new capability - `[FIX]` bug fix - `[SECURITY]` hardening -
`[DOCS]` documentation - `[INFRA]` build/deploy.

---

## 1.4.0 features

- **Web account panel:** email and public username are read-only with a copy
  button; the display name is editable; password change and active-session
  management live in the existing drawer. Anonymous accounts see a note
  instead of a password form.
- **Web MFA login:** the sign-in form now accepts a 6-digit authenticator code
  for accounts with MFA enabled.
- **Web crash capture:** uncaught errors stay in memory and are sent only when
  the user presses **Send report**. URLs and token/password-like fields are
  redacted; no location, IP, email, cookie, CSRF token or device ID.
- **Security:** `POST /auth/password/change` now verifies the current password.
  A missing or wrong value returns `401` without revoking the session.
  `must_change_password` accounts keep the force-change path.
- **Anonymous sign-in:** repeated clicks are ignored while a request is in
  flight. Server-side session scoping remains tracked in #49.
- **Docs:** non-essential design, SDLC, UML, component, tutorial, project and
  thesis docs moved to the
  [Pinpoint wiki](https://github.com/Prince-of-Nothing/Pinpoint/wiki); the
  archived tree was removed.

## Known issues

- #49 server-side anonymous session scoping remains open; the client-side
  duplicate-click guard is included here.
- MFA management in the web UI is not included; MFA-enabled accounts can sign
  in with a code.
- Background location is not portable to a browser tab.

## Upgrade note

If a locally built/debug APK with a different signature is installed, uninstall
it once before installing this release.

---

# Pinpoint 1.3.15 — Release Notes

**Released:** 2026-11-16 · **Version:** `1.3.15` (from `1.3.14`) ·
**Status:** Universal APK and a release-notes update dialog with direct download.

**Artifacts:** `pinpoint-android-1.3.15.apk` — **universal**: `arm64-v8a` and
`armeabi-v7a` in one APK. GitHub also generates the source `zip`/`tar.gz`
archives for the `v1.3.15` tag.

Flags: `[FEATURE]` new capability · `[FIX]` bug fix · `[SECURITY]` hardening ·
`[DOCS]` documentation · `[INFRA]` build/deploy.

---

## 1.3.15 fixes

- **One universal APK:** the release now contains both ARM ABIs in a single
  `pinpoint-android-1.3.15.apk`. A 64-bit phone can no longer pick the 32-bit
  asset and fail with a bad-package error.
- **Real update data:** the checker uses GitHub's Releases API for the tag,
  release page, body and asset list. The Atom feed is only a fallback.
- **Direct download:** **Download APK** opens the universal asset URL directly;
  the asset name and size are shown, and the link can be copied.
- **What's new:** a launch dialog shows the release notes for the available
  version (like Aniyomi), with **Download APK**, **Open release page** and
  **Later**. About → Updates shows the same notes.
- **Upgrade note:** if a locally built/debug APK with a different signature is
  installed, uninstall it once before installing this release.

## 1.3.14 features

- **Android share-in:** Pinpoint appears in the text-share sheet again. A small
  Java `PinpointActivity` captures cold-start and repeat shares, wakes the Rust
  UI, and applies the allowlisted deep link. Verified with two consecutive
  shares on the API 37 emulator.
- **Android biometric unlock:** optional Profile toggle; platform
  `BiometricPrompt` on API 28+ unlocks the PIN lock, with the PIN always
  available as fallback. No biometric data is stored.
- **Android background location:** user-started, non-exported foreground
  service with a persistent notification, 15–600 s interval, explicit stop
  control and the Android “all the time” permission flow.
- **Services page:** `GET /status` and `/api/v1/services` report the service
  kind, database mode, component paths and sanitized OSM/tile endpoints
  (ADR-0016).
- **Issue-link CI:** every PR now fails unless its body links an issue with
  `Closes`/`Refs #N` (ADR/policy #29).
- **Build:** Android APKs are packaged by Gradle (Java bridge, DEX, service
  manifest, resources) around the cargo-apk-built `.so`; the release workflow
  zipaligns and signs the unsigned Gradle output. Requires JDK 21 for AGP 8.7
  (ADR-0017).

## 1.3.13 update notification

- **Launch check:** Pinpoint now checks GitHub's public releases feed at most
  once a day after launch/unlock. An older build shows a banner naming the new
  release, with **View update** and **Later** (dismisses that tag).
- **Opt-out:** Profile & settings → Updates turns the automatic check off. The
  manual About → Check for updates button always works.
- **Privacy:** only `Pinpoint/<version>` is sent; no account, location, friend
  or device data. The check is skipped while the app lock is showing. Details in
  ADR-0015 and `docs/PRIVACY.md`.
- **Persistence:** `prefs.json` stores the opt-out, dismissed release tag and
  last check time on-device.
- **Tests:** preference round-trip, corrupt/missing fallback, unwritable store
  and the 24-hour throttle are unit-tested.

## 0. 1.3.12 features

- **GeoJSON import:** paste a `FeatureCollection` exported from Pinpoint into
  Saved → Import GeoJSON. Points are validated (finite, latitude/longitude
  ranges), non-Point geometries are rejected, duplicates against existing
  places and within the batch are skipped, and name/note/category survive.
  One import is capped at 500 places and writes `bookmarks.json` once.
- **Immediate map refresh:** imported places appear as blue pins without a
  restart.
- **Tests:** round-trip, duplicate/invalid classification, malformed/empty
  documents, non-writable stores and the 500-place cap.
- **Issue-linked PRs:** AGENTS/CONTRIBUTING and the PR template require
  `Closes #N`/`Refs #N`; automated enforcement is tracked in #29 pending the
  `workflow` token scope.

## 1. 1.3.11 hardening

- **Private Unix diagnostics:** `crash-reports/` and `logs/` are created with
  mode `0700`; report and log files use `0600`. Panic details, backtrace paths
  and log tails are no longer readable by other local users. Unix-gated tests
  assert the modes.
- **Branch/PR workflow:** `AGENTS.md`, `CONTRIBUTING.md` and the PR template now
  require a fresh branch from `main`, a PR, Squash and merge, then a new branch
  for the next change. `scripts/commit.ps1` refuses to commit on `main`.
  GitHub is configured for squash-only merges with automatic branch deletion.
- **No direct main pushes:** this release itself lands through PR #23 from
  `fix/diagnostics-permissions`.

## 1. 1.3.10 hardening

- **URL allowlist:** `platform::open_url` accepts only `mailto:`, `tel:` and
  `geo:`. HTTP/HTTPS, `file:`, `content:`, `intent:`, `javascript:` and
  `pinpoint:` are rejected. Whitespace and control characters are rejected too,
  so a future caller cannot launch an arbitrary OS component.
- **About diagnostics:** Troubleshooting now shows the backend URL with any
  userinfo redacted and the number of pending crash reports. Seen reports are
  kept on disk but not counted.
- **Crash count logic:** only `crash-*.txt` files are counted, matching the
  existing pending-report selection.
- **Docker maintenance:** `scripts/docker-maintenance.ps1` prunes BuildKit cache
  and compacts Docker Desktop's WSL disk, which had reached 91.4 GB; the manual
  run reduced it to 12.3 GB. README documents the workflow.
- **Tests:** scheme allowlist/rejection, userinfo redaction and pending-report
  counting are unit-tested.

## 1. 1.3.9 features

- **Tap a saved-place pin:** a screen-space hit test finds the nearest bookmark
  (20 pt radius), opens its place sheet without a reverse-geocode request, and
  centres the map at the current zoom. The duplicate blue label is suppressed
  while the selected-place marker is shown.
- **Place sheet actions:** Directions, Set as origin, Save, Copy, Share, OSM,
  Google, Organic and Magic Earth all work from a bookmark tap.
- **Lockout countdown verified:** on the API 37 emulator, the lock screen showed
  **“Try again in 108s.”** with Unlock disabled, then **“Try again in 80s.”**
  seconds later.
- **Bookmark tap verified:** tapping the blue “Saved Cafe” pin logged
  `bookmark tapped` and opened the sheet with **0 m from map centre**.

## 1. 1.3.8 security fix

- **Persistent lockout:** `lock.json` now stores the consecutive failure count
  and a lockout deadline (`locked_until_ms`). Force-stopping the app no longer
  resets the retry delay.
- **Startup enforcement:** the remaining lockout is loaded before the first
  frame; the Unlock button stays disabled until it expires.
- **Reset on success:** a correct PIN clears the failure count and deadline.
- **Atomic writes:** failure state follows the same temp-file + fsync + rename
  path as the PIN hash; corrupt state keeps the app locked.
- **Device verification:** a wrong PIN persisted `failures:1` across
  force-stop; a 60 s persisted lockout rejected the correct PIN; after expiry
  the correct PIN unlocked and the file returned to `failures:0`,
  `locked_until_ms:0`.
- **Tests:** reload persistence, growing failure count, lockout expiry,
  startup deadline and corrupt-state fail-closed are unit-tested.

## 1. 1.3.7 features

- **Saved places on the map:** on-device bookmarks render as blue pins with
  their names, drawn under friend pins. They survive login/logout resets and
  update immediately after adding or removing a place.
- **Saved place actions:** every entry on the Saved screen now has **Map**
  (centre the map there) and **Share** (`geo:` link through the system share
  sheet; desktop copies it).
- **Verified on the API 37 emulator:** a bookmark at 46.99, 28.84 rendered as a
  visible blue “Saved Cafe” pin, and the Saved screen listed 1 saved place.

## 1. 1.3.6 fix

- **Removed the Android text-share target.** Android delivers a second
  `ACTION_SEND` to `onNewIntent`; `NativeActivity` does not update
  `getIntent`, so Pinpoint appeared in the share sheet but silently ignored
  every share while an instance was alive.
- **Why not `launchMode=standard`:** live testing handled three consecutive
  shares, but Android Activity lifecycle callbacks then logged
  `Ignoring spurious JVM callback after last activity reference was dropped`
  and thread counts stayed elevated. Not safe to ship.
- **What remains:** share-out from place sheets and the kiosk QR card,
  `pinpoint://` and `geo:` deep links, `pinpoint://publish`, GPS, PIN lock and
  offline packs.
- **Re-enabling share-in** needs a minimal Java `onNewIntent` bridge (prebuilt
  DEX + manifest component). That breaks the Rust-only packaging rule and
  requires an explicit product decision; see ADR-0014.

## 1. 1.3.5 features

- **Active GPS:** when no provider has a recent last-known fix, **Use GPS**
  requests location updates for up to 8 seconds (GPS → fused → network) and
  removes them afterwards. The fix still has to be recent and reasonably
  accurate before it is published.
- **`pinpoint://publish`:** publishes the current location on launch. It waits
  up to 12 seconds for the persisted session to restore, so a cold-start deep
  link does not race the login.
- **Verified publish path:** on the API 37 emulator with a local backend, the
  app logged `applied launch deep link`, then `location published`, and the
  server recorded `POST /api/v1/locations/me`.
- **Debug UI automation:** a debug-only marker (`files/allow_screenshots`)
  allows screenshots for testing; release builds ignore it and keep
  FLAG_SECURE.

## 1. 1.3.4 features

- **One-shot GPS:** **Use GPS** requests the runtime location permission through
  the live activity, then reads the last known fix from every LocationManager
  provider. Approximate-only grants are accepted; coordinates are validated
  before publishing.
- **Share in/out:** Android accepts shared `geo:`/map links and plain
  coordinates (`ACTION_SEND`, `text/plain`) and opens the map there on launch.
  Repeat shares while Pinpoint is already running need a cold start because
  the Rust-only `NativeActivity` cannot observe Android's `onNewIntent`.
  Place sheets share a `geo:` link and the kiosk QR card shares its
  `pinpoint://kiosk?token=...` link through the system share sheet. Desktop
  copies the link instead.
- **PIN app lock:** Profile → App lock stores an Argon2id hash only and locks on
  launch and when returning to the foreground, with an exponential lockout
  after wrong PINs.
- **JNI fix:** android-activity exposes the Application context, not the
  Activity, so FLAG_SECURE, deep links, permission dialogs and sharing were
  silently failing in 1.3.3. The live `NativeActivity` is now registered and
  every JNI helper uses it.
- **Pinch-to-zoom:** multi-touch pinch steps the map zoom proportionally,
  clamped to zoom 2–18.
- **Verification:** FLAG_SECURE confirmed by a black app-window `screencap`;
  `ACTION_SEND` confirmed in logcat (`action=android.intent.action.SEND`,
  `text_present=true`); location permission and provider enumeration confirmed
  on the API 37 emulator.

## 1. 1.3.3 patch

- **Deep links:** `pinpoint://` and `geo:` are registered and routed on launch,
  including kiosk tokens and map-centre hand-off.
- **Kiosk QR:** the Kiosk screen renders a scannable
  `pinpoint://kiosk?token=...` QR.
- **FLAG_SECURE:** screenshots and screen recording of location data are blocked.
- **No-email fallback:** report and log copies use Android's system
  `ClipboardManager`, so the full report survives when no mail app exists.
- **Fallback safety:** a release with no configured backend disables session
  restore and sign-in instead of sending credentials or a stored session to the
  shared loopback port.
- **Diagnostics:** stronger redaction for Authorization/Bearer headers, JSON
  values and suffixed tokens; log clearing keeps the open writer usable.
- **Android 15:** the mobile tab bar reserves a bottom inset for the edge-to-edge
  gesture bar.
- **Crash prompt:** reports are marked seen after Email or Close, so the prompt
  does not repeat.

## 1. 1.3.2 patch

- **Android release startup:** a rejected default backend URL now falls back to
  the loopback offline client instead of panicking. The app opens with an
  **Offline** badge; `adb reverse tcp:8080 tcp:8080` lets a USB phone reach the
  laptop server at the fallback URL.
- **Crash reports:** Rust panics and `eframe::run_native` errors write a redacted
  report under the app data directory. The next launch shows an editable dialog
  with Email/Copy/Discard/Close. Nothing is sent automatically; Email opens the
  user's mail app with a `mailto:` link to `plesudinu+waitlist@proton.me`.
- **Logs and troubleshooting:** a rolling 1 MiB log, a device/page-size summary,
  log copy/clear, and a manual **Report a problem** flow in About/Profile.
- **Update check privacy:** opening About no longer contacts GitHub; the check
  runs only when the user presses **Check for updates**.
- **16 KB alignment:** the arm64 linker alignment from 1.3.1 remains in place.

## 1. 1.3.1 patch

- **Android 16 KB pages:** the arm64 native library is now linked with
  `-Wl,-z,max-page-size=16384`, so its `PT_LOAD` segments are 16 KB aligned.
  Android 15+ phones configured with 16 KB pages refuse to load 4 KB-aligned
  libraries (`program alignment (4096) cannot be smaller than system page size
  (16384)`), which closed the app before the UI started.

## 2. 1.3.0 native parity

- **Native map proxy**: search/reverse/routing go through the Pinpoint backend
  with map-centre bias, stops, avoid classes, alternatives and capability
  gating.
- **Route planner**: place-based A/B, up to six stops, swap, car/walk/bike,
  alternatives, steps, route share, GPX/KML/GeoJSON export and follow mode.
- **Place sheet**: map tap → reverse geocode, selected marker, Directions,
  Set as origin, Save, Copy, distance and external map hand-off.
- **Sharing**: per-friend sheet with existing-share lookup, enable/pause,
  precision, duration, incoming status and revoke.
- **Account**: MFA TOTP login, anonymous sign-in + recovery code, keychain
  session resume, active sessions/revoke and sign out everywhere.
- **Admin**: overview, users/roles/active/delete, audit log and diagnostics
  triage; correct `admin`/`administrator` gating.
- **Mobile UI**: ArtCraft-inspired Tools tile sheet, light/dark OSM map,
  category chips, bottom tabs, route sheet above the tab bar, About/updates.
- **Update checker**: user-initiated GitHub `releases.atom` check with semver
  comparison, release notes and release-page hand-off.
- **CI/security**: sqlx is Postgres-only, the lockfile is refreshed (mio
  1.2.5), wasm stubs exclude axum/sqlx/tokio, and `cargo audit` reports zero
  vulnerabilities (unmaintained-only warnings remain).
- **Repository**: archived MSVC/SDKs, installers, Go toolchain and build output
  are untracked; 7,810 → 796 tracked files.

## 1. 1.2.1 patch

- **ZAP baseline hardening:** every web response now sets
  `Permissions-Policy`, `Cross-Origin-Embedder-Policy`,
  `Cross-Origin-Opener-Policy` and `Cross-Origin-Resource-Policy`.
  The re-scan is **0 high / 0 medium / 0 low**; the remaining notices are
  informational (intentional `no-store` on HTML, modern-web-app hint). Raw
  evidence: `docs/evidence/zap-baseline.html|json|md`; summary:
  `docs/evidence/zap-baseline-1.2.1.md`.
- **Caching:** static JS/CSS use `no-cache` (revalidate); HTML keeps
  `no-store`.
- **Test robustness:** OS-keychain tests skip cleanly when Windows returns
  `ERROR_NO_SUCH_LOGON_SESSION` (headless/sessionless runs).

---

## 1. What is functional now

| Area | State | What works |
|---|---|---|
| **Auth & sessions** | ✅ | Email/password (Argon2id), opaque hashed sessions, CSRF, logout/logout-all, session list/revoke, MFA, password change, lockout |
| **Anonymous accounts** | ✅ | Create device-bound account, one-time recovery code, recovery rotation |
| **Usernames & privacy** | ✅ | Stable public handle (`calm-otter-42`), exact-match lookup, friend requests by handle, pending requests show handles only, names/emails revealed to accepted friends, numeric IDs not walkable, free-text search admin-only |
| **Web navigation** | ✅ | Leaflet map (local), place search + reverse, place sheet, A→B + multi-stop routes, alternatives, steps, navigate preview with live next-stop, GPX/KML/GeoJSON, route history, saved-place filters, map appearance, locate-me |
| **Route interop** | ✅ | Google Maps + Organic Maps links, Magic Earth `magicearth://` links, KML/GeoJSON/GPX import files |
| **Per-friend sharing** | ✅ | On/off, exact/approximate/city, 1h–7d/forever, immediate stop, real expiry |
| **Admin console** | ✅ | Role-gated modular tab: overview, all accounts (incl. anonymous/never signed in), role/active changes, safe purge revoking sessions |
| **Diagnostics** | ✅ | `pinpoint-diag` on **8005**, PII-free reports (category + message + public handle only), admin triage, no IP/UA/coordinates stored |
| **Admin API** | ✅ | `pinpoint-admin` on **8006**, same fail-closed role gates; report triage + append-only audit |
| **Audit trail** | ✅ | Role/active changes, account deletions and report triage in `pinpoint_admin_audit`; ids/handles only, no email or coordinates |
| **Alice/Bob containers** | ✅ | `pinpoint-demo-client` logs in through the API and publishes real locations on an interval; compose profile `demo` |
| **Desktop client** | ✅ | Map, OSM search/routing/POIs, bookmarks, check-ins, emergency, offline packs; shows the user's public handle and accepts handles or emails for friend requests |
| **Storage** | ✅ | Schema/migrations 1–15 applied against live Postgres 16; new users and locations persisted across a server restart; split ports share sessions/reports/audit |
| **OSM self-host** | ⚠️ Config | `PINPOINT_*_URL` overrides wired; public providers are demo fallbacks |
| **Mobile / WASM** | ⚠️ Scaffold | Signed Android APKs (arm64 + armv7); iOS/WASM remain scaffolds |
| **Privacy** | ✅ | No trackers/IPs, local assets, explicit consent, self-host-first OSM, user-initiated diagnostics |
| **Verification** | ✅ | fmt/clippy/build/test green; browser checks for identity, report → triage → audit; 8005/8006 ports; demo-client login/publish |

### Known limitations (honest)

- No live traffic, weather, lane guidance, road signs or voice guidance.
- Walk/bike routing requires self-hosted OSRM profiles; avoid options require
  a self-hosted profile with exclusions.
- Navigation is a preview with live next-stop; no off-route auto-reroute.
- Postgres persistence was verified against a live Postgres 16 instance,
  including migrations 1–15, restart survival and shared split-port sessions.
- ZAP scan, Android APK (NDK/JDK 21) and iOS (macOS/Xcode) remain external.
- Compose config is validated and the existing stack runs; the new 1.2.0
  images were not rebuilt in this environment.

---

## 2. Change log (flagged)

### `[FEATURE]`
- Generated public usernames + public/friend/private profile split.
- Friend requests by exact handle (legacy exact email fallback for older
  clients); pending requests no longer leak legal names.
- Step-up admin actions: `pinpoint-diag` 8005, `pinpoint-admin` 8006,
  `/api/v1/diagnostics/report`, admin diagnostics triage, `/api/v1/admin/audit`.
- Migration 15: `pinpoint_diagnostic_reports`, append-only
  `pinpoint_admin_audit`, `users.created_at`, report/audit counters.
- Real Diagnostics and Audit admin modules replacing the placeholders.
- Help & diagnostics report form in the web client (category + message only).
- Headless `pinpoint-demo-client` for Alice/Bob containers; compose profile
  `demo`.
- Explicit `PINPOINT_ALLOW_PLAINTEXT_HOSTS` exact-host opt-in for internal
  container DNS (HTTPS remains the default).
- GPX/KML/GeoJSON route export and Google/Organic/Magic Earth hand-off.

### `[FIX]`
- Location publish and emergency alerts now use the real UTC clock instead of
  the seed timestamp; check-ins and friend requests do the same.
- Duplicate friend requests are rejected instead of stacking rows.
- Accepted-friend vs pending vs unrelated profile responses are now distinct.
- Admin deletion records an audit row and removes the account without putting
  its email in the audit trail.

### `[SECURITY]`
- ZAP baseline re-scan: 0 high / 0 medium / 0 low; COOP/COEP/CORP and
  Permissions-Policy headers set on every web response.
- Argon2id; hashed sessions; strict CSP; local assets; admin role gates.
- No raw IP/UA storage; diagnostics explicitly exclude location and device
  identifiers; audit is ids/handles only; OSM calls never carry user/friend
  data.
- Numeric user IDs are not walkable; non-admin user search is exact-handle
  only.

### `[DOCS]`
- Dual-auth model updated with the public-identity matrix and lookup rules.
- Admin console guide now documents the real modules, 8005/8006 split and the
  append-only audit guarantees.
- Client-platforms guide documents the headless demo clients and plaintext
  opt-in.

### `[INFRA]`
- New binaries: `pinpoint-diag`, `pinpoint-admin`, `pinpoint-demo-client`.
- Compose: diag/admin services + gateway routes + `demo` profile for Alice/Bob.
- Per-service grants: diag gets report INSERT/SELECT only; admin reuses the
  users role with sessions/report/audit grants.

---

## 3. Run

Combined demo (web UI + all APIs):

```powershell
$env:BIND_ADDR = "127.0.0.1:8080"
.\pinpoint-server.exe
# http://127.0.0.1:8080/ · alice@example.com / DemoPass123!
# admin: admin@example.com / ChangeMe123!
```

Split ports:

```powershell
$env:BIND_ADDR = "127.0.0.1:8005"; .\pinpoint-diag.exe
$env:BIND_ADDR = "127.0.0.1:8006"; .\pinpoint-admin.exe
```

Alice/Bob as real clients:

```powershell
$env:PINPOINT_BACKEND = "http://127.0.0.1:8080"
$env:PINPOINT_EMAIL = "alice@example.com"
$env:PINPOINT_PASSWORD = "DemoPass123!"
$env:PINPOINT_LAT = "47.0105"
$env:PINPOINT_LON = "28.8638"
.\pinpoint-demo-client.exe
```

Docker compose:

```powershell
docker compose up --build -d
docker compose --profile demo up -d alice bob
# gateway :8000 · diagnostics :8005 · admin API :8006
```

Desktop: `pinpoint-desktop.exe --backend http://127.0.0.1:8080`.

Self-hosted OSM:

```powershell
$env:PINPOINT_TILE_URL = "https://tiles.example.com/{z}/{x}/{y}.png"
$env:PINPOINT_NOMINATIM_URL = "https://nominatim.example.com"
$env:PINPOINT_OSRM_URL = "https://osrm.example.com"
$env:PINPOINT_OVERPASS_URL = "https://overpass.example.com/api/interpreter"
```

## 4. Verification

- fmt ✅ · clippy `-D warnings` ✅ · build ✅ · release build ✅
- **174 unit + 7 asset + 10 contract + 14 router tests** ✅
- Browser: username display, request by handle, pending/accept, duplicate and
  unknown-handle states; report → triage → audit in the admin console.
- Ports: 8005 ingestion 201, invalid category 400, admin route isolated 404;
  8006 health 200, unauthenticated admin 401.
- Demo client: logged in as `@bright-badger-2`, published through
  `/locations/me`, and a friend read back the fresh `updated_at`.
- Postgres 16: migrations 13–15 applied; a new user (`@coral-badger-5`) and
  published location survived a server restart; one login on 8080 authorized
  the admin API on 8006 with the shared session; a report ingested on 8005
  appeared in the 8006 list, was triaged, and wrote an audit row.
- ZAP 2.17.0 baseline: **0 high / 0 medium / 0 low**, 4 informational; raw
  reports archived under `docs/evidence/`.
- Compose: `docker compose config --quiet` ✅, 11 services including
  diag/admin/alice/bob.

## 5. Next

1. Authenticated ZAP active scan (seeded session + context).
2. Android APK (NDK + JDK 21) and iOS build on macOS.
3. Rebuild the 1.2.x compose images and run the full gateway/demo profile.
4. Final thesis report / demo script.

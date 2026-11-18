# Native Today screen review

**Checked:** 2026-11-18, branch `docs/midterm2`, HEAD `5285bc6b` ("[DOCS] midterm 2: report, demo script and slide deck").
**Scope:** `src/ui/today.rs`, `src/app.rs`, `src/api.rs`, `src/backend/mod.rs`, `src/backend/failover.rs`, `src/map.rs`; the web Today screen (`web/app/`) for parity.
**Method:** static review of the source at HEAD. No live backend, account or device; no build or test command was run in this read-only review. `tests/today.rs` and `tests/contract.rs` were read to confirm the intended contract.
**Verdict:** the owner-only API wiring and the happy-path refresh story are **correct**. The screen's error handling, draft handling and refresh independence are **not**: every Today action discards its error, the composer clears before the POST succeeds, and an unrelated refresh failure can stop Today from loading. A real screenshot of the native Today screen cannot be produced with the repository's tooling.

## Summary

| ID | Finding | Verdict |
|---|---|---|
| P1 | Owner-only endpoints; the client never supplies a user id | PASS |
| P2 | Session cookie on GET, CSRF on POST/DELETE | PASS |
| P3 | 24-hour trail window matches the server | PASS |
| P4 | Refresh after login, register, session restore and forced password change | PASS |
| P5 | Refresh after publishing a location | PASS |
| P6 | Refresh after note create/delete | PASS on the happy path (errors lost, F1) |
| P7 | Logout clears cached private data | PASS |
| P8 | Note ownership on delete; no existence leak | PASS |
| P9 | Map-centre happy path | PASS (latent risk, F10) |
| F1 | Today actions discard their errors; no feedback | FIX, High |
| F2 | Draft cleared before save; server limits not enforced locally | FIX, High |
| F3 | Fail-fast refresh and false empty states | FIX, High |
| F4 | 401 keeps private data; an in-flight refresh can cross accounts | FIX, High |
| F5 | Delete has no confirmation or undo | FIX, Medium |
| F6 | Photos are downloaded and cloned although never rendered | FIX, Medium |
| F7 | Lists truncate silently; no "show on map" for a note | FIX, Medium |
| F8 | Private trail is drawn on the map with no visibility toggle | FIX, Medium |
| F9 | Failover retries non-idempotent create and 4xx errors | FIX, Medium (failover builds) |
| F10 | Map centre is read without initialising the viewport | FIX, Low (defensive) |
| F11 | No repaint when a Today action finishes | FIX, Low |
| F12 | Today's tab and copy bypass the i18n registry | FIX, Low |

## PASS findings

### P1 - The wire path is genuinely owner-only

- The client calls only the owner paths and never sends a user id: `get_location_history` `GET /api/v1/locations/history` (`src/api.rs:650-656`), `get_place_notes` `GET /api/v1/locations/notes` (`src/api.rs:658-664`), `create_place_note` `POST /api/v1/locations/notes` (`src/api.rs:666-688`), `delete_place_note` `DELETE /api/v1/locations/notes/{id}` (`src/api.rs:690-700`).
- The server requires `CurrentUser` and scopes every query by `cu.user_id`: history (`src/server/locations_handlers.rs:362-407`), notes (`:496-536`), delete (`:540-575`). A missing session returns 401; another user's note becomes `NOTE_NOT_FOUND` (`:549-571`).
- `tests/today.rs:77-119` proves Alice's trail is empty for Bob and unauthenticated reads are 401. `tests/today.rs:121-226` proves notes are owner-only, Bob's DELETE is rejected, and validation is enforced.
- No trail or note field appears in friend/share/kiosk payloads. Inside the client, `AppData.history/notes` (`src/models.rs:401-404`) is consumed only by Today (`src/ui/today.rs`) and the owner's map trail (`src/map.rs:434-458`).

### P2 - Cookie and CSRF discipline

`src/api.rs:187-203` attaches the session cookie to every request and `X-CSRF-Token` only for unsafe methods. `tests/contract.rs:342-385` pins the method, path and CSRF header for history, notes, create and delete. PASS.

### P3 - The 24-hour window is real

The server cuts at `Utc::now() - 24h` and filters `recorded_at >= cutoff` (`src/server/locations_handlers.rs:370-407`; the PostgreSQL query at `:372-376` does the same). The screen says "in the last 24 hours" (`src/ui/today.rs:33,56-62`), which matches the endpoint. The native copy correctly does not repeat the web's stronger "kept for 24 hours" claim (`web/app/index.html:330`): PostgreSQL actually retains seven days (`src/server/state/locations.rs:232-233,269-275`). That is a web/doc wording problem, not a native one.

### P4 - Refresh after authentication is complete

- Login: `apply_signed_in` sets the Map screen, then `refresh_all` runs (`src/app.rs:656-667`).
- Register: `src/app.rs:700-711`.
- Persisted session restore: `src/app.rs:794-801`.
- Anonymous sign-in and recovery: `src/app.rs:817-823` and `:838-844`.
- Forced password change: refresh after the change (`src/app.rs:960-968`); if other sessions were revoked, the data is correctly reset and the user is signed out instead (`:949-959`).

PASS. The caveat is that `refresh_all` can stop before Today (F3).

### P5 - Refresh after publishing a location

On success, `publish_location` refreshes friend locations, the caller's own location and Today (`src/app.rs:1255-1269`). A new trail point therefore appears without waiting for the 30 s poll. PASS.

### P6 - Refresh after note create and delete

Both actions call `refresh_today` after the mutation (`src/app.rs:1046-1050`, `:1054-1057`), and the Today screen's Refresh button calls the same function (`src/ui.rs:861-863`). The refresh *call* is present and correctly scoped; its failure is lost (F1) and the draft is cleared too early (F2). PASS on the happy path.

### P7 - Logout removes cached private data

`logout` replaces `AppData` with fresh data and returns to the Map (`src/app.rs:729-742`); `fresh_data` contains no history or notes (`src/app.rs:159-166`, fields in `src/models.rs:359-432`). The app lock and logout paths do not leave a persisted list in the keychain. PASS.

### P8 - Deleting another user's note does not leak it

The PostgreSQL delete is `WHERE id=$1 AND user_id=$2`; both branches return `NOTE_NOT_FOUND` for a missing or foreign id (`src/server/locations_handlers.rs:549-575`). `tests/today.rs:158-177` covers Bob deleting Alice's note: rejected, Alice still sees one note. PASS.

### P9 - The map-centre happy path is correct

The Map screen is the startup screen (`CurrentScreen` default at `src/app.rs:133-136`; state at `:209`). Drawing it calls `map::render_map_with` -> `ensure_viewport` (`src/map.rs:198-210`, `:20-38`), which seeds the centre from the first pin or Chișinău (`src/map.rs:14-17`). Today then uses that centre and displays it literally (`src/ui/today.rs:120`), and the server accepts the coordinate (`src/server/locations_handlers.rs:430-437`). PASS for the route a user can actually take; F10 covers the dependency on another screen.

### P10 - Empty and signed-out states are present

The empty trail state is actionable ("Publish your location on the Map screen"), `src/ui/today.rs:50-54`. Signed-out users get the shared sign-in form, not the lists: `src/ui.rs:2010-2015`. PASS.

## FIX findings

### F1 (High) Every Today action discards its error

`src/ui.rs:848-863` matches `CreateNote`, `DeleteNote` and `RefreshToday` with `let _ = app.…await`. The corresponding app methods never call `set_error` or `set_notice`: `refresh_today` `src/app.rs:1030-1037`, `create_place_note` `:1039-1052`, `delete_place_note` `:1054-1058`.

Consequences:

- Backend down, 401 or 400: pressing **Refresh** changes nothing and shows nothing. The only error text shown anywhere is one another path placed in the header (`src/ui.rs:1318-1323`).
- **Add note here** clears the composer and appears to work even when the POST failed (also F2).
- **Delete** appears to do nothing when the delete or the following refresh failed; the note stays visible.
- The manual refresh does not call `set_loading` (`src/ui/today.rs:44-46`), so there is no spinner. On a failover build `try_all` can spend about 63 s on one backend (4 attempts x 15 s timeout plus 0.5/1/2 s backoff) before giving up (`src/backend/failover.rs:68-106`, `src/api.rs:123-125`), during which the button looks dead.

Fix: handle the `Result` in `spawn_action` (or inside these three app methods), call `set_error` on failure and `set_notice` on success; wrap the manual refresh in `set_loading(true)` / `set_loading(false)`. `publish_location` already shows the intended pattern (`src/app.rs:1261-1278`).

### F2 (High) The draft is cleared before the save succeeds, and the server limits are not enforced in the UI

`src/ui/today.rs:100-118` spawns the action and immediately clears `note_label` and `note_message` in the same frame. The request runs later. If it fails for any reason (offline, 401, validation, server error), the text is gone and no error appears (F1).

The composer also has no character limit (`src/ui/today.rs:84-97`). The server rejects labels over 120 characters and messages over 2000 (`src/server/locations_handlers.rs:444-455`), and the web enforces exactly those maxima (`web/app/index.html:363,366`). A long note is therefore rejected server-side after the UI has already discarded it.

There is no offline queue for notes: `PendingMutation` covers publish, friend requests, shares, check-ins, emergencies and password changes (`src/app.rs:12-45`), but not notes. Publishing has a queue (`src/app.rs:1271-1278`); notes do not.

Fix: add `char_limit`/a counter to both fields, validate locally, and clear the draft only after `create_place_note` returns `Ok` (keep or restore it on error). If the app is offline, keep the draft and say so.

### F3 (High) An unrelated refresh failure stops Today from loading and produces false empty states

`refresh_all` returns immediately when friendships or shares fail (`src/app.rs:992-999`) and only then reaches `refresh_today` (`:1003-1005`). Today does not depend on either call. On the first login, one failing friends endpoint leaves history and notes empty until a manual Refresh; the screen then says "No points yet" / "No notes yet" (`src/ui/today.rs:50-54,129-131`) as though the fetches had succeeded.

`refresh_today` itself fetches sequentially with `?` (`src/app.rs:1030-1032`): if the notes request fails, a successful history response is discarded too.

The server hides database failures behind empty arrays: `unwrap_or_default()` for history (`src/server/locations_handlers.rs:371-379`) and notes (`:504-511`). A 200 response can therefore mean "query failed". The web has the same `.catch(() => [])` behaviour (`web/app/app.js:274-280`), so the false empty state is cross-client.

Fix: load friends, shares, history and notes independently instead of returning early across features; track a per-section loaded/error flag and render an error (with a Retry) instead of the empty state; return an error envelope on a database failure. Today's own Refresh already has the right scope (`src/ui.rs:861-863`).

### F4 (High) Auth boundary: 401 keeps private data; an in-flight refresh can cross accounts

**Expired session.** `ApiClient::check` turns every non-2xx into a generic error (`src/api.rs:214-224`) and nothing clears the session or `AppData`. `refresh_all` only sets the error (`src/app.rs:1003-1005`); `is_signed_in` remains true because `data.user` is still set (`src/app.rs:380-382`). Today continues to render the cached trail and notes (`src/ui/today.rs:129-161`). The web explicitly hides the app and shows the login panel when `/auth/me` fails (`web/app/app.js:262-273`).

**Stale response.** `refresh_today` assigns its response unconditionally after the await (`src/app.rs:1030-1036`). If an auto-refresh cycle is in flight when the user logs out (`src/app.rs:729-742` resets `AppData`) and signs in to another account, the previous account's history/notes can be written into the new session. The loop checks sign-in only before the cycle (`src/app.rs:274-276`), not after the request. If the new session's refresh then fails at F3, the old account's private lists remain on screen.

Fix: detect 401, clear the session and private fields, and route to Login; capture the signed-in user id (or a generation counter) before the fetch and apply the result only if it still matches; ignore results that arrive after logout. This requires keeping the HTTP status or a typed error through `ApiClient::check`.

### F5 (Medium) Delete has no confirmation or undo

`src/ui/today.rs:162-169` deletes on one click. The web asks `window.confirm("Delete this private note?")` (`web/app/app.js:487-489`). The server delete is permanent (`src/server/locations_handlers.rs:549-575`), and the button is a 44 pt touch target, so an accidental tap loses a private note.

Fix: an inline two-step confirm ("Delete" -> "Confirm delete") or a modal, matching the web's intent.

### F6 (Medium) Every note's photo is downloaded and cloned although the native client never shows it

`get_place_notes` returns the full `PlaceNote`, including the base64 `photo` data URL (`src/api.rs:658-664`; model `src/models.rs:182-197`). `refresh_today` stores the whole list (`src/app.rs:1030-1035`). `draw_today_screen` clones both vectors on every frame (`src/ui/today.rs:19-28`), which copies every photo string, and then only checks `photo.is_some()` (`:155-160`). The default auto-refresh is every 30 s (`src/models.rs:248-250`), so all photos are re-downloaded on every cycle. The server caps one photo at 900,000 characters (`src/server/locations_handlers.rs:410-418`).

This is unnecessary mobile data and a per-frame allocation of up to roughly 0.9 MB per note with a photo; expect jank or memory pressure on Android with a photo-heavy account.

Fix: snapshot only the display fields plus `has_photo: bool`; do not clone the photo. Prefer a native list representation/query without the photo bytes (if the API changes, add a contract test), or cache notes and refresh on user action.

### F7 (Medium) Silent truncation and no way to locate a note on the map

- Notes: `.iter().rev().take(24)` (`src/ui/today.rs:133`) hides every note older than the newest 24, with no count and no "load more". Those notes cannot be viewed or deleted from the native client. The web renders all notes (`web/app/app.js:432`) and offers **Show on map** plus note markers (`:456-466`, `web/app/map.js:505-529`).
- Trail: the caption reports `history.len()` (`src/ui/today.rs:56-62`) but the list renders only 24 (`:63`). With more than 24 points the "N point(s) ... newest first" line is misleading; "point(s)" is also unpolished (the web pluralises, `web/app/app.js:391`).
- Native has no **Show on map** for a note, and the egui map never draws note markers (only the trail is drawn, `src/map.rs:434-458`). A saved note can only be found by reading its coordinates.

Fix: show "latest 24 of N" or render all notes in the existing ScrollArea; add **Show on map** using `focus_map_at` (`src/ui.rs:692-701`); optionally draw note markers. Fix the trail caption to state the visible count.

### F8 (Medium) The private trail is drawn on the map automatically; the web requires an explicit toggle

`src/map.rs:434-458` draws the trail whenever `state.history.len() > 1`, on every Map render. The web defaults `trailVisible` to false (`web/app/app.js:15-17`) and only draws after the user presses **Show trail** (`:410-415`, `:1112-1124`; `web/app/map.js:466-503`).

The data is owner-only, so this is not a friend-visible leak. It is still a change in the default exposure of a private trail (screen sharing, screenshots, a borrowed device) and a visible divergence from the web; the native UI offers no way to hide it.

Fix: add a trail visibility toggle (default off) persisted in prefs, or draw only after an explicit action.

### F9 (Medium, failover builds) Non-idempotent creates and 4xx errors are retried

`FailoverBackend::try_all` retries every `Err` against all backends for `max_retries = 3` (`src/backend/failover.rs:68-106`; configured at `src/backend/mod.rs:373-375`), and `create_place_note` is included (`src/backend/failover.rs:543-561`). If an attempt times out after the server committed the insert, the retry creates a second private note; there is no idempotency key or client-generated note id.

A 400 validation error (for example the over-length message in F2) is also retried for four rounds with 0.5/1/2 s backoff and a 15 s timeout each (`src/backend/failover.rs:98-102`, `src/api.rs:123-125`) before the UI silently gives up. `delete_place_note` is retried too (`src/backend/failover.rs:563-571`); a repeated delete is harmless, but a `NOTE_NOT_FOUND` is retried needlessly.

Fix: do not fail over/retry non-idempotent POSTs, or send a client-generated idempotency key and dedupe server-side; treat 4xx as non-retryable. Add a failover test where the first attempt commits and then times out; the only failover test today constructs the pool (`src/backend/failover.rs:805-820`).

### F10 (Low, defensive) Today reads the map centre without initialising it or guarding (0, 0)

`src/ui/today.rs:19-28` reads `map_center_lat/lon` raw. `AppData::default` is `0.0, 0.0` with `map_zoom = 0` (`src/models.rs:359-432`, fields at `:416-421`). `ensure_viewport` is the only code that turns that into Chișinău or the first pin (`src/map.rs:20-38`), and it is called from `render_map_with` (`src/map.rs:198-210`), not from Today.

This is currently safe in user flow: Map is the startup screen (`src/app.rs:133-136`) and is drawn before Today can be clicked (Today is a toolbar/menu item, `src/ui.rs:1346,1831`, gated at `:2010-2015`). There is no `pinpoint://today` deep link (`src/mobile/mod.rs:163-175`), and the compact map also calls `render_map_with` (`src/ui/map.rs:515-539`).

It is fragile: if Today is ever rendered first (a future deep link, a UI test/harness, a restored screen), the composer posts `(0.0, 0.0)`, which the server accepts as valid (`src/server/locations_handlers.rs:430-437`). The screen would show "Map centre: 0.00000, 0.00000" (`src/ui/today.rs:120`), so the value is visible but wrong.

Fix: call `crate::map::ensure_viewport` before reading, and disable **Add note here** until the viewport is initialised (or reject exactly 0, 0). Add "Pan the map first" to the hint (`src/ui/today.rs:98`), matching `web/app/index.html:372`.

### F11 (Low) There is no repaint when a Today action finishes

`spawn_action` requests a repaint immediately after spawning (`src/ui.rs:781-1210`; the request is at `:1210`), but the spawned task does not request one when it finishes. `refresh_today`, create and delete update `AppData` later (`src/app.rs:1030-1058`). On a live backend the response usually arrives after that frame, so a manual Refresh or a newly created note may not appear until the next pointer/key event or the 30 s auto-refresh (`src/app.rs:248-279`). The map screen masks this with its tile-load repaint (`src/map.rs:137`); Today has no such loop.

Fix: clone the `egui::Context` into the spawned task and call `request_repaint()` when the action completes (a small wrapper around `spawn_action`).

### F12 (Low) Today's tab and copy bypass the i18n registry

The tab label is the raw string `"Today"` in the desktop toolbar (`src/ui.rs:1346`) and the mobile tools list (`src/ui.rs:1831`). All copy in `src/ui/today.rs` is hardcoded English (`:30-33,42,50-62,84-98,100,120,127-130,155-159`). The surrounding shell uses `i18n::t` (`src/ui.rs:1377`), and there is no `TODAY` key in `src/i18n.rs` (grep: no matches). Switching language leaves this screen in English.

Fix: add `key::TODAY` and the handful of visible strings to the registry, or explicitly document Today as English-only in this build.

## Screenshot of the actual Today screen: not possible with this repo's tooling

There is no way to produce a **genuine** screenshot of the native Today screen with the committed tooling:

1. `--screenshot <PATH>` is not a framebuffer capture. After 120 frames it creates a synthetic 1200x800 RGB gradient and saves that (`src/ui.rs:1717-1738`), then closes the window. It never shows the UI. `docs/DEPLOYMENT.md:364` calls the flag a "capture", which overstates what the code does.
2. The screenshot run always starts on `CurrentScreen::Map` (`src/app.rs:133-136,209`), does not sign in, and has no `--screen`/deep-link option (`src/main.rs:89-119`). Today is only reachable by clicking its tab after a real sign-in (`src/ui.rs:1346,2010-2015`).
3. There is no native UI test harness. `tests/today.rs` drives only the Axum router; `tests/contract.rs` drives only the HTTP client. `Cargo.toml` has eframe, not `egui_kittest`.
4. Browser tooling can capture the web Today page, not the egui window.
5. The existing assets cannot stand in: `docs/midterm2/assets/06-native.png` is the native Map screen, signed out, and its tab strip predates the Today tab (there is no Today tab in that image; compare `src/ui.rs:1344-1357`). `docs/midterm2/assets/03-today.png` and `04-notes.png` are the web client (web email chrome and MapLibre map), not the native screen.

A real capture would require running the desktop binary on an interactive Windows desktop against a live/loopback backend, signing in with a test account that has two published points and one note, clicking **Today**, and using an OS-level window capture (for example Win+Shift+S). That needs a live account and data, writes an artifact outside this review, and is not available in this environment; it was not attempted.

## Not verified here

- Runtime behaviour against a live or loopback backend; no account, no seeded points/notes, no Android device or emulator.
- `cargo fmt --all -- --check`, clippy, build and tests were not run; this was a static review under the read-only constraint. The two test files above were read, not executed.
- Visual layout/overflow on narrow screens (the list and composer were reviewed in code only).
- Web client behaviour beyond the Today/notes sections used for parity.
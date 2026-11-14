# Native Client Web-Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the missing `web/app` flows into the native Rust eframe client so
desktop and Android can search, route, share, manage sessions and administer
Pinpoint without using the web UI.

**Architecture:** Add thin `ApiClient`/`Backend` methods over the existing Axum
`/map/*`, `/auth/*`, `/diagnostics/*` and `/admin/*` endpoints. Keep UI state in
`PinpointAppUI` and `AppData`; add a reusable route sheet module and a
per-friend share sheet. No new server contract, no Kotlin/WebView.

**Tech Stack:** Rust, eframe/egui 0.32, Axum 0.7, serde, reqwest, existing
`KeyManager`, existing JSON bookmark/route stores.

**Spec:** `docs/superpowers/specs/2026-11-14-native-web-parity-design.md`

## Global Constraints

- Rust only; no Kotlin, WebView, Tauri, CDN or keyed map SDK.
- All map/search/route traffic goes through the Pinpoint backend; never call
  Nominatim/OSRM directly from the client.
- Server-side enforcement only; client role checks are UI hints.
- ODbL attribution stays visible; friend coordinates never go to OSM.
- No secrets or real user data in tests; logs and errors stay PII-free.
- Full gate after each task: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo build --locked`,
  `cargo test --locked`.
- Commit subjects use `[FEATURE]`/`[FIX]` and CHANGELOG entries under
  `[Unreleased]`.

## Review Focus

1. **MFA-enabled login** — a valid password plus a 6-digit TOTP must create a
   session; a missing/invalid code must show `MFA_REQUIRED`, never a generic
   failure.
2. **Route errors** — invalid coordinates, unavailable foot/bike profiles,
   avoid options without self-hosted OSRM, and upstream timeouts must show a
   readable message and leave the map usable.
3. **Map tap vs route** — tapping the map with the route sheet open sets B;
   otherwise it opens "What's here?" and never does both.
4. **Existing share** — opening a friend who already shares must load and PATCH
   that share instead of creating a duplicate.
5. **Session resume/revoke** — an expired/revoked cookie must fall back to the
   login screen; "sign out everywhere" must clear local state even if the
   network call fails.

---

### Task 1: Backend map proxy + capabilities

**Files:**
- Modify: `src/osm.rs` (`SearchPlace.distance_m`, serde derives on `Route*`)
- Modify: `src/api.rs` (`MapCapabilities`, `map_capabilities`, `map_search`,
  `map_reverse`, `map_route`)
- Modify: `src/backend/mod.rs` (trait methods)
- Modify: `src/app.rs` (wrappers)
- Test: `src/api.rs` unit tests, `tests/router_probe.rs`

**Interfaces:**
- `pub struct MapCapabilities { pub autocomplete: bool, pub modes: Vec<String>, pub avoid: bool }`
- `ApiClient::map_search(&self, q: &str, lat: Option<f64>, lon: Option<f64>, limit: usize) -> anyhow::Result<Vec<SearchPlace>>`
- `ApiClient::map_reverse(&self, lat: f64, lon: f64) -> anyhow::Result<SearchPlace>`
- `ApiClient::map_route(&self, from: (f64,f64), to: (f64,f64), mode: &str, via: &[(f64,f64)], avoid: &[String], alternatives: bool) -> anyhow::Result<Vec<Route>>`
- Matching `Backend` methods with borrowed args and `BackendFuture`.
- `PinpointApp::map_search/map_reverse/map_route/map_capabilities`.

- [ ] **Step 1: Write failing deserialization/query tests**

Add tests that parse the exact server envelopes:
`{"results":[{...,"distance_m":12.5}]}` into `SearchPlace` and
`{"routes":[{...,"points":[[47.0,28.8]],"steps":[],"legs":[]}]}` into
`Route`; assert `distance_m` and route points survive.
Add a `map_route_query()` pure helper test asserting `via`/`avoid`/`mode`
encoding and that no identity fields are present.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --locked map_`
Expected: compile error / missing fields.

- [ ] **Step 3: Implement types, query helper and client methods**

Use `self.request(Method::GET, path, None)`, `Self::check`, and parse:
- search: `#[derive(Deserialize)] struct SearchResponse { results: Vec<SearchPlace> }`
- reverse: `SearchPlace`
- route: `#[derive(Deserialize)] struct RouteResponse { routes: Vec<Route> }`

Percent-encode query values with the existing `urlencoding()` helper in
`api.rs` (or reuse `osm::urlencoding` if public). `via` is
`lat,lon;lat,lon`, `avoid` is comma-separated. Reject >6 stops and >3 avoid
classes client-side.

- [ ] **Step 4: Wire the Backend trait and PinpointApp**

Add methods to `Backend` with `BackendFuture`; implement in `ApiClient` and
`rest::RestBackend` delegation. Add `PinpointApp` wrappers that call
`self.backend`.

- [ ] **Step 5: Run the gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo build --locked && cargo test --locked`
Expected: all green.

- [ ] **Step 6: Commit**

`[FEATURE] api: backend map proxy and capabilities`
CHANGELOG: Added — map/search/route proxy methods.

---

### Task 2: MFA-aware login

**Files:**
- Modify: `src/api.rs` (`login` body includes `totp` when non-empty)
- Modify: `src/backend/mod.rs` (`login_with_totp`)
- Modify: `src/app.rs` (`login(email, password, totp)`)
- Modify: `src/ui.rs` (`login_totp` field)
- Modify: `src/ui/auth.rs` (optional "Authenticator code" field)
- Test: `src/api.rs` unit test for the login body

**Interfaces:**
- `ApiClient::login_with_totp(&self, email: &str, password: &str, totp: Option<&str>) -> anyhow::Result<User>`
- `Backend::login_with_totp(...)`; `PinpointApp::login(email, password, totp)`.
- UI shows the code field always; the server ignores it for non-MFA users.

- [ ] **Step 1: Write the failing body test**

Assert `login_body("a@b.c", "pw", Some("123456"))` contains `"totp":"123456"`
and `login_body(..., None)` does not contain `totp`.

- [ ] **Step 2: Run it to fail**

Run: `cargo test --locked login_body`
Expected: FAIL (helper missing).

- [ ] **Step 3: Implement and wire the UI**

Use `serde_json::json!` conditional; propagate `MFA_REQUIRED` from
`format_api_error` unchanged. Add the field and pass it through
`PendingAction::Login { email, password, totp }`.

- [ ] **Step 4: Run the gate and commit**

`[FIX] auth: accept TOTP codes during native login`
CHANGELOG: Fixed — MFA-enabled accounts can sign in.

---

### Task 3: Route state and route sheet

**Files:**
- Create: `src/ui/route.rs` (`RoutePoint`, `RoutePlan`, draw helpers)
- Modify: `src/ui.rs` (module, state fields, dispatch, `PendingAction::Route`)
- Modify: `src/ui/places.rs` (replace raw lat/lon form with route sheet)
- Modify: `src/ui/map.rs` (route sheet entry from place/search)
- Modify: `src/map.rs` (draw selected route; already draws `last_route`)
- Test: unit tests in `src/ui/route.rs`

**Interfaces:**
- `RoutePoint { lat: f64, lon: f64, name: String }`
- `RoutePlan { origin: Option<RoutePoint>, destination: Option<RoutePoint>, stops: Vec<RoutePoint>, avoid: Vec<String>, mode: String, routes: Vec<Route>, selected: usize, open: bool, following: bool, next_stop: usize }`
- `RoutePlan::swap()`, `apply_selected(&mut AppData)`, `summary()`.
- `draw_route_sheet(&mut self, ui, ctx)` in `PinpointAppUI`.

- [ ] **Step 1: Write failing state tests**

`swap` exchanges A/B and preserves stops; `apply_selected` copies the chosen
route to `last_route`; `summary` formats km/min; a route with no alternatives
is cleared safely.

- [ ] **Step 2: Implement state and backend call**

`PendingAction::Route` calls `app.map_route(from, to, mode, via, avoid, true)`
and stores `routes`; errors go to `app.set_error`. Default mode `driving`;
origin defaults to map centre or `my_location`.

- [ ] **Step 3: Build the shared sheet**

Sections: A/B fields (click to set from place sheet), stops with remove,
"Add stop" (uses map click/next place), avoid checkboxes gated by
`capabilities.avoid`, swap, mode tabs gated by `capabilities.modes`,
alternatives chips, summary, steps with distance, Navigate/Share/GPX/KML/
GeoJSON/Clear, Google/Organic/Magic Earth route links.
Use `egui::ScrollArea` and a `CENTER_BOTTOM` sheet on compact.

- [ ] **Step 4: Run the gate and commit**

`[FEATURE] route: native route planner with stops, modes and exports`
CHANGELOG: Added — route planner/navigation sheet.

---

### Task 4: Place sheet parity + map tap

**Files:**
- Modify: `src/models.rs` (`AppData.selected_place`, `map_click`)
- Modify: `src/map.rs` (selected marker, `screen_to_latlon`)
- Modify: `src/ui/map.rs` (tap handling, place actions, distance, copy, origin)
- Modify: `src/ui.rs` (`PendingAction::Reverse` stores selected place)
- Test: `src/map.rs` projection round-trip

**Interfaces:**
- `pub fn screen_to_latlon(map_rect: Rect, center: (f64,f64), zoom: u8, pos: Pos2) -> (f64,f64)`
- `AppData.selected_place: Option<SearchPlace>`, `AppData.map_click: Option<(f64,f64)>`.
- Place sheet actions: Directions (sets route destination + opens sheet),
  Set as origin, Save (toggle), Copy (name + coords), OSM/Google/Organic/
  Magic Earth, distance from map centre.

- [ ] **Step 1: Write the projection test**

Round-trip a known lat/lon through `project_to_rect` and
`screen_to_latlon` at z=13, assert <1e-6 degrees error.

- [ ] **Step 2: Implement tap + reverse**

In `render_map_with`, on `response.clicked()` with a pointer position, set
`state.map_click`; the UI consumes it, spawns `PendingAction::Reverse`, and
`Reverse` sets `selected_place` on success. If the route sheet is open, set
the destination instead.

- [ ] **Step 3: Implement place actions and marker**

Draw `selected_place` as a distinct ring/pin. Add distance via haversine
from `map_center_lat/lon`; Copy uses `ctx.copy_text`; Save uses
`BookmarkStore` (remove if already saved by name+coords).

- [ ] **Step 4: Run the gate and commit**

`[FEATURE] map: tap-to-select, place actions and selected marker`
CHANGELOG: Added — map tap/place parity.

---

### Task 5: Per-friend sharing sheet

**Files:**
- Create: `src/ui/share_sheet.rs` (`ShareSheet` state + draw)
- Modify: `src/ui.rs` (state, `PendingAction::SaveShare`)
- Modify: `src/ui/friends.rs` (accepted friends + Sharing button)
- Modify: `src/ui/sharing.rs` (open sheet instead of duplicate form)
- Test: unit tests for existing-share lookup

**Interfaces:**
- `ShareSheet { friendship_id: Option<i64>, enabled: bool, precision: String, duration_hours: Option<i64>, incoming: String }`
- `ShareSheet::from_existing(share: Option<&LocationShare>)`.
- `PendingAction::SaveShare { friendship_id, enabled, precision, duration_hours }`

- [ ] **Step 1: Write failing lookup tests**

Given shares `[friendship_id=7, is_active=true, precision="exact"]`, the sheet
for friend 7 loads `enabled=true/exact`; friend 8 creates a new default.
- [ ] **Step 2: Implement load/save**

On open, refresh shares and find `share.friendship_id == id`. If found,
PATCH `is_active` + `precision`; if not and enabled, create with expiry.
Revoke = PATCH `is_active=false`. Keep duplicate prevention.
- [ ] **Step 3: Wire Friends UI + sheet**

Accepted friendships list gets "Sharing"; sheet shows "This friend shares
with you: exact/approximate/city only" based on the incoming share.
- [ ] **Step 4: Run the gate and commit**

`[FEATURE] sharing: per-friend sheet with existing-share updates`
CHANGELOG: Added — per-friend sharing sheet.

---

### Task 6: Locate, publish and mobile refresh

**Files:**
- Modify: `src/ui.rs` (`PendingAction::Locate`, refresh helper)
- Modify: `src/ui/map.rs` (locate does not publish; publish button opens sheet)
- Modify: `src/ui.rs` mobile drawer/header (refresh)
- Modify: `src/app.rs` (`locate` wrapper using `platform::default_gps_provider`)
- Test: `src/platform.rs` parse fix tests already exist; add locate-with-no-fix test

**Interfaces:**
- `PinpointApp::locate() -> Option<(f64,f64,f64)>`; sets `my_location` and
  centres the map without publishing.
- `PinpointAppUI::refresh_mobile()` calls `refresh_all`, `refresh_my_location`,
  `refresh_check_ins`, `refresh_emergencies`.
- `mobile_publish_open` is set by a "Publish" control and by the sheet close.

- [ ] **Step 1: Write the failing no-fix test**

With no env/file fix, `locate()` returns `None` and `set_error` is not called
from the platform layer; the UI shows "Location unavailable".
- [ ] **Step 2: Implement and wire**

Replace `PublishGpsLocation` on the floating ◎ with `Locate`; add a separate
Publish action in the drawer/controls. Add refresh to the compact drawer.
- [ ] **Step 3: Run the gate and commit**

`[FIX] mobile: locate without publishing, working publish sheet and refresh`
CHANGELOG: Fixed — mobile locate/publish/refresh.

---

### Task 7: Session resume + auth session APIs

**Files:**
- Modify: `src/api.rs` (`me`, `sessions`, `revoke_session`, `logout_all`,
  session export/import)
- Modify: `src/backend/mod.rs` (trait methods)
- Modify: `src/app.rs` (restore on startup, clear on 401)
- Modify: `src/ui.rs` (startup `RestoreSession` action)
- Test: session JSON round-trip and per-account redaction

**Interfaces:**
- `SessionSnapshot { cookies: Vec<String>, csrf_token: Option<String> }`
  with `to_json`/`from_json`.
- `ApiClient::export_session() -> Option<String>`,
  `ApiClient::import_session(&str) -> anyhow::Result<()>`.
- `Backend::me()`, `list_sessions() -> Vec<SessionInfo>`,
  `revoke_session(id, current)`, `logout_all()`.

- [ ] **Step 1: Write failing tests**

`SessionSnapshot` round-trips; malformed JSON returns `None`; the JSON never
contains a password/email.
- [ ] **Step 2: Implement persistence**

Store the snapshot in the OS keychain under `session_snapshot` via
`KeyManager`; fall back to memory if the keychain is unavailable. On startup,
import and call `/auth/me`; on 401 clear the snapshot and show Login.
- [ ] **Step 3: Run the gate and commit**

`[FEATURE] auth: resume and revoke native sessions`
CHANGELOG: Added — session resume and management.

---

### Task 8: Anonymous account + recovery

**Files:**
- Modify: `src/api.rs` (`anonymous`, `recover`)
- Modify: `src/backend/mod.rs`, `src/app.rs`
- Modify: `src/ui/auth.rs` (buttons, recovery form, code panel)
- Modify: `src/ui.rs` (`recovery_code: Option<String>`, `recover_code`)
- Test: response parsing for `{user, recovery_code}`

- [ ] **Step 1: Write failing parsing test**
- [ ] **Step 2: Implement API + UI**

Anonymous button posts `{device_name: "Pinpoint Android"}`; show the recovery
code with Copy/Dismiss; recover form posts `{recovery_code}` and replaces the
code. Never log the code.
- [ ] **Step 3: Run the gate and commit**

`[FEATURE] auth: anonymous accounts and recovery codes`
CHANGELOG: Added — anonymous/recovery.

---

### Task 9: Sessions UI, diagnostics, appearance and recents

**Files:**
- Modify: `src/ui/profile.rs` / `src/ui.rs` drawer (sessions + logout-all)
- Create: `src/ui/diagnostics.rs` (report form)
- Modify: `src/models.rs` (`Appearance`, `RecentPlace`, `RecentRoute`)
- Modify: `src/map.rs` (label/icon sizes)
- Create: `src/recent.rs` (per-account JSON store)
- Modify: `src/ui/bookmarks.rs` (clear-all, filters)
- Test: recent store key/round-trip; appearance clamping

- [ ] **Step 1: Write failing store tests**
- [ ] **Step 2: Implement sessions/diagnostics**

Sessions list shows auth method, id prefix, last activity and current badge;
Revoke calls DELETE; sign-out-everywhere clears local state regardless of the
network result. Diagnostics posts category/message/platform "android" or
"desktop"; only category/message leave the device.
- [ ] **Step 3: Implement appearance + recents**

Sliders 10–18 (labels) and 18–40 (icons), Reset; persist per device and feed
`map.rs`. Recents store last 20 places/routes per user, clear actions.
- [ ] **Step 4: Run the gate and commit**

`[FEATURE] account: sessions, diagnostics, appearance and recents`
CHANGELOG: Added — account utilities.

---

### Task 10: Admin console parity

**Files:**
- Modify: `src/api.rs` (`admin_overview`, `admin_users`, `admin_update_user`,
  `admin_delete_user`, `admin_diagnostics`, `admin_update_report`,
  `admin_audit`)
- Modify: `src/backend/mod.rs`, `src/app.rs`
- Rewrite: `src/ui/admin.rs` (tabs Overview/Users/Audit/Diagnostics)
- Test: admin response parsing; role gating accepts `admin`/`administrator`

- [ ] **Step 1: Write failing parsing/gating tests**
- [ ] **Step 2: Implement API + tabbed UI**

Auto-load on first open; Users: search, role select, active checkbox, delete
with confirmation; Overview cards; Audit newest-first; Diagnostics status
filter + triage select. Gate with `admin || administrator` and hide the nav
item for residents (server still authoritative).
- [ ] **Step 3: Run the gate and commit**

`[FEATURE] admin: overview, roles, audit and diagnostics triage`
CHANGELOG: Added — admin parity.

---

### Task 11: Compact layouts + final verification

**Files:**
- Modify: `src/ui/*.rs` (compact branches for Friends/Sharing/Places/Profile/
  Admin/Kiosk/Password; scroll areas; no fixed-width overflow)
- Modify: `src/ui.rs` (drawer refresh, admin visibility)
- Update: `android/README.md`, `docs/assets/android-map-first.png`,
  `CHANGELOG.md`, `RELEASE_NOTES.md` if needed.

- [ ] **Step 1: Emulator pass**

Build `cargo apk build --lib --target x86_64-linux-android`, install, verify
map tap/search/route/place/share/sessions/admin against the in-memory server.
- [ ] **Step 2: Full gate + screenshot**
- [ ] **Step 3: Commit**

`[FEATURE] mobile: parity layouts and final parity pass`
CHANGELOG: Changed — mobile parity.

## Self-Review

- Spec coverage: maps to slices 1–3 and the 8 web feature groups in the audit.
- Step scan: each task has a failing test, implementation, gate and commit.
- Type consistency: `SearchPlace`/`Route`/`RoutePlan` names match across tasks;
  `Backend` methods use the same signatures as `ApiClient`.
- Review focus: MFA, route errors, map tap, existing share and session
  revoke each have an owning test.
- Proportion: plan is task-level, not a code transcript.

## Out of Scope Follow-ups

Kiosk share-link backend routes, inbound `pinpoint://`/`geo:` deep links and
the Android LocationManager JNI bridge remain separate platform tasks. They
are not part of `web/app` parity; document the env/file GPS fallback until
the bridge lands.

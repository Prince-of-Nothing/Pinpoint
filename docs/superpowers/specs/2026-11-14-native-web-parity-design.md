# Native Client Web-Parity Design

Date: 2026-11-14 · Status: **Approved** (full parity, 3 slices) ·
Scope owner: Pinpoint thesis demo

Related: `docs/superpowers/specs/2026-11-13-navigation-and-map-ui-design.md`,
ADR-0007 (Rust multiplatform), ADR-0011 (self-hosted OSM), ADR-0012
(opaque sessions), `AGENTS.md`.

## 1. Intent

The Android/eframe client currently covers the shell and a subset of the
active web client in `web/app/`. This design closes the functional gap so a
user can do the same map, sharing, account and admin tasks natively without
falling back to the web UI. It is parity of **flows**, not pixel-identical
Leaflet rendering.

## 2. Goals

- Place search and category search through the Pinpoint backend, with
  map-centre bias, distance and a search-results sheet.
- Map tap → reverse geocode → place sheet; selected-place marker.
- Place actions: Directions, Set as origin, Save, Copy, OSM, Google,
  Organic Maps, Magic Earth, distance from the map centre.
- A→B route planning from places, with stops, avoid motorway/toll/ferry,
  swap, car/walk/bike, alternatives, steps, clear, route share and
  GPX/KML/GeoJSON export.
- Lightweight route following using the platform GPS provider, with a
  next-step banner and route progress; no voice guidance.
- Per-friend share sheet: existing-share lookup, enable/disable, precision,
  duration presets, incoming status, revoke.
- Account lifecycle: MFA code at login, anonymous account + recovery code,
  recover account, session resume, active sessions, per-session revoke and
  sign out everywhere.
- Diagnostics report form; map appearance label/icon sizing; recent places
  and recent routes; saved-place parity.
- Admin console: overview, users (role/active/delete), audit, diagnostics
  triage, correct `admin`/`administrator` gating.
- Compact (Android) layouts for every screen reachable from the mobile
  drawer, plus a refresh action.

## 3. Non-goals

- Pixel-identical Leaflet/CSS rendering.
- Voice guidance, lane guidance, traffic, public transport, vector tiles.
- Kiosk share-link backend, inbound `pinpoint://`/`geo:` deep links and the
  Android LocationManager bridge are **platform follow-ups**, not web
  parity. The client keeps the env/file GPS fallback and documents it.
- Rewriting the web client or adding Kotlin/WebView.

## 4. Constraints

- Rust only (ADR-0007); no Kotlin, no CDN, no Google/Mapbox SDK.
- Server-side enforcement only. All map/search/route calls go through the
  Pinpoint backend (`/map/*`), never directly to Nominatim/OSRM, so the
  privacy envelope and rate limits stay server-side.
- ODbL attribution remains visible on every map view.
- No new secrets or real user data in tests.
- Keep `PinpointApp` depending on `DynBackend`, not `ApiClient`.
- Compact threshold stays `width < 700 pt`; one codebase for desktop and
  mobile.

## 5. Approaches considered

| Option | Why not / why |
|---|---|
| **Port flows into existing egui screens using backend proxies (chosen)** | Reuses the Rust data model, eframe shell and all server endpoints; keeps one codebase; only UI/API glue is new. |
| WebView/Tauri around `web/app` | Exact web UI, but reverses the Rust-only decision and adds Kotlin/Gradle or a second runtime. Rejected by the owner. |
| Keep the current subset | Fastest, but leaves the client unable to plan routes, manage sessions or administer the demo. Rejected. |

## 6. Design

### 6.1 Backend/API surface

Add thin methods to `ApiClient` and the `Backend` trait; `PinpointApp` wraps
them. Existing Axum routes already exist:

- `GET /map/capabilities` → `MapCapabilities { autocomplete, modes, avoid }`
- `GET /map/search?q=&lat=&lon=&limit=` → `{ results: [SearchPlace] }`
- `GET /map/reverse?lat=&lon=` → `SearchPlace`
- `GET /map/route?from_lat=&from_lon=&to_lat=&to_lon=&mode=&via=&avoid=&alternatives=` → `{ routes: [Route] }`
- `POST /api/v1/auth/login` with optional `totp`
- `POST /api/v1/auth/anonymous`, `POST /api/v1/auth/anonymous/recover`
- `GET /api/v1/auth/sessions`, `DELETE /api/v1/auth/sessions/{id}`,
  `POST /api/v1/auth/logout-all`, `GET /api/v1/auth/me`
- `POST /api/v1/diagnostics/report`
- `/api/v1/admin/*`: overview, users, update/delete user, diagnostics,
  audit.

`SearchPlace` gains `distance_m: Option<f64>`; `Route`, `RouteStep` and
`RouteLeg` gain serde derives. No wire-contract change beyond existing
server fields, so `tests/contract.rs` stays authoritative.

### 6.2 Client state

- `RoutePlan` (new `src/ui/route.rs`): origin/destination `RoutePoint`,
  stops, avoid flags, mode, alternatives, selected index, sheet open,
  following flag, next-stop index. The selected route is copied to
  `AppData.last_route` for the map renderer.
- `AppData`: `selected_place`, `map_click`, `map_dark`, appearance sizes,
  `sessions`, `diagnostics`, `recent_places`, `recent_routes`. All default
  empty; no new secrets.
- `PinpointAppUI`: `share_sheet_friend`, route sheet state, recovery code,
  compact refresh/error state.

### 6.3 UI components

- `src/ui/route.rs`: shared route sheet for desktop and mobile, with
  place fields, stops, avoid, swap, mode tabs, alternatives, steps,
  summary, Navigate/Share/GPX/KML/GeoJSON/Clear and external links.
- `src/ui/map.rs`: mobile map tap → reverse, selected marker, place sheet
  parity, route/share sheet entry points.
- `src/ui/friends.rs`: accepted friends list with a Sharing button.
- `src/ui/share_sheet.rs`: per-friend sharing sheet.
- `src/ui/profile.rs` + drawer: sessions, diagnostics form, appearance,
  recents/routes, anonymous/recovery entry points.
- `src/ui/admin.rs`: tabbed Overview / Users / Audit / Diagnostics.
- `src/ui/auth.rs`: TOTP field, anonymous and recovery flows, recovery-code
  panel.
- `src/map.rs`: draw selected place, route progress, configurable label/icon
  sizes, and map-tap hit testing.

### 6.4 Storage

- Session resume: persist the opaque cookie + CSRF token in the OS keychain
  (existing `KeyManager`) as a JSON blob; never plaintext on disk. Restore
  at startup, call `/auth/me`, clear on 401 or logout.
- Saved places, recents and routes: per-account JSON under
  `app_data_dir()/pinpoint/<user_id>/`, with clear actions.
- Appearance: per-device JSON or keychain secret; applies to the map
  renderer and persists across restarts.

### 6.5 Testing

- Unit tests for query builders, coordinate projection round-trips, route
  selection/swap, session JSON, per-account storage keys, TOTP body.
- Router probes for backend map/admin/auth endpoints (existing suite).
- Full gate after each task: fmt, clippy `-D warnings`, build, test.
- Emulator verification for the map-critical and account slices; admin
  verified against the in-memory server.

## 7. Delivery slices

1. **Map-critical**: API proxy/MFA, route planner + following, place sheet
   + map tap, per-friend sharing, locate/publish/refresh.
2. **Account/utilities**: session resume, anonymous/recovery, sessions +
   logout-all, diagnostics, appearance, recents/routes.
3. **Admin**: overview, users/roles/delete, audit, diagnostics triage,
   role gating, compact layouts and final parity audit.

Each slice is committed separately with `[FEATURE]`/`[FIX]` subjects and a
CHANGELOG entry under `[Unreleased]`.

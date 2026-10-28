# Third-Party Notices & Licensing — Always Together Mobile

This document lists every third-party component the mobile app depends on,
its license, and the obligations we fulfill. **Goal: ship 0 trackers and
never break a provider's Terms of Service** (especially OpenStreetMap's).

## Privacy stance (0 trackers)

The app contains **no** analytics, crash-reporting, advertising, or
attribution SDKs — not Firebase, not Google Play Services, not Mapbox, not
Sentry, not Countly, not Bugsnag. Nothing in the dependency tree phones
home except:

1. **Our own API** (`api.always-together.example` / local dev stack) — the
   user's account data lives only there.
2. **The configured tile server** (default: OpenFreeMap) — map tile
   requests only; no user identifiers are attached by us, and MapLibre
   Native ships **no telemetry module** (unlike Mapbox's SDK).

If a future contribution adds any SDK that transmits data, it must be
documented here in the same PR.

## Map data: OpenStreetMap (ODbL)

- **Data**: © OpenStreetMap contributors, available under the
  [Open Database License v1.0 (ODbL)](https://opendatacommons.org/licenses/odbl/1-0/).
- **Obligation — attribution**: the map screen permanently displays
  "© OpenStreetMap contributors" (`MapScreen.kt`, plus MapLibre's built-in
  logo/attribution, which we explicitly keep enabled —
  `uiSettings.isAttributionEnabled = true`).
- **Obligation — share-alike**: if we *redistribute* the database (e.g.
  bundle downloaded tiles in an APK or on our own server), the ODbL's
  share-alike clause applies to that redistribution. Offline packs
  downloaded **by the end user onto their own device** for personal use are
  governed by the provider's tile-usage terms, not a redistribution by us.
- **Tile usage policy**: public OSM tile servers
  (tile.openstreetmap.org) must **not** be used for bulk/offline downloads
  or as a default app tile source — see the
  [OSMF Tile Usage Policy](https://osmfoundation.org/wiki/Tile_Usage_Policy).
  **We therefore never default to tile.openstreetmap.org.**

## Default tile/style provider: OpenFreeMap

- **What**: community-run, donation-funded vector tile + style service
  (https://www.openfreemap.org).
- **License**: the tiles and styles are derived from OSM data (ODbL); the
  software serving them is open source (BSD/MIT family).
- **Why chosen**: zero API keys, zero telemetry, zero usage tracking,
  compatible with the 0-tracker requirement. Attribution requirement is
  the standard OSM credit (fulfilled above).
- **Ops note**: OpenFreeMap is volunteer infrastructure. Production
  deployments with real traffic should either
  [donate](https://www.openfreemap.org/donate/) or self-host tiles from
  Geofabrik extracts (ODbL, attribution-preserving) — e.g. with Martin or
  TileServer GL. Tile source is configuration, not code: see `MapConfig`.

## SDKs and libraries

| Component | Version | License | Notes / obligations |
|---|---|---|---|
| MapLibre Native (Android) | 11.5.1 | **BSD-2-Clause** | No telemetry module. Renders OSM-derived tiles; UI attribution kept enabled by us. |
| Kotlin stdlib | 2.0.21 | Apache-2.0 | |
| AndroidX Core KTX, Lifecycle, Navigation, DataStore, Activity | see `gradle/libs.versions.toml` | Apache-2.0 | Google-maintained, no services attached. |
| Jetpack Compose (BOM 2024.10.01) | | Apache-2.0 | |
| Retrofit | 2.11.0 | Apache-2.0 | HTTP client for **our API only**. |
| OkHttp + logging-interceptor | 4.12.0 | Apache-2.0 | Logging at BASIC level; headers/bodies (tokens, passwords, coordinates) never logged. |
| kotlinx.serialization | 1.7.3 | Apache-2.0 | |
| kotlinx.coroutines | 1.9.0 | Apache-2.0 | |
| JUnit / Truth (tests only) | 4.13.2 / 1.4.4 | EPL-1.0 / Apache-2.0 | Not shipped in the APK. |

Apache-2.0 obligations: retain the license and NOTICE text (satisfied by
Gradle's dependency resolution shipping the licenses inside the app's
open-source distribution; a generated OSS attribution file should be added
to the Play Store listing's "open source licenses" section — build task
TODO tracked in `docs/TASKS.md`).

## Navigation hand-off (geo: URIs)

In-app offline routing is delivered as an **intent export** (board T14):
"Navigate here" emits a standard `geo:` URI that external navigation apps
handle. If the user has an offline-capable nav app installed (OsmAnd,
Organic Maps, …) with its own downloaded maps, navigation works fully
offline. Those apps' licenses/terms are theirs, not ours; we merely launch
them via the public Android intent system. A future in-app engine
(GraphHopper Apache-2.0 / Valhalla BSD-3, both OSM-data compatible) plugs
into `RoutingEngine` without violating any of the above.

## Data protection posture (user security)

- Tokens: Android Keystore AES-GCM (hardware-backed when available);
  plaintext exists only in process memory.
- `allowBackup="false"`: app data (including encrypted tokens) never
  leaves the device via Android backup.
- Cleartext HTTP blocked by network security config except to local dev
  hosts; production traffic is HTTPS-only.
- Location leaves the device **only** through our API, and only for
  friends with an active, server-side-consented share.
- No account deletion / data export exists yet in the mobile app (web has
  it); tracked for a future increment.

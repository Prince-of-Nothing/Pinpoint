# Always Together — Android App

Kotlin/Jetpack Compose client for the Always Together API. Privacy-first:
**0 trackers**, server-side-consented location sharing, offline maps, and
offline navigation hand-off. Licensing obligations (OSM/ODbL etc.) are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Status

M1 scaffold — the full architecture plus four working screens (login, map
with offline downloads, friends, sharing). Build verification happens in CI
(the project needs JDK 17 + Android SDK; see `.github/workflows/ci.yml`).

## Module map (Clean Architecture)

```
mobile/
├── app                  # composition root: MainActivity, NavHost, manual DI (Container)
└── core/
    ├── domain           # pure Kotlin: models, repository interfaces, RoutingEngine
    ├── network          # Retrofit/OkHttp only; DTOs mirror the Go API JSON
    ├── data             # repository impls; Keystore-encrypted SessionStore
    └── map              # MapLibre wrapper, OfflineMapManager, geo: nav export
└── feature/
    ├── auth             # login (depends on :core:domain only)
    ├── map              # map screen + offline downloads UI
    ├── friends          # friend requests / circle list
    └── sharing          # per-friend location sharing consent
```

Dependency rules (enforced by Gradle, not convention):

- `:feature:*` → `:core:domain` only (screens cannot see HTTP, storage, or map SDKs).
- `:core:data` implements domain interfaces over `:core:network` + Keystore.
- `:core:network` is the only module importing Retrofit/OkHttp.
- `:core:map` is the only module importing MapLibre.
- DI: manual composition root (`app/di/Container.kt`) — no Hilt/KSP.

## Building

Requirements: JDK 17, Android SDK 35, Gradle 8.10+ (or Android Studio
Ladybug+ which bundles these).

```bash
# From repo root — debug APK against the local docker compose backend:
# (Android emulator reaches the host via 10.0.2.2)
gradle -p mobile :app:assembleDebug

# If you have the Gradle wrapper installed once:
cd mobile && gradle wrapper && ./gradlew :app:assembleDebug
```

Run the backend the app talks to: `docker compose up` (port 8000), then
launch the app; the debug build points at `http://10.0.2.2:8000/api/v1/`.

## Features in M1

- **Login** with session restore (encrypted tokens survive restarts;
  logout wipes memory + disk).
- **Map** (MapLibre, OpenFreeMap tiles, OSM attribution enforced).
- **Offline maps**: download the current area (≈11 km box, zoom 8–15) to
  app-private storage; list/delete downloads; renders offline when the
  network is gone.
- **Navigation hand-off**: "Navigate here" exports a `geo:` intent to any
  installed nav app (works offline if that app has offline maps).
- **Friends**: invite by email, accept/reject requests.
- **Sharing**: per-friend on/off switch + precision; revocation is
  server-side.

## Roadmap (space for improvement)

- Fused location provider → publish own location + friend markers on the map
  (board T14/T21; while-in-use permission already declared).
- Real offline routing engine behind `RoutingEngine` (GraphHopper/Valhalla).
- Self-hosted tile server option (fleet deployments, zero third-party contact).
- Unit tests for ViewModels (all state logic is already framework-free).
- Release signing config + OSS attribution generation task.

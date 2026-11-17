# Android & desktop packaging for multi-region offline packs

**Status:** audit, 2026-11-17 · branch `feat/global-maps` · no code.
Related: [design](../superpowers/specs/2026-11-17-global-maps-design.md),
[ADR-0021](../adr/0021-global-map-catalog.md), [catalog-security.md](./catalog-security.md),
[android-webview-online.md](./android-webview-online.md).

## 1. Embedded `web/map/*` assets

`src/mobile/map_server.rs` `include_bytes!`s 11 files into
`libpinpoint_desktop.so` (~1.8 MiB; `maplibre-gl.js` 1.1 MiB and its worker
520 KiB dominate) and serves only the exact-path `ASSETS` table; anything
unlisted is 404. Gradle does not rebuild them — committed source, re-embedded
by `cargo apk build`. The only embedded `.pmtiles` is the 94 KiB Monaco demo
fixture.

Desktop differs: `src/map_server.rs` serves the `web/` tree from disk
(`PINPOINT_WEB_ROOT`, cwd, next to the executable), so a file missing from
`ASSETS` passes desktop QA and 404s only in the APK. A region-list UI must add
every new HTML/CSS/JS file and the bundled catalog fallback JSON to `ASSETS`;
unlisted files (e.g. `web/vendor/sqlite3.wasm`) never reach the APK.

## 2. Storage and permissions

| Target | `app_data_dir()` | packs root |
|---|---|---|
| Android | `/data/data/com.pinpoint.desktop/files` (hardcoded, no JNI `getFilesDir()`) | `files/packs/<region>/` |
| Desktop | `~/.pinpoint` (`HOME`/`USERPROFILE`) | `%USERPROFILE%\.pinpoint\packs\<region>\` |

App-private Android storage is uid-isolated under SELinux, so `create_dir_all`,
`.part` writes, rename and delete need **no storage permission and no SAF** —
the manifest declares none. Cleartext is already allowed for loopback.
Caveats: the hardcoded path is user 0 (`/data/data` → `/data/user/0`), so a
secondary user or work profile gets EACCES; and `android:allowBackup` is unset
(default true), so exclude `packs/` via `dataExtractionRules` or Auto Backup
silently stops at its 25 MB quota.

Budget per region ≈ 0.92× + 1.09× the PBF (Moldova 204 MB installed, Ukraine
~1.7 GB). An update holds the old copy and the new `.part` at once (~2× the
region size). There is **no free-space preflight**; ENOSPC is discovered
mid-download (clean failure, old pack intact).

## 3. APK size impact

No catalog pack is bundled: packs live in app data and arrive by download. The
universal APK carries two `.so` files, i.e. the ~1.8 MiB asset block twice,
and `useLegacyPackaging`/`extractNativeLibs` compress libs in the APK and
extract them at install, so APK growth is smaller than installed growth. A
region-list UI adds kilobytes, not megabytes.

## 4. Legacy root files → `packs/<region>/`

`packs/moldova.pmtiles` and `packs/moldova.graph.bin` move into `packs/moldova/`
with the same names. Same volume, so `rename` is a same-directory move and
EXDEV is not expected. Move both or neither, idempotent, skip while a job runs,
verify size/SHA before adopting, never overwrite, destination-exists = success.

Android needs nothing extra. On Windows the current installer treats a failed
`rename` as fatal; Rust opens files with `FILE_SHARE_READ|WRITE|DELETE`, so the
loopback reader does not block the move, but Defender, backup/sync agents and
indexers can (os error 5 or 32). Fall back to copy → `sync_all` → rename →
delete, retry once, and keep serving the legacy path until it succeeds. A
half-migrated layout must roll back or report the graph missing.

## 5. Loopback routes, Range and WebView origin

Both routers now mount `GET|HEAD /packs/:region/:name` beside the legacy
`/packs/:name`; the handler lands with the catalog slice. `valid_pack_name`
(rejects `/ \ :`, whitespace, leading dot; requires `.pmtiles`) cannot be
reused for the region form: `region` must match `^[a-z0-9-]{1,64}$`, `name`
must come from that region's catalog `installed_name` list, and the result must
canonicalize under `packs/`. Keep `206`/`416`, `Content-Range`,
`Accept-Ranges: bytes`, `application/octet-stream`, `no-cache`, `nosniff`, and
streamed `take(length)` per request. Concurrent PMTiles ranges serialize on the
`new_current_thread` runtime without buffering; `.part` files are unservable.

The document origin is `http://127.0.0.1:<ephemeral port>` and the WebView pins
exact host+port; `/packs/...` is same-origin, so no CORS. CSP and
`Referrer-Policy` follow [android-webview-online.md](./android-webview-online.md):
`connect-src 'self'` covers every region pack, `worker-src 'self'` (no `blob:`),
and the two inline `<script>` blocks need `'unsafe-inline'` or hashes —
externalising them is the cheaper tightening. The port-scoped origin wipes
`localStorage` on every cold start, so the selected region must not live only
there.

## 6. Build and test

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked; cargo test --locked
cargo test --locked map_server offline_install

cargo check --locked --target aarch64-linux-android --lib
cd android
.\gradlew.bat :app:assembleDebug -Pabi=arm64-v8a           # device debug
.\gradlew.bat :app:assembleRelease -Pabi=universal -PrustProfile=release
```

Needs `ANDROID_HOME`, `ANDROID_NDK_HOME` (27.2.x), JDK 21. Cannot be verified
here: real WebView Range traffic for a 90 MB+ pack, ENOSPC, secondary-user
storage, Windows rename under a live scanner, install/update/delete timings,
installed footprint, CSP enforcement in the system WebView, backup/restore.

## 7. Top 5 risks

1. **Route widening.** First caller-supplied multi-segment path; sloppy
   validation means traversal or reading another region. Catalog lookup +
   canonicalize + 404.
2. **Windows migration.** A scanner-held file turns "upgrade" into "no offline
   pack" if rename is fatal. Copy fallback, retry, legacy path until success.
3. **Hardcoded Android path.** User 0 only; secondary users/work profiles
   cannot install. Fix with a `getFilesDir()` JNI call.
4. **Disk exhaustion.** Ukraine-sized pack + `.part` + old copy needs ~3.4 GB
   and there is no preflight; a long download ends in ENOSPC. Show sizes and
   check free space first.
5. **CSP vs. inline page.** A strict policy breaks the inline theme/module or
   the worker; a loose one leaves `connect-src *` on a page that drives the
   installer. Ship the header, keep the OpenFreeMap toggle off by default.

## 8. QA checklist

- [ ] Fresh install, no packs: map opens on the Monaco fixture, `/map/config`
      reports it, `/packs/...` is 404 not 500.
- [ ] Download Moldova: both files in `files/packs/moldova/`, sizes match the
      catalog, no `.part`, map switches to the pack.
- [ ] Range: debug DevTools shows `206` + `Content-Range` for
      `/packs/moldova/moldova.pmtiles`; `HEAD` gives length and no body.
- [ ] Path battery: `..`, `%2e%2e`, `\`, `:`, unknown region, unknown file,
      uppercase extension, `.part` → 404, never 200/500.
- [ ] Legacy migration: seed v2.3.0 root files, launch, files appear under
      `packs/moldova/`, no re-download; second launch is a no-op.
- [ ] Migration while a job runs is skipped; kill mid-move and relaunch: whole
      layout or clean rollback, both files usable.
- [ ] Windows: hold the legacy file with another tool, migrate; retry/fallback
      succeeds, and the legacy path still serves if it does not.
- [ ] Low disk: download fails cleanly, old pack still served, `.part` removed,
      page shows the error.
- [ ] APK: `unzip -l` shows no loose `*.pmtiles`/`*.graph.bin`; record APK size
      and `adb shell du -sh files/packs` after install.
- [ ] Universal APK installs and renders the map on an arm64 device.
- [ ] CSP: toggle online on/off, no console violations for `/packs/*`, worker
      or OpenFreeMap; offline pack still wins.
- [ ] Backup/restore excludes packs; gate (fmt, clippy, build, tests) green.

**Blockers.** (1) The branch is mid-slice: both routers mount `/map/catalog`
and `/packs/:region/:name` while `src/offline_install.rs` still exports only
the single-region handlers, so it does not compile at audit time and nothing
in §4–§5 can be QA'd until that lands. (2) The hardcoded `/data/data/...` path
blocks multi-user support. (3) No free-space preflight. No packaging blocker:
no catalog pack is embedded and `packs/<region>/` needs no SAF or permission.

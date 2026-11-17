# android/ — Android client (Rust core + thin Java bridge)

The Android app is the same `pinpoint_desktop` crate as desktop/web. The Rust
core is still built with **cargo-apk 0.10** for NDK/linker handling and uses
the eframe/winit `android-native-activity` backend. **Gradle** packages the
`.so`, the Java bridge, resources and manifest into the APK (ADR-0017).

**Status:** builds, installs and runs on the API 37 x86_64 emulator (tested on
Windows with Android Studio's SDK). The map uses live OSM tiles through the
Pinpoint backend and caches them on-device. Release signing is handled by
`.github/workflows/release.yml`; a physical AArch64 device run is still a
follow-up.

## Device features

- **One-shot GPS:** `Use GPS` asks for the runtime location permission, then
  reads the last known fix from every LocationManager provider. If none is
  recent, it actively requests a fix for up to 8 seconds (GPS → fused →
  network) and removes the request afterwards.
- **Publish deep link:** `pinpoint://publish` publishes the current location on
  launch; it waits for a restored session before firing.
- **Share out:** place sheets have **Share** (`geo:lat,lon?q=name`) and the Kiosk
  QR card has **Share link** (`pinpoint://kiosk?token=...`), both opening the
  system share sheet.
- **Share-in:** Pinpoint appears in Android's text-share sheet. The small
  `PinpointActivity` subclass overrides `onNewIntent` and calls `setIntent`, so
  every share is captured (cold start and while an instance is alive), then
  parsed by the allowlisted `MobileDeepLink::parse_shared`. A `nativeWakeUi`
  JNI call repaints an idle UI. Verified with two consecutive shares.
- **Biometric unlock:** optional Profile toggle; the platform
  `BiometricPrompt` (API 28+) unlocks the existing PIN lock. The PIN remains
  the fallback. State is polled from Rust; no biometric data is stored.
- **Background location:** Profile → **Background sharing** starts a
  non-exported foreground service with a persistent notification. It publishes
  GPS/network/passive fixes at 15–600 s through the same Rust session, consent
  and encryption paths, and stops from the same screen. Android's "all the
  time" location and notification permissions are requested explicitly.
- **Deep links:** `pinpoint://map`, `pinpoint://kiosk?token=...`, `pinpoint://admin`
  and `geo:` are routed on launch.
- **FLAG_SECURE:** screenshots and screen recording of the app window are
  blocked. Verified on the emulator: `screencap` returns a black app window.
- **On-device PIN lock:** Profile → App lock stores only an Argon2id hash in
  `files/lock.json`; the app locks on launch and when returning from the
  background. Wrong-attempt count and lockout deadline are persisted, so
  force-stopping does not reset the delay.
- **Android 15:** the mobile tab bar reserves the gesture-navigation inset.
- **Pinch-to-zoom** works on the map.
- **MapLibre WebView (prototype):** the compact map screen's **▦** control
  starts a Rust loopback server and opens the shared `web/map/` MapLibre page
  in a full-screen WebView. See [MapLibre WebView](#maplibre-webview-prototype).
- **Saved places on the map:** bookmarks render as blue pins with their names.
  Tapping a pin opens its place sheet (Directions, Set as origin, Save, Copy,
  Share, map apps) without a network lookup. The Saved screen offers **Map**
  and **Share** for each place, and the map updates immediately after
  add/remove.
- **Debug UI automation:** debug builds skip FLAG_SECURE when
  `files/allow_screenshots` exists (`adb shell run-as com.pinpoint.desktop touch
  files/allow_screenshots`). Release builds ignore the marker.

## MapLibre WebView (prototype)

The merged `web/map/` foundation page also runs inside the Android app. The
compact map screen has a **▦** control next to the zoom/locate/route controls;
it opens a full-screen WebView over the egui surface with a **Close map** button
(that button is the overlay's back control; the egui app stays underneath).

### Why a loopback server

ES modules, the MapLibre worker and PMTiles byte ranges need an HTTP origin, so
`src/mobile/map_server.rs` runs a tiny Axum/Tokio server on `127.0.0.1:0` and
loads `http://127.0.0.1:<ephemeral>/map/index.html`. It is started once per
process and reused when the overlay is reopened.

The server:

- serves the unchanged `web/map/index.html`, `preview.css`, `map.js`,
  `style.json` and `fixtures/monaco.pmtiles`, plus `web/vendor/maplibre-gl.js`,
  `maplibre-gl-worker.mjs`, `maplibre-gl.css` and `pmtiles.js`, embedded in the
  Rust binary at build time;
- serves an **exact-path table only** (no directory listing, no path traversal)
  with the same MIME map as `web/map/serve.mjs`;
- advertises `Accept-Ranges: bytes` and implements single `bytes=` ranges with
  `206 Partial Content` and `416 Range Not Satisfiable`, which PMTiles requires;
- serves downloaded packs from `<files>/packs/<name>.pmtiles` at
  `/packs/<name>.pmtiles`, canonicalised under the packs directory. Bodies
  (full or ranged) are streamed, so serving the 93 MB Moldova archive never
  buffers it in memory; single `bytes=` ranges still win for PMTiles;
- mounts the shared native installer from `src/offline_install.rs`:
  `GET /map/offline/status`, `POST /map/offline/download` and
  `GET /map/config`. The Rust process streams both release artifacts to
  `packs/.<name>.part`, verifies exact bytes + incremental SHA-256, then
  atomically renames them. The page shows progress and reloads onto
  `/packs/moldova.pmtiles`; there is no GitHub fetch from the WebView;
- never serves account data, session cookies or CSRF tokens.

Adding a file to `web/map/index.html` also means adding it to the table in
`src/mobile/map_server.rs`; the embedded copies are rebuilt with the app.

### WebView security

`PinpointActivity` creates the WebView with JavaScript and DOM storage enabled
(the page keeps its theme in `localStorage`) but file/content access disabled,
mixed content blocked, multiple windows disabled and geolocation disabled. The
`WebViewClient` allowlist is the exact loopback host **and port**: only
`http://127.0.0.1:<server port>/...` may navigate. Other `http`/`file`/
`content`/`intent`/`javascript` URLs are blocked. The page's https attribution
links (OSM, OpenMapTiles, ODbL) open in the system browser instead.

The WebView has its own cookie jar and storage; it is never given the Rust
session cookie or CSRF token. `PinpointBridge` is the only JavaScript interface
and exposes just the two methods below.

### Bridge contract

| Direction | Contract |
|---|---|
| Page → Java | `PinpointBridge.onMapClick(latitude, longitude)` reports a map click. |
| Page → Java | `PinpointBridge.onShareRequest(latitude, longitude, label)` opens the system share sheet with `geo:lat,lon?q=label`. |
| Java → Rust | `nativeMapClick(double, double)` validates the point, stores it and wakes egui. `nativeMapWebViewClosed()` clears the open flag. |
| Java → page | On `onPageFinished`, a fixed bootstrap subscribes to the preview's `window.__pinpointPreview.handle.onClick`; no page source is modified. A production page can call `PinpointBridge.onMapClick` from its own handler instead. |

After the overlay closes, the compact map screen shows a **⌖** share control
for the last WebView click and re-centres the egui map on it. No session or
location data is passed into the WebView.

### Build and verify

```powershell
cargo check --locked
cargo test --locked map_server

# Android target check (Gradle/cargo-apk normally adds the NDK clang to PATH):
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:ANDROID_NDK_HOME\toolchains\llvm\prebuilt\windows-x86_64\bin;$env:PATH"
cargo check --locked --target aarch64-linux-android --lib

cd android
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-21.0.13+9" # JDK 21, not 25
.\gradlew.bat :app:assembleDebug -Pabi=arm64-v8a
```

The loopback server tests check the page/vendor MIME types, range requests
(`206`/`416`), HEAD, traversal rejection and pack serving from a temporary
`packs/` directory. The installer has fixture tests for a successful stream +
SHA-256 install, a wrong-hash rejection that leaves the old file untouched,
status transitions and `409` while running. The prototype was built as an
arm64 debug APK; no emulator or phone was attached in this environment, so the
on-device render is still a follow-up. Android Studio's emulator with host GPU is the recommended device
check; a headless SwiftShader emulator can show the system-UI warning described
under [Known limitations](#known-limitations).

## Prerequisites

- Android Studio with SDK **platform 35** (`platforms/android-35`),
  build-tools 36, platform-tools and an emulator image.
- **NDK 27.2.12479018** under `$ANDROID_HOME/ndk/27.2.12479018`.
- Rustup stable with `x86_64-linux-android` (emulator) and/or
  `aarch64-linux-android` (devices):
  `rustup target add x86_64-linux-android aarch64-linux-android`.
- `cargo install cargo-apk --locked` (builds the Rust `.so`).
- **JDK 21** for Gradle/AGP 8.7. JDK 25 is too new for AGP 8.7.
- The MSVC build tools for the host-side build scripts.

## Build and install

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-21.0.13+9"

cd android
# Emulator (x86_64); Gradle invokes cargo-apk and copies the .so:
.\gradlew.bat :app:assembleDebug -Pabi=x86_64
adb install -r -d -t app\build\outputs\apk\debug\app-debug.apk
adb shell am start -n com.pinpoint.desktop/.PinpointActivity

# Devices: universal APK (arm64-v8a + armeabi-v7a), release .so:
.\gradlew.bat :app:assembleRelease -Pabi=universal -PrustProfile=release
```

Gradle output is an *unsigned* release APK; the release workflow zipaligns and
signs it. `cargo apk build --lib` is only the Rust build step now; do not
install the APK it writes, because it has no Java DEX or service declaration.

## Releases (GitHub Actions)

`.github/workflows/release.yml` is adapted from the ArtCraft release pattern:

- **Push a `v*` tag** whose version matches `Cargo.toml` (for example
  `v1.3.4`). The workflow builds one universal Gradle APK containing
  `arm64-v8a` and `armeabi-v7a`, zipaligns and signs it with the release key,
  verifies the certificate and creates a **draft** GitHub release named after
  the tag with an empty "What's Changed" section.
- **Publish the draft** on GitHub. It becomes the latest release, and the
  in-app About → Updates checker will detect it.
- **Manual run** (Actions → Release → Run workflow) is a dry run: it builds
  the version in `Cargo.toml`, keeps the APKs as the run artifact and releases
  nothing. Without release secrets it uses a throwaway key and warns.

Create the release keystore once (JKS supports separate store/key passwords):

```bash
keytool -genkeypair -v -storetype JKS \
  -keystore pinpoint-release.jks \
  -alias pinpoint \
  -keyalg RSA -keysize 4096 -validity 10000 \
  -storepass "$PINPOINT_KEYSTORE_PASSWORD" \
  -keypass "$PINPOINT_KEY_PASSWORD" \
  -dname "CN=Pinpoint, O=Pinpoint, C=MD"
base64 -w0 pinpoint-release.jks > pinpoint-release.jks.b64
```

Add these repository secrets (Settings → Secrets and variables → Actions):

| Secret | Value |
|---|---|
| `PINPOINT_KEYSTORE_BASE64` | contents of `pinpoint-release.jks.b64` |
| `PINPOINT_KEYSTORE_PASSWORD` | keystore store password |
| `PINPOINT_KEY_ALIAS` | key alias (`pinpoint` in the example) |
| `PINPOINT_KEY_PASSWORD` | key password; optional when it equals the store password |

Tagged releases **fail** without `PINPOINT_KEYSTORE_BASE64`,
`PINPOINT_KEYSTORE_PASSWORD` and `PINPOINT_KEY_ALIAS`; a release is never
signed with the temporary key. The temporary key and decoded release keystore
are deleted in an `always()` step.

## Crash reports and logs

- Rust panics and `eframe::run_native` startup errors write a redacted report
  under `files/crash-reports/` in the app sandbox.
- A rolling log lives under `files/logs/pinpoint.log`.
- On the next launch, About → Troubleshooting shows the report. **Email report**
  opens the phone's mail app with a `mailto:` link; nothing is sent
  automatically.
- Pre-Rust/native crashes (linker failures, SIGSEGV) still need
  `adb logcat -d | findstr /i "pinpoint AndroidRuntime linker"`.

## Connecting a real phone to a local backend

A **debug** build defaults to `http://10.0.2.2:8080` and accepts the loopback
fallback, so a USB phone can reach a laptop server:

```powershell
adb reverse tcp:8080 tcp:8080
$env:BIND_ADDR = "0.0.0.0:8080"
cargo run --bin pinpoint-server
```

Open Pinpoint and sign in; the reverse tunnel reaches the laptop server and
clears the Offline badge. A remote backend must use HTTPS; non-loopback plain
HTTP is rejected by the transport policy.

A **release** build with no configured backend now marks the backend
unavailable: it does not restore a session and disables sign-in, because any
app on the phone can bind the shared loopback port and impersonate the server.
Use an explicit HTTPS `PINPOINT_BACKEND` for release installs.

## Manifest and resources

cargo-apk 0.10 **generates** `AndroidManifest.xml` from
`[package.metadata.android]` in `Cargo.toml`. It injects NativeActivity, the
`android.app.lib_name` meta-data, the launcher intent and the debug flag
automatically. The launcher icons live under `android/res/mipmap-*/`.

The old hand-written `AndroidManifest.xml` / `network_security_config.xml`
files were removed because cargo-apk 0.10 ignores them; keep manifest changes
in `Cargo.toml`.

## Backend and tiles

- Android emulator reaches the host at **`http://10.0.2.2:8080`** (the default;
  8080 avoids the Docker gateway that often owns 8000).
- The transport policy allows `10.0.2.2` in **debug** builds only. Release
  builds must use HTTPS; remove `uses_cleartext_traffic = true` from the
  release manifest/profile.
- Map tiles are fetched through `{backend}/map/tiles/{z}/{x}/{y}` and cached
  under `/data/data/com.pinpoint.desktop/files/tiles`. A self-hosted
  `PINPOINT_TILE_URL` remains the production path (ADR-0011); never use
  `tile.openstreetmap.org`.
- The app no longer starts offline-first; it uses the live backend and falls
  back to cached tiles. The map remains usable without a session; no demo pins are shown.

## UI

The mobile shell switches to a compact layout below 700 pt: a top bar with a
drawer, a map-first screen (live tiles, locate, publish sheet, provider
buttons), touch-sized controls and scrollable location lists. Kiosk check-in
and the admin console are reachable from the drawer and deep links
(`pinpoint://kiosk?token=...`, `pinpoint://admin`).

## Known limitations

- **MapLibre WebView prototype:** the page starts on the Monaco fixture and
  has an **Offline map** card that downloads the Moldova PMTiles archive and
  routing graph into `files/packs/` through the Rust process. It reloads onto
  `/packs/moldova.pmtiles` after install; there is no pack picker or delete
  control yet. The full-screen overlay is Android-only; desktop/iOS keep the
  egui/native map.
- The WebView bridge edits no `web/map/*` file: it binds the preview's existing
  map handle. A production page should call `PinpointBridge.onMapClick` and
  `PinpointBridge.onShareRequest` directly.
- No device/emulator was attached when this prototype was built, so the
  WebView render and the loopback server on-device were not exercised here.
- Debug APK is ~130 MB for arm64 (and ~185 MB for x86_64 with debug symbols);
  the signed release APKs are ~5 MB per ABI.
- The arm64 build is linked with `-Wl,-z,max-page-size=16384` (see
  `.cargo/config.toml`) for Android 15+ 16 KB-page devices. A physical AArch64
  device run is still pending; an Android 15+ 16 KB device is the target for
  the first hardware validation.
- Headless emulators with SwiftShader can show Android's "System UI isn't
  responding" dialog under low memory; use Android Studio's emulator with host
  GPU for the demo.
- iOS remains a separate `cargo-xcodebuild` target and needs macOS/Xcode.

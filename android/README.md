# android/ — Android client (Rust, `cargo-apk`)

The Android app is the same `pinpoint_desktop` crate as desktop/web, packaged
with **cargo-apk 0.10** using an `android.app.NativeActivity` entry
(`android_main`) and the eframe/winit `android-native-activity` backend.

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
- **Share in:** share a `geo:` link, a Google/OSM/Apple Maps link or plain
  `lat, lon` from another app and Pinpoint opens the map there
  (`ACTION_SEND`, `text/plain`). Android delivers repeat shares to
  `onNewIntent`, which the Rust-only `NativeActivity` cannot observe; close
  Pinpoint before sharing if it is already running.
- **Share out:** place sheets have **Share** (`geo:lat,lon?q=name`) and the Kiosk
  QR card has **Share link** (`pinpoint://kiosk?token=...`), both opening the
  system share sheet.
- **Deep links:** `pinpoint://map`, `pinpoint://kiosk?token=...`, `pinpoint://admin`
  and `geo:` are routed on launch.
- **FLAG_SECURE:** screenshots and screen recording of the app window are
  blocked. Verified on the emulator: `screencap` returns a black app window.
- **On-device PIN lock:** Profile → App lock stores only an Argon2id hash in
  `files/lock.json`; the app locks on launch and when returning from the
  background.
- **Android 15:** the mobile tab bar reserves the gesture-navigation inset.
- **Pinch-to-zoom** works on the map.
- **Debug UI automation:** debug builds skip FLAG_SECURE when
  `files/allow_screenshots` exists (`adb shell run-as com.pinpoint.desktop touch
  files/allow_screenshots`). Release builds ignore the marker.

## Prerequisites

- Android Studio with SDK **platform 35** (`platforms/android-35`),
  build-tools 36, platform-tools and an emulator image.
- **NDK 27.2.12479018** under `$ANDROID_HOME/ndk/27.2.12479018`.
- Rustup stable with `x86_64-linux-android` (emulator) and/or
  `aarch64-linux-android` (devices):
  `rustup target add x86_64-linux-android aarch64-linux-android`.
- `cargo install cargo-apk --locked`.
- JDK 17+ (the JDK 25 used by Android Studio works) and the MSVC build tools
  for the host-side build scripts.

## Build and install

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot"

# Emulator (x86_64):
cargo apk build --lib --target x86_64-linux-android
adb install -r -t target/debug/apk/pinpoint.apk
adb shell am start -n com.pinpoint.desktop/android.app.NativeActivity

# Devices (AArch64): replace the target and build a fat APK if needed:
# cargo apk build --lib --target aarch64-linux-android
```

`cargo apk build --lib` is required because the package also exposes desktop
binaries; without `--lib`, cargo-apk tries to package the `bin` artifact.

## Releases (GitHub Actions)

`.github/workflows/release.yml` is adapted from the ArtCraft release pattern:

- **Push a `v*` tag** whose version matches `Cargo.toml` (for example
  `v1.3.4`). The workflow builds one APK for `arm64-v8a` and one for
  `armeabi-v7a`, re-signs both with the release key, verifies the certificate
  and creates a **draft** GitHub release named after the tag with an empty
  "What's Changed" section.
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
  back to cached tiles. The `Demo:` pins remain visible without a session.

## UI

The mobile shell switches to a compact layout below 700 pt: a top bar with a
drawer, a map-first screen (live tiles, locate, publish sheet, provider
buttons), touch-sized controls and scrollable location lists. Kiosk check-in
and the admin console are reachable from the drawer and deep links
(`pinpoint://kiosk?token=...`, `pinpoint://admin`).

## Known limitations

- Debug APK is ~185 MB (debug symbols, x86_64); the signed release APKs are
  ~5 MB per ABI.
- The arm64 build is linked with `-Wl,-z,max-page-size=16384` (see
  `.cargo/config.toml`) for Android 15+ 16 KB-page devices. A physical AArch64
  device run is still pending; an Android 15+ 16 KB device is the target for
  the first hardware validation.
- Headless emulators with SwiftShader can show Android's "System UI isn't
  responding" dialog under low memory; use Android Studio's emulator with host
  GPU for the demo.
- iOS remains a separate `cargo-xcodebuild` target and needs macOS/Xcode.

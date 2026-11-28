# Packaging

`.github/workflows/release.yml` builds every release artifact. A `v*` tag on
`main` runs all jobs and creates a **draft** GitHub release with
`SHA256SUMS.txt`. A manual run (workflow_dispatch) builds the same files and
releases nothing.

| Platform | Artifact | Built by | Signing today | Verified in CI |
| --- | --- | --- | --- | --- |
| Android | `pinpoint-android-<v>.apk` (universal arm64 + armv7) | `apk` job, Gradle + cargo-apk | Release key (set) | Signature, version |
| Android | `pinpoint-android-<v>.aab` (Play upload) | `apk` job, Gradle `bundleRelease` | Release key (set), jarsigner | `jarsigner -verify` |
| iOS | `pinpoint-ios-<v>-unsigned.ipa` | `ios` job, macOS runner, `aarch64-apple-ios` | None | arm64 Mach-O, Info.plist, IPA layout |
| Windows | `pinpoint-desktop-windows-x64-<v>.msi` | `windows` job, cargo-packager (WiX) | None | Installs, runs `--help`, uninstalls |
| Windows | `pinpoint-desktop-windows-x64-<v>.zip`, `pinpoint-server-windows-x64-<v>.zip` | `windows` job | None | Desktop and server start, `/health`, `/privacy` |
| macOS | `pinpoint-desktop-macos-<v>.dmg` | `macos` job, `hdiutil` | Ad-hoc (no Developer ID) | Mounts, `codesign --verify` |
| macOS | `pinpoint-desktop-macos-<v>.zip` (Pinpoint.app), `pinpoint-server-macos-<v>.zip` | `macos` job | Ad-hoc | Launches, server serves pages |
| Linux | `pinpoint-desktop-linux-amd64-<v>.deb` | `linux` job, cargo-deb | None | Installs, runs, removes |
| Linux | `pinpoint-desktop-linux-x86_64-<v>.AppImage` | `linux` job, appimagetool 1.9.0 (SHA-256 pinned) | None | Runs `--help` |
| Linux | `pinpoint-desktop-linux-x64-<v>.zip`, `pinpoint-server-linux-x64-<v>.zip` | `linux` job | None | Xvfb launch, server |
| Web | Served by `pinpoint-server` | The server zips above | Not applicable | `/`, `/privacy`, `/health` |

The web client is embedded in `pinpoint-server` (see `RELEASE_NOTES.md`), so
the server zips are the web build. A standalone static web bundle would still
need the API at the same origin, so it is not shipped separately.

## Local commands

```powershell
cargo build --release --locked --bin pinpoint-desktop
cargo install cargo-packager --version 0.11.8 --locked
cargo packager --release --formats wix      # target/packager/*.msi
```

Config lives in `Cargo.toml` (`[package.metadata.packager]`,
`[package.metadata.deb]`, `[package.metadata.bundle]`,
`[package.metadata.android]`) and in `packaging/`.

## Signing

Credentials and the order of a release are in [SIGNING.md](SIGNING.md).

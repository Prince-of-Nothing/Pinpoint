# Signing and store credentials

Every release artifact is built by `.github/workflows/release.yml` when a
`v*` tag is pushed on `main`. Until the credentials below exist, the builds
are **unsigned** (or ad-hoc signed on macOS), and the release notes say so.
Credentials go into GitHub Actions secrets on the repository. They never go
into the repository, a commit or a chat.

## Already set

| Secret | Used for |
| --- | --- |
| `PINPOINT_KEYSTORE_BASE64`, `PINPOINT_KEYSTORE_PASSWORD`, `PINPOINT_KEY_ALIAS`, `PINPOINT_KEY_PASSWORD` | Android: the APK and the AAB are signed with the release key on tags. |

Keep an offline backup of the Android keystore. Losing it means a new app
identity on Google Play, so the backup matters more than any other secret.

## What the owner must provide

Each item lists the secret names that CI will read once they are added. The
workflow does not read them yet; wiring them is a separate change.

### 1. Google Play (Android store upload)

- A Google Play Console developer account (one-time registration fee).
- Enrol the app in Play App Signing and upload the AAB that CI produces.
- Store listing material: privacy policy URL (`/privacy` on the server),
  data-safety form (the app shares location with friends), content rating,
  screenshots and a 512 px icon.
- Optional, to automate uploads: a service-account JSON with the Play
  Developer API enabled, as `PLAY_SERVICE_ACCOUNT_JSON`.

### 2. Apple (iOS and macOS)

- An Apple Developer Program membership (annual fee).
- Team ID (10 characters) as `APPLE_TEAM_ID`.
- The bundle ID `com.pinpoint.desktop` registered as an App ID.
- iOS: an Apple Distribution certificate exported as `.p12`
  (`IOS_DIST_P12_BASE64`, `IOS_DIST_P12_PASSWORD`) and an App Store
  provisioning profile for that App ID (`IOS_PROVISION_BASE64`).
- macOS: a Developer ID Application certificate exported as `.p12`
  (`MACOS_DEVID_P12_BASE64`, `MACOS_DEVID_P12_PASSWORD`). Without it, the DMG
  opens only after right-click, Open.
- Notarization and TestFlight use an App Store Connect API key: the key ID,
  issuer ID and the `.p8` file (`ASC_KEY_ID`, `ASC_ISSUER_ID`,
  `ASC_KEY_P8_BASE64`).

### 3. Windows (MSI and EXE signing)

- A code-signing certificate. Either a `.pfx` with its password
  (`WINDOWS_SIGN_PFX_BASE64`, `WINDOWS_SIGN_PFX_PASSWORD`), or an Azure
  Trusted Signing account. EV certificates now live on hardware tokens, so
  they cannot be used from CI; choose OV or Azure Trusted Signing.
- Without a signature, Windows SmartScreen warns before the MSI runs.

### 4. Linux

- Nothing is required. The `.deb` and the AppImage are unsigned, and the
  SHA-256 sums protect the download.
- Optional: a GPG key (`GPG_PRIVATE_KEY`, `GPG_PASSPHRASE`) to sign
  `SHA256SUMS.txt`.

### 5. Brand assets (not secrets, but needed for a premium release)

- A 1024 x 1024 master app icon as PNG. The repository has only a 192 px
  icon, so the macOS `.icns`, the Windows `.ico` and the iOS icon set come out
  soft. Send it, or commit it under `packaging/icons/`.

## Release order

1. Land the work on `Beta` and run the gate.
2. Open the one release PR from `Beta` to `main` and merge it with a merge
   commit (never a squash), as `COMMIT_RULES.md` requires.
3. Tag the release commit on `main` as `vX.Y.Z`. The tag must match
   `version` in `Cargo.toml`, or the workflow refuses to build.
4. Review the draft GitHub release, check `SHA256SUMS.txt`, and publish it.

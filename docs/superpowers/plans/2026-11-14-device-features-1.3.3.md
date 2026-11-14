# Pinpoint 1.3.3 Device Features Plan

**Goal:** Make the Android client useful on a real phone beyond the 1.3.2 crash-reporting release.

**Base:** `v1.3.2` (`f55fea7c`).

## Scope and feasibility

| # | Feature | Status | Notes |
|---|---|---|---|
| 5 | Real GPS, one-shot first | Implement | JNI `LocationManager.getLastKnownLocation` + runtime permission. Live updates later. |
| 6 | Deep links + share intents | Implement | `pinpoint://` and `geo:` intent filters; read launch intent via JNI; Android `ACTION_SEND` share. |
| 7 | Offline demo tile pack | Implement if assets fit | Bundle a small ODbL pack around Chișinău; extract to `tiles/{z}/{x}/{y}.png` on first run. |
| 8 | Android polish | Partial | Back button, bottom safe-area, IME focus, pinch zoom. Test on emulator. |
| 9 | Deploy backend | Config + docs only | Needs the owner's hosting account. Prepare Render/Fly/Docker config; cannot deploy without credentials. |
| 10 | Background location/foreground service | Blocked | Requires a Java/Kotlin `Service` and manifest component; the cargo-apk Rust-only APK has no Java sources/services. Needs a Gradle/Java bridge decision. |
| 11 | Biometric lock / FLAG_SECURE | Partial | FLAG_SECURE and an app PIN lock are feasible via JNI/Rust. BiometricPrompt needs AndroidX/Java and is blocked with item 10. |
| 12 | Kiosk/share QR codes | Implement | `qrcode` crate + egui painter for `pinpoint://kiosk?token=...` and `pinpoint://admin`. |

## Order

1. Deep links + launch-intent routing (6)
2. Kiosk QR codes (12)
3. FLAG_SECURE (11 partial)
4. GPS one-shot + permission (5)
5. Offline demo tile pack (7)
6. Android polish (8)
7. Backend deploy config/docs (9)
8. Document blocked items 10 and biometric 11

## Constraints

- Keep platform/JNI code in `src/platform.rs`.
- No secrets, no real user data, no unlicensed tiles.
- Every Android change must build with `cargo apk build --lib --target aarch64-linux-android`.
- Emulator verification for permissions/deep links where possible; real-phone check remains the final step.
- Version `1.3.3`; release workflow remains the signed APK path.

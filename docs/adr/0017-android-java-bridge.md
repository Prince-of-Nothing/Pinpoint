# 0017. Minimal Java Android bridge for share-in, biometrics and background location

Date: 2026-11-15

## Status

Accepted (owner requested completion of #24–#26)

## Context

The Rust-only `NativeActivity` packaging (ADR-0007, ADR-0013) cannot express
three Android features:

- **Repeat share-in:** Android delivers later `ACTION_SEND` intents to
  `onNewIntent`; `NativeActivity` does not call `setIntent`, so Rust keeps
  seeing the cold-start intent. ADR-0014 removed the share target because a
  silent no-op is worse than no target.
- **Biometric unlock:** `BiometricPrompt` is a platform Java API with an
  `AuthenticationCallback`; it cannot be instantiated from Rust without a Java
  subclass.
- **Background location:** Android requires a declared `Service` component and
  a foreground notification for continuous location updates; `cargo-apk`
  cannot declare a `<service>` element.

`cargo-apk` can only package native libraries, resources and one generated
manifest. It has no DEX or service support.

## Decision

Add a small, explicit Java layer under `android/app/src/main/java` and build the
APK with Gradle:

- `PinpointActivity extends NativeActivity`. It registers
  `libpinpoint_desktop` with the class loader, captures `ACTION_SEND` text
  (cold start and `onNewIntent`), and exposes `consumeShare()` to Rust.
  `nativeWakeUi()` wakes an idle egui loop.
- `PinpointActivity` starts the platform `BiometricPrompt` on API 28+ and
  stores `idle/pending/success/failed/unavailable`; Rust polls the state and
  unlocks the existing PIN lock on success. The PIN remains the fallback and
  biometrics are opt-in in `prefs.json`.
- `PinpointLocationService` is a non-exported foreground service with a
  persistent notification. It only starts from the explicit Profile action,
  requests GPS/network/passive updates at the chosen interval, and calls
  `nativePublishLocation`. Rust validates the fix and publishes through the
  active session, consent and encryption paths.
- Gradle packages the prebuilt Rust `.so` into `jniLibs`; the Rust library is
  still built by `cargo apk build` (NDK/linker handling) and Gradle owns the
  Java, manifest, DEX, resources and APK assembly.
- The release workflow builds one Gradle APK per ABI, zipaligns it and signs it
  with the existing release keystore.

Security rules:

- The service is `exported=false`; no other app can start it.
- No session cookie, CSRF token or password is ever passed to Java. The
  service calls Rust, which owns the backend client and consent checks.
- Background location is user-started, shows an ongoing notification and has
  an in-app stop control. It is never enabled silently.
- Share text is parsed by the existing allowlist (`geo:`, map links, kiosk
  token); arbitrary internal routes such as `pinpoint://admin` are rejected.
- Java contains no analytics, trackers or persistent secrets.

Alternatives rejected:

- **Keep the share target removed:** leaves #24 unresolved.
- **Poll `getIntent()` from Rust:** ADR-0014 proved `onNewIntent` does not
  update the activity intent without `setIntent`.
- **AndroidX BiometricPrompt:** unnecessary; the platform API covers API 28+.
- **Kotlin/AndroidX app rewrite:** violates the Rust-multiplatform rule and
  would replace the working eframe UI.

## Consequences

Easier:

- All three Android features share one auditable Java file set and one
  manifest.
- The Rust core, consent model and UI remain unchanged and portable.
- Gradle handles DEX, services, permissions and signing predictably.

Harder / risks:

- Android builds now need JDK 21 (AGP 8.7), Gradle and the Android SDK in
  addition to cargo-apk/NDK.
- The release APK grows by the small DEX and support code.
- The foreground service publishes only while the Rust process is alive; a
  force-stop ends it, as Android intends. The UI says so.
- Biometrics need an enrolled device; the emulator test covers the prompt
  invocation and PIN fallback, while a physical device test remains the final
  verification for face/fingerprint hardware.

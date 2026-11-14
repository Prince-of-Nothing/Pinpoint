# 0013. Use the live Android Activity for JNI and accept cold-start share/deep-link delivery

Date: 2026-11-14

## Status

Accepted. The `ACTION_SEND` share-in part is superseded by
[ADR-0014](./0014-remove-action-send-target.md).

## Context

`android-activity` publishes the JVM and the Android **Application** context
through `ndk_context::android_context()`. The Application context cannot start
activities, read the launch intent, show the runtime permission dialog or query
the window, so the Rust-only build must use the live `NativeActivity` object
instead. `AndroidApp::activity_as_ptr()` exposes that object, but its JNI global
reference is unowned and only guaranteed while the `AndroidApp` exists.

The APK has no Java/Kotlin sources (ADR-0007). `NativeActivity` keeps its
original intent when Android delivers a second share or deep link through
`onNewIntent`, and the Rust side cannot override that callback without a Java
subclass. Keeping `singleTop` avoids stacked copies of a GL/event-loop
application but makes repeat shares invisible while an instance is alive.

## Decision

We will keep the Rust-only APK and:

1. Register the `NativeActivity` in `android_main` by creating an **owned** JNI
   global reference and storing it in a process-wide mutex. A later
   `android_main` replaces it and releases the old reference. Every
   activity-affine JNI helper borrows that owned reference.
2. Use the Activity for FLAG_SECURE, launch-intent reads, runtime permission
   requests, share-sheet starts and the app-settings deep link.
3. Treat `ACTION_SEND`/`VIEW` delivery as **launch-time only** while the app is
   already alive. Document the limitation in the Android README, About →
   Troubleshooting and release notes.
4. Keep `launch_mode = "singleTop"`.

Alternatives rejected:

- **Application context only** — silently breaks FLAG_SECURE, permission
  dialogs, deep links and activity starts.
- **A Java `NativeActivity` subclass overriding `onNewIntent`** — requires a
  Gradle/Java build layer and a prebuilt DEX, violating the Rust-only
  packaging decision. It can be revisited with a concrete ADR.
- **`launchMode = "standard"`** — does not reliably create a second instance
  and risks multiple eframe event loops in one process.

## Consequences

- FLAG_SECURE, share-out, permission requests and cold-start deep links work on
  Android without a Java layer.
- Activity recreation is safe with respect to the owned reference; a stale
  unowned pointer is no longer stored.
- Repeat shares land in `onNewIntent` and are ignored until the process is
  restarted. This is a known product limitation, not a regression to hide.
- A future foreground service or biometric prompt still needs a Java/AndroidX
  layer and its own ADR.

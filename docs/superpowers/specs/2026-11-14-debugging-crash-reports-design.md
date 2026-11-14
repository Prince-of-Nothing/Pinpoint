# Pinpoint 1.3.2 — Debugging, Crash Reports and Release Startup Fix

**Status:** Approved for planning
**Date:** 2026-11-14
**Target release:** 1.3.2

## Intent

Make the Android release app actually reach its UI on a real phone, and give
the maintainer a no-server way to receive useful failure reports from that
phone.

The immediate trigger is two release-only startup paths:

1. The arm64 native library was 4 KB aligned and failed to load on Android 15+
   devices with 16 KB pages. That was fixed in 1.3.1.
2. The Android release default backend is `http://10.0.2.2:8080`. The URL
   policy rejects plain HTTP to that host unless `debug_assertions` is on, so
   `build_backend` fails in release and the fallback calls the same rejected
   URL behind `.expect(...)`. The app panics before the UI. This affects every
   release APK, including 1.3.1.

1.3.2 fixes the second path and adds the diagnostics needed to find the next
problem without a server.

## Goals

- A release Android build opens offline when no valid backend is configured,
  instead of panicking.
- Rust panics and `eframe::run_native` startup errors are written to an
  app-private crash report.
- A rolling text log is written to app-private storage on Android (and desktop).
- On the next launch, a non-blocking dialog shows the previous crash report and
  offers **Email report**, **Copy report**, **Discard** and **Close**.
- A manual **Report a problem** action exists in About and Profile, so
  non-crash weirdness can be reported through the same flow.
- **Email report** opens the user's email app with a `mailto:` link to the
  public maintainer address. On Android this needs a small `ACTION_VIEW` JNI
  helper because the `webbrowser` crate only opens `http(s)` there.
- The report shows exactly what will be sent; nothing is sent automatically.
- A debug card in About shows version, build identity, platform/ABI, page size,
  backend URL, log path, pending crash count, and buttons to copy/clear the log.
- The map shows an **Offline** badge when the app fell back to the offline
  backend, so a blank map reads as "offline" rather than "broken".

## Non-goals

- No automatic upload, no server, no Telegram bot token, no secrets in the APK.
- No Crashpad/Sentry/native signal handler in 1.3.2. A native crash before Rust
  (linker, SIGSEGV in a dependency) still needs `adb logcat`. The panic hook
  covers Rust panics, which is the controllable surface for this release.
- No guessing at the current "funky" UI symptoms. 1.3.2 ships the report flow;
  the first real report drives the next UI fix.
- No GPS/LocationManager bridge, no backend URL editor, no new plaintext HTTP
  exceptions. Those are separate changes.

## Constraints

- Security is thesis-mandated: server-side enforcement, no committed secrets,
  no real user data in tests.
- The report email is public (`plesudinu+waitlist@proton.me`, the commit author
  address). It is not a secret. If the maintainer wants another address, only
  `crash::REPORT_EMAIL` changes.
- Logs and reports live in the app sandbox. Android path:
  `/data/data/com.pinpoint.desktop/files/`.
- The report is user-reviewed and opt-in. No background transmission.
- Android URL opening for non-http schemes must use an explicit `Intent`; the
  existing `webbrowser` path stays for `http(s)`.
- Platform-specific code stays in `src/platform.rs`; no Android/JNI code in
  `src/ui.rs`.
- The release APK continues to be built by `.github/workflows/release.yml` with
  the 16 KB `build.rs` linker argument.

## Approaches Considered

### Report transport

1. **mailto in the user's email app — chosen.** No server, no token, the user
   sees and edits the report before sending, and the maintainer receives it in
   their normal inbox.
2. **Telegram deep link.** Also no token, but requires the maintainer's handle
   and Telegram on the phone; the report is still manually sent. Good fallback,
   not the primary choice.
3. **Embedded Telegram bot token.** Rejected: a public APK leaks the token and
   anyone can spam the bot.
4. **GitHub issue.** Rejected for crash reports: public by default and easy to
   leak location/account context.
5. **Sentry/Crashpad or a hosted crash service.** Rejected for 1.3.2: needs a
   server/DSN, adds a large native dependency, and conflicts with the
   no-server constraint.

### Crash capture

1. **Rust panic hook + rolling file log — chosen.** Small, portable, no native
   dependency, and covers the app code.
2. **Native signal handler.** Best-effort and not async-signal-safe for file
   I/O; deferred.
3. **Crashpad/Sentry native handler.** Robust but server/DSN-bound; deferred.

## Current State (verified)

- `src/main.rs` initializes only `tracing_subscriber::fmt::init()` (stderr).
- `src/mobile/android.rs` has no logger and no panic hook.
- `src/mobile/mod.rs` defaults to `BackendConfig::from_env()`, which resolves
  to `platform::default_backend_url()` = `http://10.0.2.2:8080` on Android.
- `src/api.rs::validate_backend_url_with_allowlist` accepts `10.0.2.2` only
  when `cfg!(all(target_os = "android", debug_assertions))`.
- `src/mobile/android.rs` falls back to
  `build_default_backend(crate::platform::default_backend_url()).expect(...)`,
  which panics in release.
- `webbrowser` 1.2.7's Android backend calls `target.get_http_url()` and
  rejects `mailto:` before building the Intent.
- `src/ui/about.rs` has no troubleshooting section; `src/ui/profile.rs` has a
  backend-submitted diagnostics form.
- `src/platform.rs` already provides `app_data_dir()`, `build_identity()` and
  the platform label.
- The release workflow and 16 KB `build.rs` fix are in place; 1.3.2 only needs
  the version bump and release steps.

## Design

### New modules

- `src/logging.rs`
  - `init()`: installs a `tracing-subscriber` registry with a file layer on
    native targets and a stderr layer on desktop. Android gets the file layer
    only. Wasm is a no-op.
  - `log_path() -> Option<PathBuf>`
  - `tail(max_lines: usize) -> String`
  - `clear()`
  - private `tail_lines(text: &str, max_lines: usize) -> String` for tests.
  - File: `<app_data_dir>/logs/pinpoint.log`, append, rotate to `.1` when it
    exceeds 1 MiB at startup. Keep the current file and one rotation.
- `src/crash.rs`
  - `REPORT_EMAIL`, `MAX_EMAIL_BODY`, `MAX_LOG_LINES`.
  - `CrashKind { Panic, Startup, Manual }`.
  - `StoredReport { path: PathBuf, text: String }`.
  - `build_report(kind, summary, details, log_tail) -> String`.
  - `write_report(kind, summary, details) -> Option<PathBuf>`.
  - `pending_report() -> Option<StoredReport>` (newest report).
  - `discard_report(path: &Path)`.
  - `redact(text: &str) -> String`.
  - `mailto_url(report: &str) -> String`.
  - `device_summary() -> String`.
  - `manual_report(summary: &str, note: &str) -> String`.
  - `install_panic_hook()` (idempotent via `Once`).
  - `capture_startup_error(error: &str)`.
  - Reports live in `<app_data_dir>/crash-reports/`; keep the newest five.
- `src/ui/crash_dialog.rs`
  - `CrashDialog { report, path, copied, error, source }`.
  - `from_pending(StoredReport)` and `manual(report)`.
  - `PinpointAppUI::draw_crash_dialog(ctx)` and `open_manual_report()`.
  - Dialog text: "Nothing is sent automatically. Review the report, then open
    your email app." Editable report body; buttons Email/Copy/Discard/Close.
  - Email path copies the full report to the clipboard first, then opens the
    `mailto:` URL (so a truncated email body still has the full text available).
- `src/mobile/mod.rs`
  - `build_mobile_backend(config) -> (DynBackend, bool)` where the bool is
    `offline_fallback`. On error, logs the error, builds the loopback fallback
    `http://localhost:8080` (accepted by the loopback policy), and returns
    `offline_fallback = true`.

### Report contents

```
Pinpoint crash report
Version: 1.3.2 (Pinpoint 1.3.2 on Android / aarch64)
Time: 2026-11-14T15:30:00Z
Kind: panic | startup | manual
Summary: <one line>

Details:
<panic payload, location, backtrace or startup error>

Recent log (last 100 lines):
<redacted tail>

---
This report was generated locally. Nothing was sent automatically.
```

`device_summary()` adds `OS`, `ARCH`, and on Unix the page size
(`libc::sysconf(_SC_PAGESIZE)`). On Android the page size is the key signal for
the 16 KB class of failures.

### Redaction

Apply before writing the report and before building the email body:

- email addresses → `<email>`
- `bearer|token|password|passwd|secret|session|csrf|cookie|authorization`
  followed by `:` or `=` and a value → `<key>=<redacted>`
- `lat|lon|latitude|longitude` followed by `:` or `=` and a number →
  `<key>=<redacted>`
- `q|query=` values → `<key>=<redacted>`
- Preserve newlines and the rest of the log.

The report is still shown to the user, who can edit it. Redaction is a safety
net, not a guarantee.

### Storage and lifecycle

- Panic: hook writes a `Panic` report and calls the previous hook.
- Startup error: `android_main` writes a `Startup` report if
  `eframe::run_native` fails. A rejected backend URL is an expected offline
  fallback: log it and show the Offline badge, not a crash report (otherwise
  every launch without a server would prompt).
- Next launch: `PinpointAppUI::update` checks `crash::pending_report()` once
  (`crash_checked`), opens the dialog, and never reopens it automatically in
  that session.
- Discard deletes the report; Close keeps it for a later manual view.
- Manual report: About/Profile builds a `Manual` report with the current log
  tail and an optional user note.
- Keep the newest five report files; older ones are deleted.

### Android `mailto:`

`src/platform.rs` gets:

```rust
pub fn open_url(url: &str) -> Result<(), String>
```

- Non-Android: `webbrowser::open(url).map_err(|e| e.to_string())`.
- Android:
  - `http://` / `https://`: existing `webbrowser::open`.
  - everything else (`mailto:`): a private `open_android_intent(url)` using the
    `android.content.Intent` `ACTION_VIEW` pattern from `webbrowser` 1.2.7:
    `Uri.parse`, `Intent(action, uri)`, `FLAG_ACTIVITY_NEW_TASK` when the
    `ndk-context` reference is an application context, then `startActivity`.
  - Add direct Android dependencies `jni = "0.22"` and
    `ndk-context = "0.1"` (already in the lockfile transitively).
  - Return a readable error when no email app can handle the intent; the
    dialog still has the copied report.

### UI

- `PinpointAppUI` gains `crash_dialog: Option<CrashDialog>` and
  `crash_checked: bool`.
- `update` calls `draw_crash_dialog` after `draw_share_sheet` and
  `draw_recovery_code`.
- About gets a **Troubleshooting** card:
  - Report a problem (manual dialog)
  - View last crash report (if pending)
  - Copy recent log
  - Clear log
  - Device summary (`build_identity`, OS/arch, page size, backend URL, log path)
- Profile's "Help & diagnostics" gets an **Email report** button next to the
  existing backend "Send report". The email path works without a server.
- Map shows an **Offline** badge when `app.is_offline_mode()` is true.
- About content is wrapped in a vertical `ScrollArea` so the new cards remain
  reachable on a phone.

### Release

- Version `1.3.2` in `Cargo.toml`/`Cargo.lock`.
- `CHANGELOG.md` and `RELEASE_NOTES.md` get a 1.3.2 section:
  - Fixed the release-only backend startup panic.
  - Added local crash reports, rolling logs, mailto reporting and the debug card.
- `android/README.md` documents how to test the report flow and that pre-Rust
  native crashes still need logcat.
- Tag `v1.3.2`; the existing release workflow builds signed arm64/armv7 APKs and
  drafts the GitHub release. Publish the draft with the 1.3.2 notes.
- Verify the CI-built arm64 APK still has `LOAD ... Align 0x4000`.

## Open Assumptions

- Report email: `plesudinu+waitlist@proton.me` (public commit author address).
- The first real phone report will define the next "funky UI" fix; 1.3.2 does
  not guess at those symptoms.
- The email body is intentionally truncated to a safe `mailto:` length; the
  full report is copied to the clipboard before the email app opens.

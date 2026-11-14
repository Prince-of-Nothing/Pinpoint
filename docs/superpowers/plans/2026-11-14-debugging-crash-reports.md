# Pinpoint 1.3.2 Debugging & Crash Reports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the release-only Android startup panic and add local crash reports, rolling logs, mailto reporting and an in-app debug card.

**Architecture:** A panic hook and `eframe::run_native` errors write redacted reports into app-private storage. A `tracing-subscriber` file layer keeps a rolling log. The UI shows the pending report on the next launch and offers a `mailto:` email flow, clipboard copy and manual reports. Android opens `mailto:` through a small JNI `ACTION_VIEW` Intent helper because `webbrowser` only accepts `http(s)` there. The release Android backend falls back to a loopback offline client instead of panicking.

**Tech Stack:** Rust 2021, eframe/egui 0.32, tracing-subscriber 0.3, jni 0.22, ndk-context 0.1, regex 1, libc 0.2, chrono 0.4, cargo-apk 0.10.

**Spec:** `docs/superpowers/specs/2026-11-14-debugging-crash-reports-design.md`

## Global Constraints

- Version `1.3.2`; artifacts remain signed arm64-v8a and armeabi-v7a APKs.
- No server, no bot token, no secrets in the APK or repo.
- The report email is public: `plesudinu+waitlist@proton.me`.
- Nothing is sent automatically; the user reviews and edits the report first.
- Logs/reports live in `platform::app_data_dir()/logs` and
  `platform::app_data_dir()/crash-reports`.
- Keep Android/JNI code in `src/platform.rs`; keep `src/ui.rs` portable.
- No new plaintext HTTP exceptions. The fallback is `http://localhost:8080`,
  which the loopback policy already accepts.
- wasm32 must still compile; crash/log code returns `None`/no-op there.
- Every commit uses the repo's `[TYPE] area: subject` convention and no
  `Co-authored-by` trailers.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo build --locked`, `cargo test --locked`, the wasm check and
  `cargo audit` before pushing.
- Android release builds use `cargo apk build --release --lib --target ...`
  with the local release keystore; never commit the keystore or password.

## Review Focus

- A release build with the default `http://10.0.2.2:8080` backend must open
  offline, not panic. A user on a phone with no server should see the UI and an
  Offline badge, not a crash loop.
- A crash report must never deadlock on the log writer or panic while handling a
  panic. The hook reads the log file directly and ignores write errors.
- `mailto:` bodies must percent-encode `&`, `?`, `#`, spaces and non-ASCII, and
  truncate on a UTF-8 char boundary.
- A phone with no email app must get a readable error and still keep the report
  on the clipboard.
- Logs can contain emails, tokens, coordinates and search queries; redaction
  must cover those before the report is written or emailed.
- The report prompt must not reappear on every launch for an expected offline
  backend; only real panics/startup errors create reports.
- Desktop and wasm builds must compile with the new modules.

---

### Task 1: Release-safe mobile backend fallback and Offline badge

**Files:**
- Modify: `src/mobile/mod.rs`
- Modify: `src/mobile/android.rs`
- Modify: `src/mobile/ios.rs`
- Modify: `src/ui.rs` (`draw_header_bar`)
- Test: inline `#[cfg(test)]` in `src/mobile/mod.rs`

**Interfaces:**
- Consumes: `crate::backend::{build_backend, build_default_backend, BackendConfig, DynBackend}`.
- Produces:
  - `pub const MOBILE_OFFLINE_FALLBACK: &str = "http://localhost:8080";`
  - `pub fn build_mobile_backend(config: &crate::backend::BackendConfig) -> (crate::backend::DynBackend, bool);`
    The bool is `true` when the configured backend was rejected and the loopback
    fallback is in use.

- [ ] **Step 1: Write the failing tests**

Add to `src/mobile/mod.rs`:

```rust
#[cfg(test)]
mod backend_tests {
    use super::*;
    use crate::backend::BackendConfig;

    #[test]
    fn invalid_backend_falls_back_offline_instead_of_panicking() {
        let cfg = BackendConfig::rest_v1("ftp://example.com");
        let (_backend, offline) = build_mobile_backend(&cfg);
        assert!(offline, "a rejected URL must select the offline fallback");
    }

    #[test]
    fn https_backend_stays_online() {
        let cfg = BackendConfig::rest_v1("https://example.com");
        let (_backend, offline) = build_mobile_backend(&cfg);
        assert!(!offline, "a valid HTTPS backend must not be offline");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run with the repo's Windows build environment loaded:

```powershell
cargo test --locked mobile::backend_tests -v
```

Expected: compile error because `build_mobile_backend` does not exist.

- [ ] **Step 3: Implement `build_mobile_backend`**

In `src/mobile/mod.rs`, add:

```rust
/// Loopback fallback used when a release build has no valid backend URL.
/// The loopback policy accepts this even in release; requests fail gracefully
/// and the app opens in offline mode.
pub const MOBILE_OFFLINE_FALLBACK: &str = "http://localhost:8080";

/// Resolve the mobile backend without ever panicking on a rejected URL.
/// Returns `(backend, offline_fallback)`.
pub fn build_mobile_backend(
    config: &crate::backend::BackendConfig,
) -> (crate::backend::DynBackend, bool) {
    match crate::backend::build_backend(config) {
        Ok(backend) => (backend, false),
        Err(error) => {
            tracing::error!(
                %error,
                url = %config.base_url,
                "mobile backend URL rejected; starting with the offline fallback"
            );
            let fallback = crate::backend::build_default_backend(MOBILE_OFFLINE_FALLBACK)
                .expect("the loopback fallback is always accepted by URL policy");
            (fallback, true)
        }
    }
}
```

- [ ] **Step 4: Use the fallback in Android and iOS**

In `src/mobile/android.rs`, replace the `build_backend` match with:

```rust
let (backend, offline_fallback) = crate::mobile::build_mobile_backend(&cfg.backend);
let mut app = crate::ui::PinpointAppUI::new(backend, None);
app.set_offline_mode(cfg.offline_first || offline_fallback);
```

Remove the now-unused `use crate::backend::build_backend;` import.

Apply the same change in `src/mobile/ios.rs`:

```rust
let (backend, offline_fallback) = crate::mobile::build_mobile_backend(&cfg.backend);
let mut app = crate::ui::PinpointAppUI::new(backend, None);
app.set_offline_mode(cfg.offline_first || offline_fallback);
```

- [ ] **Step 5: Add the Offline badge**

In `src/ui.rs`, inside `draw_header_bar`, after the app title label, add:

```rust
if self.app.is_offline_mode() {
    ui.add_space(6.0);
    ui.label(
        RichText::new("Offline")
            .size(10.0)
            .strong()
            .color(Color32::from_rgb(250, 204, 21)),
    );
}
```

Keep this inside the existing title row so it appears on compact and desktop.

- [ ] **Step 6: Run the focused tests and the gate**

```powershell
cargo test --locked mobile::backend_tests -v
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
```

Expected: both backend tests pass; fmt/clippy/build pass.

- [ ] **Step 7: Commit**

```powershell
git add src/mobile/mod.rs src/mobile/android.rs src/mobile/ios.rs src/ui.rs
git commit -m "[FIX] mobile: fall back offline instead of panicking on release"
```

---

### Task 2: Rolling file log

**Files:**
- Create: `src/logging.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`
- Modify: `src/mobile/android.rs`
- Test: inline `#[cfg(test)]` in `src/logging.rs`

**Interfaces:**
- Consumes: `crate::platform::app_data_dir()`.
- Produces:
  - `pub fn init();`
  - `pub fn log_path() -> Option<std::path::PathBuf>;`
  - `pub fn tail(max_lines: usize) -> String;`
  - `pub fn clear();`
  - private `fn tail_lines(text: &str, max_lines: usize) -> String;`
  - private `fn rotate_if_needed(path: &std::path::Path);`

- [ ] **Step 1: Write the failing tests**

Add to `src/logging.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::tail_lines;

    #[test]
    fn tail_returns_only_the_last_lines() {
        let text = "one\ntwo\nthree\nfour\nfive\n";
        assert_eq!(tail_lines(text, 2), "four\nfive");
    }

    #[test]
    fn tail_handles_crlf_and_empty_input() {
        assert_eq!(tail_lines("one\r\ntwo\r\nthree\r\n", 2), "two\nthree");
        assert_eq!(tail_lines("", 10), "");
    }

    #[test]
    fn tail_zero_lines_is_empty() {
        assert_eq!(tail_lines("one\ntwo\n", 0), "");
    }
}
```

Add a rotation test that uses a temporary directory:

```rust
#[test]
fn rotation_keeps_one_previous_file() {
    let dir = std::env::temp_dir().join(format!(
        "pinpoint-log-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pinpoint.log");
    std::fs::write(&path, vec![b'x'; 1_100_000]).unwrap();
    super::rotate_if_needed(&path);
    assert!(dir.join("pinpoint.log.1").exists());
    assert!(!path.exists() || std::fs::metadata(&path).unwrap().len() == 0);
    std::fs::remove_dir_all(&dir).ok();
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```powershell
cargo test --locked logging::tests -v
```

Expected: compile error because the module/functions do not exist.

- [ ] **Step 3: Implement the module**

Create `src/logging.rs` with:

```rust
//! Rolling application log.
//!
//! Native targets append to `<app_data_dir>/logs/pinpoint.log`; desktop also
//! keeps the stderr layer. Android gets the file layer so a crash report can
//! include the last useful lines. wasm is a no-op.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;

const MAX_LOG_BYTES: u64 = 1_048_576;

static INIT: Once = Once::new();

#[derive(Clone)]
struct SharedFile(Arc<Mutex<File>>);

impl Write for SharedFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

#[derive(Clone)]
struct FileMakeWriter(Arc<Mutex<File>>);

impl<'a> MakeWriter<'a> for FileMakeWriter {
    type Writer = SharedFile;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFile(self.0.clone())
    }
}

pub fn log_dir() -> Option<PathBuf> {
    crate::platform::app_data_dir().map(|base| base.join("logs"))
}

pub fn log_path() -> Option<PathBuf> {
    log_dir().map(|dir| dir.join("pinpoint.log"))
}

pub fn init() {
    INIT.call_once(|| {
        let Some(path) = log_path() else { return };
        let Some(dir) = path.parent() else { return };
        if fs::create_dir_all(dir).is_err() {
            return;
        }
        rotate_if_needed(&path);
        let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
            return;
        };
        let file = Arc::new(Mutex::new(file));
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(FileMakeWriter(file));
        let registry = tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::INFO)
            .with(file_layer);
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        let registry = registry.with(tracing_subscriber::fmt::layer());
        let _ = registry.try_init();
    });
}

pub fn tail(max_lines: usize) -> String {
    let Some(path) = log_path() else { return String::new() };
    let Ok(text) = fs::read_to_string(path) else { return String::new() };
    tail_lines(&text, max_lines)
}

pub fn clear() {
    if let Some(path) = log_path() {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("log.1"));
    }
}

fn rotate_if_needed(path: &Path) {
    let Ok(meta) = fs::metadata(path) else { return };
    if meta.len() <= MAX_LOG_BYTES {
        return;
    }
    let rotated = path.with_extension("log.1");
    let _ = fs::remove_file(&rotated);
    let _ = fs::rename(path, &rotated);
    let _ = fs::write(path, b"");
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > max_lines {
        lines = lines.split_off(lines.len() - max_lines);
    }
    lines.join("\n")
}
```

Keep `tail` simple: the 1 MiB rotation bounds the read.

- [ ] **Step 4: Register the module and initialize it**

In `src/lib.rs`, add `pub mod logging;` next to the other modules.

In `src/main.rs`, replace:

```rust
tracing_subscriber::fmt::init();
```

with:

```rust
pinpoint_desktop::logging::init();
```

In `src/mobile/android.rs`, add at the top of `android_main`:

```rust
crate::logging::init();
```

Do not add the panic hook yet; Task 4 does that.

- [ ] **Step 5: Run the tests and the gate**

```powershell
cargo test --locked logging::tests -v
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
```

Expected: logging tests pass; fmt/clippy/build pass. The desktop binary now
writes `~/.pinpoint/logs/pinpoint.log`.

- [ ] **Step 6: Commit**

```powershell
git add src/logging.rs src/lib.rs src/main.rs src/mobile/android.rs
git commit -m "[FEATURE] logging: add a rolling native app log"
```

---

### Task 3: Crash report core

**Files:**
- Create: `src/crash.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Test: inline `#[cfg(test)]` in `src/crash.rs`

**Interfaces:**
- Consumes: `crate::logging::tail`, `crate::platform::{app_data_dir, build_identity}`.
- Produces:
  - `pub const REPORT_EMAIL: &str = "plesudinu+waitlist@proton.me";`
  - `pub const MAX_EMAIL_BODY: usize = 1800;`
  - `pub enum CrashKind { Panic, Startup, Manual }`
  - `pub struct StoredReport { pub path: PathBuf, pub text: String }`
  - `pub fn report_dir() -> Option<PathBuf>`
  - `pub fn build_report(kind: CrashKind, summary: &str, details: &str, log_tail: &str) -> String`
  - `pub fn write_report(kind: CrashKind, summary: &str, details: &str) -> Option<PathBuf>`
  - `pub fn pending_report() -> Option<StoredReport>`
  - `pub fn discard_report(path: &Path)`
  - `pub fn redact(text: &str) -> String`
  - `pub fn mailto_url(report: &str) -> String`
  - `pub fn device_summary() -> String`
  - `pub fn manual_report(summary: &str, note: &str) -> String`
  - Private helpers used by tests: `write_report_at`, `pending_report_at`, `discard_report_at`.

- [ ] **Step 1: Add dependencies**

In `Cargo.toml`:

```toml
regex = "1"
```

Add `libc` for Unix page-size detection:

```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

`chrono` and `tracing` are already dependencies. Do not add a Telegram or
HTTP-upload dependency.

- [ ] **Step 2: Write the failing tests**

Add to `src/crash.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-crash-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn report_has_identity_and_sections() {
        let text = build_report(
            CrashKind::Panic,
            "index out of bounds",
            "at src/ui.rs:42\nbacktrace...",
            "INFO started\nERROR failed",
        );
        assert!(text.contains("Pinpoint crash report"));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains("Kind: panic"));
        assert!(text.contains("Summary: index out of bounds"));
        assert!(text.contains("at src/ui.rs:42"));
        assert!(text.contains("Recent log (last 100 lines):"));
        assert!(text.contains("INFO started"));
        assert!(text.contains("Nothing was sent automatically"));
    }

    #[test]
    fn redaction_removes_emails_tokens_coordinates_and_queries() {
        let text = "alice@example.com token=abc123 password: hunter2 \
                    lat=47.0105 lon=28.8638 q=secret%20place";
        let redacted = redact(text);
        assert!(!redacted.contains("alice@example.com"));
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("hunter2"));
        assert!(!redacted.contains("47.0105"));
        assert!(!redacted.contains("28.8638"));
        assert!(!redacted.contains("secret%20place"));
        assert!(redacted.contains("<email>"));
        assert!(redacted.contains("token=<redacted>"));
    }

    #[test]
    fn mailto_encodes_and_truncates_on_char_boundary() {
        let report = format!("{}é&? #\nend", "x".repeat(MAX_EMAIL_BODY + 20));
        let url = mailto_url(&report);
        assert!(url.starts_with("mailto:plesudinu+waitlist@proton.me?subject="));
        assert!(url.contains("%26"));
        assert!(url.contains("%3F"));
        assert!(url.contains("%23"));
        assert!(url.contains("%0A"));
        assert!(url.contains("%C3%A9") || url.contains("truncated"));
        assert!(url.len() < report.len() * 4);
    }

    #[test]
    fn report_store_round_trips_and_keeps_newest_five() {
        let dir = temp_dir();
        for i in 0..7 {
            write_report_at(&dir, CrashKind::Manual, &format!("report {i}"), "details");
        }
        let pending = pending_report_at(&dir).expect("a report");
        assert!(pending.text.contains("report 6"));
        let count = std::fs::read_dir(dir.join("crash-reports"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
            .count();
        assert_eq!(count, 5);
        discard_report_at(&pending.path);
        assert!(!pending.path.exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn device_summary_contains_arch_and_platform() {
        let summary = device_summary();
        assert!(summary.contains(std::env::consts::ARCH));
        assert!(summary.contains(crate::platform::Platform::current().label()));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

```powershell
cargo test --locked crash::tests -v
```

Expected: compile error because the module/functions do not exist.

- [ ] **Step 4: Implement `src/crash.rs`**

Key values and behavior:

```rust
//! Local crash reports. No network upload: the user reviews the report and
//! opens their email app with a mailto: link.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use chrono::{SecondsFormat, Utc};
use regex::Regex;

pub const REPORT_EMAIL: &str = "plesudinu+waitlist@proton.me";
pub const MAX_EMAIL_BODY: usize = 1800;
pub const MAX_LOG_LINES: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashKind { Panic, Startup, Manual }

impl CrashKind {
    fn label(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Startup => "startup",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredReport {
    pub path: PathBuf,
    pub text: String,
}
```

Implement `build_report` with the sections from the spec. Use
`Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)` for `Time`, and
`crate::platform::build_identity()` for `Version`.

Implement redaction with `LazyLock<Regex>`:

```rust
static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}").unwrap()
});
static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bearer|token|password|passwd|secret|session|csrf|cookie|authorization)\b\s*[:=]\s*[^\s&]+").unwrap()
});
static COORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(lat|lon|latitude|longitude)\b\s*[:=]\s*-?\d+(?:\.\d+)?").unwrap()
});
static QUERY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(q|query)\s*=\s*[^\s&]+").unwrap()
});
```

Replacements:

- email → `<email>`
- secret → `${1}=<redacted>` (use `captures` and format)
- coordinate → `${1}=<redacted>`
- query → `${1}=<redacted>`

Implement `mailto_url`:

```rust
pub fn mailto_url(report: &str) -> String {
    let subject = format!("Pinpoint {} crash report", env!("CARGO_PKG_VERSION"));
    let body = truncate_chars(report, MAX_EMAIL_BODY);
    format!(
        "mailto:{}?subject={}&body={}",
        REPORT_EMAIL,
        encode_component(&subject),
        encode_component(&body)
    )
}
```

`truncate_chars` must not split UTF-8:

```rust
fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        out.push_str("\n\n[truncated; full report copied to clipboard]");
    }
    out
}
```

`encode_component` percent-encodes every byte outside
`A-Z a-z 0-9 - . _ ~`, with space as `%20`:

```rust
fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
```

Implement storage:

- `report_dir() -> app_data_dir().map(|p| p.join("crash-reports"))`
- `write_report` calls `write_report_at(&dir, kind, summary, details)`.
- `write_report_at` creates `dir/crash-reports`, formats the file name as
  `crash-YYYYMMDD-HHMMSS-<nanos>-<kind>.txt`, where `<nanos>` is
  `SystemTime::now().duration_since(UNIX_EPOCH).as_nanos()` (zero-padded to a
  fixed width). Writes
  `build_report(kind, summary, details, &crate::logging::tail(MAX_LOG_LINES))`,
  prunes to the newest five `crash-*.txt`, returns the path. The nanos suffix
  prevents same-second collisions in the storage test and in real panic
  bursts.
- `pending_report_at` reads the newest `crash-*.txt` by lexicographic name
  (the timestamp + zero-padded nanos prefix makes that chronological).
- `discard_report_at` removes the file.
- `manual_report(summary, note)` calls `build_report(CrashKind::Manual,
  summary, note, &crate::logging::tail(MAX_LOG_LINES))`.

Implement `device_summary`:

```rust
pub fn device_summary() -> String {
    let page = page_size()
        .map(|p| format!(", page size {p}"))
        .unwrap_or_default();
    format!(
        "{} ({}, {}{})",
        crate::platform::build_identity(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        page
    )
}

#[cfg(unix)]
fn page_size() -> Option<usize> {
    let value = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (value > 0).then_some(value as usize)
}

#[cfg(not(unix))]
fn page_size() -> Option<usize> { None }
```

- [ ] **Step 5: Register the module**

In `src/lib.rs`, add `pub mod crash;`.

- [ ] **Step 6: Run the tests and the gate**

```powershell
cargo test --locked crash::tests -v
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
```

Expected: all crash tests pass; fmt/clippy/build pass.

- [ ] **Step 7: Commit**

```powershell
git add Cargo.toml Cargo.lock src/crash.rs src/lib.rs
git commit -m "[FEATURE] crash: local reports, redaction and mailto links"
```

---

### Task 4: Panic hook and startup capture

**Files:**
- Modify: `src/crash.rs`
- Modify: `src/main.rs`
- Modify: `src/mobile/android.rs`
- Test: inline `#[cfg(test)]` in `src/crash.rs`

**Interfaces:**
- Produces:
  - `pub fn install_panic_hook();`
  - `pub fn capture_startup_error(error: &str);`
  - `pub fn panic_details(payload: &str, location: Option<&str>, backtrace: &str) -> String;`

- [ ] **Step 1: Write the failing test**

Add to `src/crash.rs` tests:

```rust
#[test]
fn panic_details_formats_payload_location_and_backtrace() {
    let details = panic_details("boom", Some("src/ui.rs:42:9"), "0: backtrace");
    assert!(details.contains("boom"));
    assert!(details.contains("src/ui.rs:42:9"));
    assert!(details.contains("0: backtrace"));
    let without = panic_details("boom", None, "");
    assert!(without.contains("unknown location"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

```powershell
cargo test --locked crash::tests::panic_details_formats_payload_location_and_backtrace -v
```

Expected: compile error because `panic_details` does not exist.

- [ ] **Step 3: Implement the hook**

In `src/crash.rs`:

```rust
use std::sync::Once;

static PANIC_HOOK: Once = Once::new();

pub fn panic_details(payload: &str, location: Option<&str>, backtrace: &str) -> String {
    format!(
        "payload: {payload}\nlocation: {}\nbacktrace:\n{backtrace}",
        location.unwrap_or("unknown location")
    )
}

pub fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = if let Some(value) = info.payload().downcast_ref::<&str>() {
                (*value).to_string()
            } else if let Some(value) = info.payload().downcast_ref::<String>() {
                value.clone()
            } else {
                "non-string panic payload".to_string()
            };
            let location = info.location().map(|l| l.to_string());
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let details = panic_details(&payload, location.as_deref(), &backtrace);
            let _ = write_report(CrashKind::Panic, &payload, &details);
            previous(info);
        }));
    });
}

pub fn capture_startup_error(error: &str) {
    let _ = write_report(CrashKind::Startup, error, "");
}
```

- [ ] **Step 4: Install the hook in both entry points**

In `src/main.rs`, after `pinpoint_desktop::logging::init();`:

```rust
pinpoint_desktop::crash::install_panic_hook();
```

In `src/mobile/android.rs`, at the top of `android_main`, after
`crate::logging::init();`:

```rust
crate::crash::install_panic_hook();
```

Change the `run_native` result handling:

```rust
if let Err(error) = eframe::run_native("Pinpoint", options, Box::new(|_cc| Ok(Box::new(app)))) {
    crate::crash::capture_startup_error(&format!("Android launch failed: {error}"));
    eprintln!("Android launch failed: {error}");
}
```

Do not write a crash report for the expected offline backend fallback; Task 1
only logs it and shows the Offline badge.

- [ ] **Step 5: Run the tests and the gate**

```powershell
cargo test --locked crash::tests -v
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
```

Expected: all crash tests pass; fmt/clippy/build pass.

- [ ] **Step 6: Commit**

```powershell
git add src/crash.rs src/main.rs src/mobile/android.rs
git commit -m "[FEATURE] crash: capture panics and native startup errors"
```

---

### Task 5: Android `mailto:` Intent and platform URL opener

**Files:**
- Modify: `src/platform.rs`
- Modify: `Cargo.toml`
- Test: inline `#[cfg(test)]` in `src/platform.rs`

**Interfaces:**
- Produces:
  - `pub fn open_url(url: &str) -> Result<(), String>;`
  - `pub fn is_http_url(url: &str) -> bool;`
  - Android-only private `fn open_android_intent(url: &str) -> Result<(), String>;`

- [ ] **Step 1: Add Android dependencies**

In `Cargo.toml`, inside `[target.'cfg(target_os = "android")'.dependencies]`:

```toml
jni = "0.22"
ndk-context = "0.1"
```

These versions already appear in `Cargo.lock` transitively, so the lockfile
should not gain new crates.

- [ ] **Step 2: Write the failing test**

Add to `src/platform.rs` tests:

```rust
#[test]
fn only_http_urls_use_the_webbrowser_path() {
    assert!(is_http_url("https://example.com/x"));
    assert!(is_http_url("http://localhost:8080"));
    assert!(!is_http_url("mailto:test@example.com"));
    assert!(!is_http_url("geo:47.0,28.8"));
}
```

- [ ] **Step 3: Run the test to verify it fails**

```powershell
cargo test --locked platform::tests::only_http_urls_use_the_webbrowser_path -v
```

Expected: compile error because `is_http_url` does not exist.

- [ ] **Step 4: Implement the platform opener**

In `src/platform.rs`:

```rust
pub fn is_http_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

#[cfg(not(target_os = "android"))]
pub fn open_url(url: &str) -> Result<(), String> {
    webbrowser::open(url).map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
pub fn open_url(url: &str) -> Result<(), String> {
    if is_http_url(url) {
        webbrowser::open(url).map_err(|e| e.to_string())
    } else {
        open_android_intent(url)
    }
}
```

For the Android helper, copy the proven `webbrowser` 1.2.7 pattern. Add at the
bottom of `src/platform.rs`:

```rust
#[cfg(target_os = "android")]
mod android_intent {
    use jni::objects::JString;
    use jni::refs::Global;

    jni::bind_java_type! {
        AUri => "android.net.Uri",
        methods { static fn parse(uri: JString) -> AUri }
    }

    jni::bind_java_type! {
        AIntent => "android.content.Intent",
        type_map { AUri => "android.net.Uri" },
        constructors { fn new_with_url(action: JString, uri: AUri) },
        fields {
            #[allow(non_snake_case)]
            static ACTION_VIEW: JString,
            #[allow(non_snake_case)]
            static FLAG_ACTIVITY_NEW_TASK: jint
        },
        methods { fn add_flags(flags: jint) -> AIntent }
    }

    jni::bind_java_type! {
        AContext => "android.content.Context",
        type_map { AIntent => "android.content.Intent" },
        methods { fn start_activity(intent: AIntent) }
    }

    jni::bind_java_type! {
        AActivity => "android.app.Activity",
        type_map { AContext => "android.content.Context" },
        is_instance_of { activity: AContext }
    }

    pub(super) fn open(url: &str) -> Result<(), String> {
        let ctx = ndk_context::android_context();
        if ctx.vm().is_null() || ctx.context().is_null() {
            return Err("Android JVM/context is not available".to_string());
        }
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) }
            .map_err(|e| format!("JavaVM: {e}"))?;
        vm.attach_current_thread(|env| -> Result<(), String> {
            let context: jni::sys::jobject = ctx.context() as _;
            let context = unsafe { env.as_cast_raw::<Global<AContext>>(&context) }
                .map_err(|e| format!("context: {e}"))?;
            let context: &AContext = &context;
            let action = AIntent::ACTION_VIEW(env).map_err(|e| format!("ACTION_VIEW: {e}"))?;
            let url = JString::new(env, url).map_err(|e| format!("URL string: {e}"))?;
            let uri = AUri::parse(env, url).map_err(|e| format!("URI parse: {e}"))?;
            let intent = AIntent::new_with_url(env, &action, &uri)
                .map_err(|e| format!("Intent: {e}"))?;
            if env.as_cast::<AActivity>(&context).is_ok() {
                context.start_activity(env, &intent).map_err(|e| format!("startActivity: {e}"))?;
            } else {
                let flag = AIntent::FLAG_ACTIVITY_NEW_TASK(env)
                    .map_err(|e| format!("FLAG_ACTIVITY_NEW_TASK: {e}"))?;
                let intent = intent.add_flags(env, flag).map_err(|e| format!("addFlags: {e}"))?;
                context.start_activity(env, &intent).map_err(|e| format!("startActivity: {e}"))?;
            }
            Ok(())
        })
        .map_err(|e| format!("Android Intent failed: {e}"))
    }
}

#[cfg(target_os = "android")]
fn open_android_intent(url: &str) -> Result<(), String> {
    android_intent::open(url)
}
```

If the `bind_java_type!` macro version differs, use the equivalent low-level
`jni` calls from `webbrowser` 1.2.7 `src/android.rs`; the required behavior is
`ACTION_VIEW` + `Uri.parse` + `startActivity`, with
`FLAG_ACTIVITY_NEW_TASK` for an application context.

- [ ] **Step 5: Compile the Android target**

The JNI code is only compiled for Android, so a host `cargo check` does not
cover it. Build a debug APK:

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-25.0.5.8-hotspot"
cargo apk build --lib --target aarch64-linux-android
```

Expected: compile and package success.

- [ ] **Step 6: Run the host tests and gate**

```powershell
cargo test --locked platform::tests -v
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
```

Expected: tests pass; fmt/clippy/build pass.

- [ ] **Step 7: Manual Intent check**

Install the debug APK on the emulator, open About, press **Report a problem**,
then **Email report**. Expected: Gmail/email compose opens with the report
prefilled. If the emulator has no email app, the dialog shows a readable error
and the report remains on the clipboard. Do not automate a real email send.

- [ ] **Step 8: Commit**

```powershell
git add Cargo.toml Cargo.lock src/platform.rs
git commit -m "[FEATURE] android: open mailto crash reports with an Intent"
```

---

### Task 6: Crash dialog, About troubleshooting card and Profile email action

**Files:**
- Create: `src/ui/crash_dialog.rs`
- Modify: `src/ui.rs`
- Modify: `src/ui/about.rs`
- Modify: `src/ui/profile.rs`
- Test: inline `#[cfg(test)]` in `src/ui/crash_dialog.rs` for report selection

**Interfaces:**
- Consumes:
  - `crate::crash::{StoredReport, pending_report, mailto_url, manual_report, device_summary, discard_report}`.
  - `crate::logging::{tail, clear, log_path}`.
  - `crate::platform::open_url`.
- Produces:
  - `pub(crate) enum CrashSource { Startup, Manual }`
  - `pub(crate) struct CrashDialog { report: String, path: Option<PathBuf>, copied: bool, error: Option<String>, source: CrashSource }`
  - `impl CrashDialog { pub fn from_pending(stored: StoredReport) -> Self; pub fn manual(report: String) -> Self; }`
  - `impl PinpointAppUI { pub fn draw_crash_dialog(&mut self, ctx: &egui::Context); pub fn open_manual_report(&mut self); pub fn open_manual_report_with_note(&mut self, summary: &str, note: &str); }`

- [ ] **Step 1: Add the dialog state to `PinpointAppUI`**

In `src/ui.rs`:

```rust
pub(crate) mod crash_dialog;

use crash_dialog::CrashDialog;
```

Add fields:

```rust
    // Crash reporting
    crash_dialog: Option<CrashDialog>,
    crash_checked: bool,
```

Initialize:

```rust
            crash_dialog: None,
            crash_checked: false,
```

- [ ] **Step 2: Check for a pending report once per session**

At the start of `impl eframe::App for PinpointAppUI::update`, before
`sync_route_results`, add:

```rust
if !self.crash_checked {
    self.crash_checked = true;
    if let Some(stored) = crate::crash::pending_report() {
        self.crash_dialog = Some(CrashDialog::from_pending(stored));
    }
}
```

At the end of `update`, after `self.draw_share_sheet(ctx);` and
`self.draw_recovery_code(ctx);`, add:

```rust
self.draw_crash_dialog(ctx);
```

- [ ] **Step 3: Write the dialog**

Create `src/ui/crash_dialog.rs`. Use the About screen's dark palette. The
dialog must be non-blocking and scrollable:

```rust
use super::PinpointAppUI;
use crate::crash::{self, StoredReport};
use crate::platform;
use eframe::egui::{self, Align2, Color32, RichText};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrashSource {
    Startup,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrashDialogOutcome {
    Discard,
    Close,
}

pub(crate) struct CrashDialog {
    pub report: String,
    pub path: Option<PathBuf>,
    pub copied: bool,
    pub error: Option<String>,
    pub source: CrashSource,
}

impl CrashDialog {
    pub fn from_pending(stored: StoredReport) -> Self {
        Self {
            report: stored.text,
            path: Some(stored.path),
            copied: false,
            error: None,
            source: CrashSource::Startup,
        }
    }

    pub fn manual(report: String) -> Self {
        Self {
            report,
            path: None,
            copied: false,
            error: None,
            source: CrashSource::Manual,
        }
    }
}

impl PinpointAppUI {
    pub fn open_manual_report(&mut self) {
        let report = crash::manual_report("User-reported problem", "");
        self.crash_dialog = Some(CrashDialog::manual(report));
    }

    pub fn open_manual_report_with_note(&mut self, summary: &str, note: &str) {
        let report = crash::manual_report(summary, note);
        self.crash_dialog = Some(CrashDialog::manual(report));
    }

    pub fn draw_crash_dialog(&mut self, ctx: &egui::Context) {
        let mut outcome: Option<CrashDialogOutcome> = None;
        {
            let Some(dialog) = self.crash_dialog.as_mut() else { return };
            let compact = Self::is_compact(ctx);
            let title = match dialog.source {
                CrashSource::Startup => "Pinpoint closed unexpectedly",
                CrashSource::Manual => "Report a problem",
            };

            egui::Window::new(title)
                .collapsible(false)
                .resizable(true)
                .default_width(if compact { 330.0 } else { 560.0 })
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Nothing is sent automatically. Review or edit the report, then \
                         open your email app. The full report is copied to the clipboard \
                         before the email opens.",
                    );
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut dialog.report)
                                .desired_width(f32::INFINITY)
                                .desired_rows(12)
                                .font(egui::TextStyle::Monospace),
                        );
                    });
                    if dialog.copied {
                        ui.colored_label(Color32::from_rgb(52, 211, 153), "Report copied.");
                    }
                    if let Some(error) = &dialog.error {
                        ui.colored_label(Color32::from_rgb(248, 113, 113), error);
                    }
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Email report").clicked() {
                            ctx.copy_text(dialog.report.clone());
                            dialog.copied = true;
                            match platform::open_url(&crash::mailto_url(&dialog.report)) {
                                Ok(()) => dialog.error = None,
                                Err(error) => dialog.error = Some(format!(
                                    "Could not open the email app: {error}. The full report is on the clipboard."
                                )),
                            }
                        }
                        if ui.button("Copy report").clicked() {
                            ctx.copy_text(dialog.report.clone());
                            dialog.copied = true;
                        }
                        if dialog.path.is_some() && ui.button("Discard").clicked() {
                            outcome = Some(CrashDialogOutcome::Discard);
                        }
                        if ui.button("Close").clicked() {
                            outcome = Some(CrashDialogOutcome::Close);
                        }
                    });
                });
        }

        match outcome {
            Some(CrashDialogOutcome::Discard) => {
                if let Some(dialog) = self.crash_dialog.as_ref() {
                    if let Some(path) = dialog.path.as_ref() {
                        crash::discard_report(path);
                    }
                }
                self.crash_dialog = None;
            }
            Some(CrashDialogOutcome::Close) => self.crash_dialog = None,
            None => {}
        }
    }
}
```

Ensure `CrashDialog` fields are accessible from the parent module. Keep the
dialog's `source` copy semantics simple.

- [ ] **Step 4: Add the About troubleshooting card**

In `src/ui/about.rs`, wrap the existing `ui.vertical` content in a scroll area:

```rust
egui::ScrollArea::vertical().show(ui, |ui| {
    ui.vertical(|ui| {
        // existing content
    });
});
```

Add a card after the Project card:

```rust
ui.add_space(10.0);
card(ui, &mut |ui| {
    ui.label(RichText::new("Troubleshooting").size(17.0).strong());
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Crash reports and logs stay on this device until you choose to email them.",
        )
        .size(11.0)
        .color(muted),
    );
    ui.add_space(4.0);
    if ui.button("Report a problem").clicked() {
        self.open_manual_report();
    }
    if crate::crash::pending_report().is_some() && ui.button("View last crash report").clicked() {
        if let Some(stored) = crate::crash::pending_report() {
            self.crash_dialog = Some(crate::ui::crash_dialog::CrashDialog::from_pending(stored));
        }
    }
    if ui.button("Copy recent log").clicked() {
        ctx.copy_text(crate::logging::tail(200));
    }
    if ui.button("Clear log").clicked() {
        crate::logging::clear();
    }
    ui.add_space(4.0);
    ui.label(
        RichText::new(crate::crash::device_summary())
            .size(10.0)
            .color(muted),
    );
    if let Some(path) = crate::logging::log_path() {
        ui.label(
            RichText::new(format!("Log: {}", path.display()))
                .size(10.0)
                .color(muted),
        );
    }
});
```

The `card` closure captures `self`, so use the same pattern as the existing
cards (`|ui| { ... }` with mutable access). If the borrow checker objects,
extract the buttons into locals before the closure or use a helper method.

- [ ] **Step 5: Add the Profile email action**

In `src/ui/profile.rs`, inside the "Help & diagnostics" group, after the
existing "Send report" button, add:

```rust
let email_ready = !self.diagnostic_message.trim().is_empty();
if ui
    .add_enabled(email_ready, egui::Button::new("Email report"))
    .clicked()
{
    let summary = format!("User report: {}", self.diagnostic_category);
    let note = self.diagnostic_message.trim().to_string();
    self.open_manual_report_with_note(&summary, &note);
}
ui.label(
    RichText::new(
        "Send report uses the backend; Email report opens your mail app and works without a server.",
    )
    .size(10.0),
);
```

- [ ] **Step 6: Add tests for the pure dialog constructors**

In `src/ui/crash_dialog.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn from_pending_keeps_path_and_text() {
        let stored = StoredReport {
            path: PathBuf::from("/tmp/crash.txt"),
            text: "report".to_string(),
        };
        let dialog = CrashDialog::from_pending(stored);
        assert_eq!(dialog.report, "report");
        assert_eq!(dialog.path, Some(PathBuf::from("/tmp/crash.txt")));
        assert_eq!(dialog.source, CrashSource::Startup);
    }

    #[test]
    fn manual_dialog_has_no_path() {
        let dialog = CrashDialog::manual("manual".to_string());
        assert!(dialog.path.is_none());
        assert_eq!(dialog.source, CrashSource::Manual);
    }
}
```

- [ ] **Step 7: Run the gate**

```powershell
cargo test --locked crash_dialog::tests -v
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

Expected: tests pass; fmt/clippy/build/test pass. Fix any borrow issues in the
About closure by collecting the button actions first.

- [ ] **Step 8: Manual UI check on the emulator**

Build/install a debug APK, open About, verify:
- The Troubleshooting card is reachable by scrolling.
- **Report a problem** opens the dialog and the report text is readable.
- **Copy report** shows "Report copied."
- **Email report** opens the email app or shows the fallback error.
- **Close** dismisses the dialog; the app remains usable.
- The Offline badge appears when the app is in offline mode.

- [ ] **Step 9: Commit**

```powershell
git add src/ui/crash_dialog.rs src/ui.rs src/ui/about.rs src/ui/profile.rs
git commit -m "[FEATURE] ui: crash report dialog and troubleshooting card"
```

---

### Task 7: Release 1.3.2

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `CHANGELOG.md`
- Modify: `RELEASE_NOTES.md`
- Modify: `android/README.md`
- Modify: `README.md` (add the local report/log bullet to the Android client list)

- [ ] **Step 1: Bump the version**

Set `version = "1.3.2"` in `Cargo.toml`; run `cargo check` to update
`Cargo.lock`.

- [ ] **Step 2: Update the changelog**

Under `## [Unreleased]`, add:

```markdown
## [1.3.2] - 2026-11-14
### Added
- [FEATURE] Crash reporting: Rust panics and startup errors are written to an app-private report, shown on the next launch, and can be reviewed, edited, copied or emailed through the user's mail app. A manual Report a problem action and a Troubleshooting card expose the log, device summary and report store.
- [FEATURE] Rolling native log under the app data directory, with rotation and copy/clear actions.

### Fixed
- [FIX] Android release: a rejected default backend URL now opens the app in offline mode with a visible Offline badge instead of panicking before the UI.
```

- [ ] **Step 3: Update the release notes**

Add a `## 0. 1.3.2 patch` section at the top of `RELEASE_NOTES.md`, bump the
title/version/artifact names to 1.3.2, and note:
- The release startup panic fix (default `10.0.2.2` URL rejected in release).
- The crash/log/mailto flow and how to use About → Troubleshooting.
- Pre-Rust/native crashes still need `adb logcat`; the in-app report covers
  Rust panics and `run_native` errors.
- The 16 KB arm64 alignment remains from 1.3.1.

- [ ] **Step 4: Update Android and README docs**

In `android/README.md`, add a short "Crash reports" subsection:
- Where the log/report files live.
- About → Troubleshooting → Report a problem.
- Email report opens the mail app with `mailto:plesudinu+waitlist@proton.me`.
- Nothing is sent automatically.
- A native crash before Rust still needs `adb logcat -d`.

In `README.md`, extend the Android client bullet with:

```markdown
  Local crash reports and a rolling log are available under
  About → Troubleshooting; reports are emailed from the user's mail app.
```

- [ ] **Step 5: Run the full gate**

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
cargo check --locked --target wasm32-unknown-unknown --bin pinpoint-web --bin pinpoint-weba --bin pinpoint-webk
cargo audit
```

Expected: fmt/clippy/build/test pass; wasm exit 0; audit 0 vulnerabilities
(5 unmaintained-only warnings remain).

- [ ] **Step 6: Commit and push**

```powershell
git add Cargo.toml Cargo.lock CHANGELOG.md RELEASE_NOTES.md android/README.md README.md
git commit -m "[RELEASE] v1.3.2"
git push origin main
git tag -a v1.3.2 -m "Pinpoint 1.3.2"
git push origin v1.3.2
```

- [ ] **Step 7: Verify the release workflow**

The tag triggers `.github/workflows/release.yml`. Wait for the **Release** run
to complete, then download the arm64 asset and verify:

```powershell
# Inspect the CI-built APK's native library alignment
llvm-readelf -l lib/arm64-v8a/libpinpoint_desktop.so | Select-String "LOAD"
```

Expected: every `LOAD` line shows `Align 0x4000`. Verify the certificate is
`CN=Pinpoint, O=Pinpoint, C=MD` and the version is `1.3.2`.

- [ ] **Step 8: Publish the draft release**

Delete the `.idsig` assets (the workflow uploads them; direct installs do not
need them), set the release name to `Pinpoint 1.3.2`, replace the placeholder
body with the 1.3.2 section of `RELEASE_NOTES.md`, and publish. Confirm the
Atom feed shows `Pinpoint 1.3.2`.

- [ ] **Step 9: Real-phone acceptance**

Install `pinpoint-android-1.3.2.apk` on the phone. Expected:
- The app opens (no immediate close).
- The Offline badge is visible if no backend is configured.
- About → Troubleshooting → Report a problem shows the report.
- Email report opens the phone's mail app with the report addressed to the
  maintainer.
- Send the test report and confirm it arrives.

If the app still does not open, capture:

```powershell
adb logcat -d | findstr /i "pinpoint AndroidRuntime linker"
```

Add the logcat output to the next fix. Do not guess at the next UI fix without
that evidence.

- [ ] **Step 10: Final commit**

If verification changed docs, commit with `[DOCS] release: verify 1.3.2` and
push.

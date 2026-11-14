//! Local crash reports. No network upload: the user reviews the report and
//! opens their email app with a mailto: link.

use chrono::{SecondsFormat, Utc};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{LazyLock, Once};
use std::time::{SystemTime, UNIX_EPOCH};

pub const REPORT_EMAIL: &str = "plesudinu+waitlist@proton.me";
pub const MAX_EMAIL_BODY: usize = 1800;
pub const MAX_LOG_LINES: usize = 100;
const KEEP_REPORTS: usize = 5;

static SEQ: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashKind {
    Panic,
    Startup,
    Manual,
}

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

/// Directory holding crash reports, if the app has a writable data dir.
pub fn report_dir() -> Option<PathBuf> {
    crate::platform::app_data_dir().map(|base| base.join("crash-reports"))
}

pub fn build_report(kind: CrashKind, summary: &str, details: &str, log_tail: &str) -> String {
    let summary = redact(summary.trim());
    let details = redact(details.trim());
    let log_tail = redact(log_tail.trim_end());
    format!(
        "Pinpoint crash report\n\
         Version: {} ({})\n\
         Time: {}\n\
         Kind: {}\n\
         Summary: {}\n\
         \n\
         Details:\n{}\n\
         \n\
         Recent log (last {} lines):\n{}\n\
         \n\
         ---\n\
         This report was generated locally. Nothing was sent automatically.\n",
        env!("CARGO_PKG_VERSION"),
        device_summary(),
        Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        kind.label(),
        summary.trim(),
        if details.is_empty() {
            "(none)"
        } else {
            details.as_str()
        },
        MAX_LOG_LINES,
        if log_tail.is_empty() {
            "(no log lines)"
        } else {
            log_tail.as_str()
        },
    )
}

/// Write a report into `<base>/crash-reports/` and prune older reports.
pub fn write_report(kind: CrashKind, summary: &str, details: &str) -> Option<PathBuf> {
    let base = crate::platform::app_data_dir()?;
    write_report_at(&base, kind, summary, details)
}

fn write_report_at(base: &Path, kind: CrashKind, summary: &str, details: &str) -> Option<PathBuf> {
    let dir = base.join("crash-reports");
    std::fs::create_dir_all(&dir).ok()?;
    let log_tail = crate::logging::tail(MAX_LOG_LINES);
    let text = build_report(kind, summary, details, &log_tail);
    let now = Utc::now();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .subsec_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!(
        "crash-{}-{:09}-{:04}-{}.txt",
        now.format("%Y%m%d-%H%M%S"),
        nanos,
        seq,
        kind.label()
    ));
    std::fs::write(&path, text).ok()?;
    prune_reports(&dir, KEEP_REPORTS);
    Some(path)
}

/// Newest report, if one is waiting for the user.
pub fn pending_report() -> Option<StoredReport> {
    let base = crate::platform::app_data_dir()?;
    pending_report_at(&base)
}

fn pending_report_at(base: &Path) -> Option<StoredReport> {
    let dir = base.join("crash-reports");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("crash-") && name.ends_with(".txt")
        })
        .collect();
    files.sort_by_key(|path| {
        std::cmp::Reverse(
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    });
    for path in files {
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Some(StoredReport { path, text });
        }
    }
    None
}

pub fn discard_report(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Keep the report but stop auto-prompting for it on the next launch.
pub fn mark_seen(path: &Path) {
    let _ = std::fs::rename(path, path.with_extension("seen"));
}

fn prune_reports(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            name.starts_with("crash-") && name.ends_with(".txt")
        })
        .collect();
    files.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let overflow = files.len().saturating_sub(keep);
    for path in files.into_iter().take(overflow) {
        let _ = std::fs::remove_file(path);
    }
}

/// Redact common PII before writing or emailing a report.
pub fn redact(text: &str) -> String {
    static EMAIL_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}").unwrap());
    static AUTH_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(authorization|proxy-authorization)\b\s*[:=]\s*[^\r\n]+").unwrap()
    });
    static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r#"(?i)\b([a-z0-9_-]*(?:token|secret|password|passwd|api_key|apikey|session|cookie|csrf)[a-z0-9_-]*)\b\s*["']?\s*[:=]\s*["']?[^\s&"',}]+"#,
        )
        .unwrap()
    });
    static COORD_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?i)\b(lat|lon|latitude|longitude)\b\s*["']?\s*[:=]\s*["']?-?\d+(?:\.\d+)?"#)
            .unwrap()
    });
    static QUERY_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?i)\b(q|query)\s*["']?\s*=\s*["']?[^\s&"',}]+"#).unwrap());

    let text = EMAIL_RE.replace_all(text, "<email>");
    let text = AUTH_RE.replace_all(&text, "$1=<redacted>");
    let text = SECRET_RE.replace_all(&text, "$1=<redacted>");
    let text = COORD_RE.replace_all(&text, "$1=<redacted>");
    QUERY_RE.replace_all(&text, "$1=<redacted>").into_owned()
}

/// Build a mailto URL with a body short enough for Android/desktop mail apps.
pub fn mailto_url(report: &str) -> String {
    let subject = format!("Pinpoint {} crash report", env!("CARGO_PKG_VERSION"));
    let report = redact(report);
    let body = truncate_chars(&report, MAX_EMAIL_BODY);
    format!(
        "mailto:{}?subject={}&body={}",
        REPORT_EMAIL,
        encode_component(&subject),
        encode_component(&body)
    )
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    let mut out: String = text.chars().take(max_chars).collect();
    if total > max_chars {
        out.push_str("\n\n[truncated; full report copied to clipboard]");
    }
    out
}

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

/// Human-readable build/device summary used in reports and the About card.
pub fn device_summary() -> String {
    let page = page_size()
        .map(|value| format!(", page size {value}"))
        .unwrap_or_default();
    format!(
        "{} ({}, {}{})",
        crate::platform::build_identity(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        page
    )
}

pub fn manual_report(summary: &str, note: &str) -> String {
    build_report(
        CrashKind::Manual,
        summary,
        note,
        &crate::logging::tail(MAX_LOG_LINES),
    )
}

static PANIC_HOOK: Once = Once::new();

pub fn panic_details(payload: &str, location: Option<&str>, backtrace: &str) -> String {
    format!(
        "payload: {payload}\nlocation: {}\nbacktrace:\n{backtrace}",
        location.unwrap_or("unknown location")
    )
}

/// Install a process-wide panic hook that persists a report before the old
/// hook runs. Idempotent: a second call keeps the first hook.
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
            let location = info.location().map(|location| location.to_string());
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let details = panic_details(&payload, location.as_deref(), &backtrace);
            let _ = write_report(CrashKind::Panic, &payload, &details);
            previous(info);
        }));
    });
}

/// Persist a non-panic launch failure (for example `eframe::run_native`).
pub fn capture_startup_error(error: &str) {
    let _ = write_report(CrashKind::Startup, error, "");
}

#[cfg(unix)]
fn page_size() -> Option<usize> {
    let value = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (value > 0).then_some(value as usize)
}

#[cfg(not(unix))]
fn page_size() -> Option<usize> {
    None
}

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
    fn redaction_handles_headers_json_and_suffixed_tokens() {
        let text = "Authorization: Bearer bearer123\nX-CSRF-Token: csrf123 access_token=tok123\n{\"token\":\"json123\"}\n{\"lat\":47.01,\"lon\":28.86}";
        let redacted = redact(text);
        assert!(!redacted.contains("bearer123"));
        assert!(!redacted.contains("csrf123"));
        assert!(!redacted.contains("tok123"));
        assert!(!redacted.contains("json123"));
        assert!(!redacted.contains("47.01"));
        assert!(!redacted.contains("28.86"));
    }

    #[test]
    fn mailto_redacts_before_encoding() {
        let url = mailto_url("token=secret123");
        assert!(!url.contains("secret123"));
        assert!(url.contains("redacted"));
    }

    #[test]
    fn mailto_encodes_and_truncates_on_char_boundary() {
        // Keep the encoded characters inside the retained prefix, then add
        // enough text to force the truncation marker.
        let report = format!("é&? #\nend{}", "x".repeat(MAX_EMAIL_BODY + 20));
        let url = mailto_url(&report);
        assert!(url.starts_with("mailto:plesudinu+waitlist@proton.me?subject="));
        assert!(url.contains("%26"));
        assert!(url.contains("%3F"));
        assert!(url.contains("%23"));
        assert!(url.contains("%0A"));
        assert!(url.contains("%C3%A9"));
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
        discard_report(&pending.path);
        assert!(!pending.path.exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn marking_a_report_seen_removes_it_from_pending() {
        let dir = temp_dir();
        let path = write_report_at(&dir, CrashKind::Manual, "seen", "details").unwrap();
        assert!(pending_report_at(&dir).is_some());
        mark_seen(&path);
        assert!(pending_report_at(&dir).is_none());
        assert!(path.with_extension("seen").exists());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn device_summary_contains_arch_and_platform() {
        let summary = device_summary();
        assert!(summary.contains(std::env::consts::ARCH));
        assert!(summary.contains(crate::platform::Platform::current().label()));
    }

    #[test]
    fn panic_details_formats_payload_location_and_backtrace() {
        let details = panic_details("boom", Some("src/ui.rs:42:9"), "0: backtrace");
        assert!(details.contains("boom"));
        assert!(details.contains("src/ui.rs:42:9"));
        assert!(details.contains("0: backtrace"));
        let without = panic_details("boom", None, "");
        assert!(without.contains("unknown location"));
    }
}

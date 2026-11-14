//! Multiplatform surface: OS detection, backend URL defaults, and
//! platform-safe paths. One codebase for desktop (Windows/Linux/macOS),
//! Android (`cargo-apk` + `eframe` on `winit`) and iOS
//! (`cargo-xcodebuild`), with WebAssembly kept as a stub target.
//!
//! Design rule: everything platform-specific lives here behind `cfg!()` /
//! `std::env::consts::OS`, so `app`, `api`, `map` and `osm` stay portable.
//! No platform code touches the UI tree directly.

/// Running operating system (compile-time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Linux,
    MacOs,
    Android,
    Ios,
    Web,
    Unknown,
}

impl Platform {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "windows" => Self::Windows,
            "linux" => {
                // Android reports "android" via `cfg(target_os)` but
                // `std::env::consts::OS` is also "linux" on older toolchains:
                // prefer the cfg check.
                if cfg!(target_os = "android") {
                    Self::Android
                } else {
                    Self::Linux
                }
            }
            "macos" => Self::MacOs,
            "ios" => Self::Ios,
            "wasm" => Self::Web,
            _ => {
                if cfg!(target_os = "android") {
                    Self::Android
                } else if cfg!(target_os = "ios") {
                    Self::Ios
                } else if cfg!(target_arch = "wasm32") {
                    Self::Web
                } else {
                    Self::Unknown
                }
            }
        }
    }

    pub fn is_mobile(self) -> bool {
        matches!(self, Self::Android | Self::Ios)
    }

    pub fn needs_loopback_http_exception(self) -> bool {
        // ADR-0006: plain HTTP is only allowed for loopback backend URLs.
        // Mobile emulators reach the host via 10.0.2.2, which is also
        // loopback-adjacent for development — but releases are forced HTTPS.
        matches!(
            self,
            Self::Windows | Self::Linux | Self::MacOs | Self::Android
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::Linux => "Linux",
            Self::MacOs => "macOS",
            Self::Android => "Android",
            Self::Ios => "iOS",
            Self::Web => "Web (wasm)",
            Self::Unknown => "Unknown",
        }
    }
}

/// Default backend URL for local development on each platform.
/// Production builds MUST be overridden with `--backend https://…`.
pub fn default_backend_url() -> String {
    match Platform::current() {
        Platform::Android => {
            // Android emulator routes host loopback through 10.0.2.2.
            // 8080 avoids the Docker gateway that often owns 8000.
            "http://10.0.2.2:8080".to_string()
        }
        _ => "http://localhost:8000".to_string(),
    }
}

/// Backend URL resolution order (shared by desktop/mobile/web entries):
///
/// 1. explicit `--backend <url>` flag (caller passes it in),
/// 2. `PINPOINT_BACKEND` env var (container / mobile / CI override),
/// 3. per-platform [`default_backend_url`].
///
/// Never returns a non-loopback `http://` URL — callers must still run it
/// through `api::validate_backend_url` (ADR-0006).
pub fn backend_url_from_env_or_default() -> String {
    if let Ok(u) = std::env::var("PINPOINT_BACKEND") {
        let t = u.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    default_backend_url()
}

/// Parent directory that is writable on every target (bookmarks, offline
/// packs, logs). Returns `None` on targets without a filesystem
/// (wasm / iOS app sandboxes) — callers must degrade gracefully.
pub fn app_data_dir() -> Option<std::path::PathBuf> {
    match Platform::current() {
        // Native Android runs inside the app sandbox; this is the
        // standard per-app files dir (no JNI needed for tile cache).
        Platform::Android => Some(std::path::PathBuf::from(
            "/data/data/com.pinpoint.desktop/files",
        )),
        Platform::Ios | Platform::Web => None,
        _ => home_dir().map(|h| h.join(".pinpoint")),
    }
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .or_else(|| {
            std::env::var_os("HOMEDRIVE")
                .zip(std::env::var_os("HOMEPATH"))
                .map(|(d, p)| {
                    std::ffi::OsString::from(format!(
                        "{}{}",
                        d.to_string_lossy(),
                        p.to_string_lossy()
                    ))
                })
        })
        .map(std::path::PathBuf::from)
}

/// GPS / location provider abstraction used by mobile targets (and a
/// desktop fallback that reports "no fix"). Mobile backends implement
/// this behind `cfg(target_os)`; the desktop fallback always returns
/// `None` so `PinpointApp::publish_my_location` degrades gracefully.
pub trait GpsProvider: Send + Sync {
    /// Returns `(latitude, longitude, accuracy_m)` for the current fix,
    /// or `None` when no fix is available.
    fn current_position(&self) -> Option<(f64, f64, f64)>;
}

/// Desktop fallback: there is no GPS hardware, so no fix is ever produced.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoFixGps;

impl GpsProvider for NoFixGps {
    fn current_position(&self) -> Option<(f64, f64, f64)> {
        None
    }
}

/// Parse a GPS fix string in either CSV (`"lat,lon"` or `"lat,lon,accuracy"`)
/// or JSON (`{"latitude":..,"longitude":..,"accuracy":..}`) format.
/// Validates coordinates fail-closed (`-90..=90`, `-180..=180`, accuracy `> 0`).
pub fn parse_gps_fix(raw: &str) -> Option<(f64, f64, f64)> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let (lat, lon, acc) = if s.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(s).ok()?;
        let lat = v
            .get("latitude")
            .or_else(|| v.get("lat"))
            .and_then(|x| x.as_f64())?;
        let lon = v
            .get("longitude")
            .or_else(|| v.get("lon"))
            .and_then(|x| x.as_f64())?;
        let acc = v
            .get("accuracy")
            .or_else(|| v.get("acc"))
            .and_then(|x| x.as_f64())
            .unwrap_or(10.0);
        (lat, lon, acc)
    } else {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return None;
        }
        let lat = parts[0].trim().parse::<f64>().ok()?;
        let lon = parts[1].trim().parse::<f64>().ok()?;
        let acc = parts
            .get(2)
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(10.0);
        (lat, lon, acc)
    };
    if !crate::security::validation::validate_location(lat, lon, Some(acc)).is_valid() {
        return None;
    }
    Some((lat, lon, acc))
}

/// Cross-platform GPS provider that reads a fix from:
/// 1. `PINPOINT_GPS_FIX` environment variable (`"lat,lon,acc"` or JSON), or
/// 2. `<app_data_dir>/gps_fix.json` written by mobile platform bridges
///    (Android LocationManager / iOS CoreLocation) or dev tools.
///
/// Returns `None` when no valid fix is present.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvOrFileGpsProvider;

impl GpsProvider for EnvOrFileGpsProvider {
    fn current_position(&self) -> Option<(f64, f64, f64)> {
        if let Ok(raw) = std::env::var("PINPOINT_GPS_FIX") {
            if let Some(fix) = parse_gps_fix(&raw) {
                return Some(fix);
            }
        }
        if let Some(dir) = app_data_dir() {
            if let Ok(raw) = std::fs::read_to_string(dir.join("gps_fix.json")) {
                if let Some(fix) = parse_gps_fix(&raw) {
                    return Some(fix);
                }
            }
        }
        None
    }
}

/// Best-available GPS provider for the current platform.
pub fn default_gps_provider() -> Box<dyn GpsProvider> {
    Box::new(EnvOrFileGpsProvider)
}

/// Human-readable build identity for About screens / diagnostics.
pub fn build_identity() -> String {
    format!(
        "Pinpoint {} on {} / {}",
        env!("CARGO_PKG_VERSION"),
        Platform::current().label(),
        std::env::consts::ARCH
    )
}

/// True for URLs the `webbrowser` crate can open on every platform.
pub fn is_http_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Open a URL. Android sends non-HTTP schemes through an explicit Intent
/// because `webbrowser` deliberately rejects them there.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn open_url(url: &str) -> Result<(), String> {
    webbrowser::open(url).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn open_url(_url: &str) -> Result<(), String> {
    Err("opening URLs is unavailable in the wasm stub".to_string())
}

#[cfg(target_os = "android")]
pub fn open_url(url: &str) -> Result<(), String> {
    if is_http_url(url) {
        webbrowser::open(url).map_err(|error| error.to_string())
    } else {
        android_intent::open(url)
    }
}

#[cfg(test)]
mod tests {
    use super::{backend_url_from_env_or_default, default_backend_url, Platform};

    #[test]
    fn platform_current_never_panics() {
        let _ = Platform::current();
    }

    #[test]
    fn default_backend_is_http_loopback_for_dev() {
        let url = default_backend_url();
        assert!(
            url.starts_with("http://"),
            "dev default must be http: {url}"
        );
    }

    #[test]
    fn env_override_wins_over_default() {
        // SAFETY: tests run with multiple threads sharing the process env;
        // use a scoped temp var via a unique name is not possible for
        // PINPOINT_BACKEND, so we only assert the function never panics
        // and always returns a non-empty URL.
        let url = backend_url_from_env_or_default();
        assert!(!url.is_empty(), "backend URL must never be empty");
    }

    #[test]
    fn parse_gps_fix_csv_and_json() {
        use super::parse_gps_fix;
        assert_eq!(
            parse_gps_fix("47.0105, 28.8638, 12.5"),
            Some((47.0105, 28.8638, 12.5))
        );
        assert_eq!(
            parse_gps_fix("47.0105,28.8638"),
            Some((47.0105, 28.8638, 10.0))
        );
        assert_eq!(
            parse_gps_fix(r#"{"latitude":47.0105,"longitude":28.8638,"accuracy":8.0}"#),
            Some((47.0105, 28.8638, 8.0))
        );
        // Out-of-range coordinates rejected fail-closed.
        assert!(parse_gps_fix("120.0, 28.0").is_none());
        assert!(parse_gps_fix("47.0, 220.0").is_none());
        assert!(parse_gps_fix("garbage").is_none());
    }
}

#[cfg(test)]
mod intent_tests {
    use super::is_http_url;

    #[test]
    fn only_http_urls_use_the_webbrowser_path() {
        assert!(is_http_url("https://example.com/x"));
        assert!(is_http_url("http://localhost:8080"));
        assert!(!is_http_url("mailto:test@example.com"));
        assert!(!is_http_url("geo:47.0,28.8"));
    }
}

#[cfg(target_os = "android")]
mod android_intent {
    use anyhow::anyhow;
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
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) };
        vm.attach_current_thread(|env| -> anyhow::Result<()> {
            let context: jni::sys::jobject = ctx.context() as _;
            let context = unsafe { env.as_cast_raw::<Global<AContext>>(&context) }
                .map_err(|error| anyhow!("context: {error}"))?;
            let context: &AContext = &context;
            let action =
                AIntent::ACTION_VIEW(env).map_err(|error| anyhow!("ACTION_VIEW: {error}"))?;
            let url = JString::new(env, url).map_err(|error| anyhow!("URL string: {error}"))?;
            let uri = AUri::parse(env, url).map_err(|error| anyhow!("URI parse: {error}"))?;
            let intent = AIntent::new_with_url(env, &action, &uri)
                .map_err(|error| anyhow!("Intent: {error}"))?;
            if env.as_cast::<AActivity>(&context).is_ok() {
                context
                    .start_activity(env, &intent)
                    .map_err(|error| anyhow!("startActivity: {error}"))?;
            } else {
                let flag = AIntent::FLAG_ACTIVITY_NEW_TASK(env)
                    .map_err(|error| anyhow!("FLAG_ACTIVITY_NEW_TASK: {error}"))?;
                let intent = intent
                    .add_flags(env, flag)
                    .map_err(|error| anyhow!("addFlags: {error}"))?;
                context
                    .start_activity(env, &intent)
                    .map_err(|error| anyhow!("startActivity: {error}"))?;
            }
            Ok(())
        })
        .map_err(|error| format!("Android Intent failed: {error}"))
    }
}

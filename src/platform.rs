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

/// Hosted backend used by Android release builds.
pub const ANDROID_RELEASE_BACKEND: &str = "https://pinpoint-uj4y.onrender.com";

/// Default backend URL for each platform. Android release builds use the
/// hosted Render backend; debug builds keep the emulator loopback. Desktop
/// and iOS keep local development defaults.
pub fn default_backend_url() -> String {
    default_backend_url_for(Platform::current(), cfg!(debug_assertions))
}

fn default_backend_url_for(platform: Platform, debug_build: bool) -> String {
    match platform {
        Platform::Android if !debug_build => ANDROID_RELEASE_BACKEND.to_string(),
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
/// 3. the URL saved in Settings (`prefs.server_url`),
/// 4. per-platform [`default_backend_url`].
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
    let saved = crate::prefs::PrefsStore::default_location()
        .load()
        .server_url;
    let saved = saved.trim();
    if !saved.is_empty() {
        return saved.to_string();
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationPermission {
    Granted,
    Requested,
    Unsupported,
}

/// Result of the platform biometric prompt on Android. Other platforms report
/// [`BiometricStatus::Unavailable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiometricStatus {
    Idle,
    Pending,
    Success,
    Failed,
    Unavailable,
}

#[cfg(target_os = "android")]
#[derive(Debug, Default, Clone, Copy)]
pub struct AndroidLastKnownGps;

#[cfg(target_os = "android")]
impl GpsProvider for AndroidLastKnownGps {
    fn current_position(&self) -> Option<(f64, f64, f64)> {
        android_intent::last_known()
    }
}

pub fn default_gps_provider() -> Box<dyn GpsProvider> {
    #[cfg(target_os = "android")]
    {
        Box::new(AndroidLastKnownGps)
    }
    #[cfg(not(target_os = "android"))]
    {
        Box::new(EnvOrFileGpsProvider)
    }
}

#[cfg(target_os = "android")]
pub fn ensure_location_permission() -> LocationPermission {
    android_intent::ensure_permission()
}

#[cfg(not(target_os = "android"))]
pub fn ensure_location_permission() -> LocationPermission {
    LocationPermission::Granted
}

/// Read the foreground location grant without showing a system dialog.
#[cfg(target_os = "android")]
pub fn location_permission_granted() -> bool {
    android_intent::has_permission()
}

#[cfg(not(target_os = "android"))]
pub fn location_permission_granted() -> bool {
    true
}

/// Request "all the time" background location (and the notification
/// permission on Android 13+). Android 11+ may require the user to choose
/// "Allow all the time" in app settings after this request.
#[cfg(target_os = "android")]
pub fn ensure_background_location_permission() -> LocationPermission {
    android_intent::ensure_background_permission()
}

#[cfg(not(target_os = "android"))]
pub fn ensure_background_location_permission() -> LocationPermission {
    LocationPermission::Granted
}

/// Whether the device exposes an enrolled biometric prompt.
#[cfg(target_os = "android")]
pub fn android_biometric_supported() -> bool {
    android_intent::biometric_supported()
}

#[cfg(not(target_os = "android"))]
pub fn android_biometric_supported() -> bool {
    false
}

/// Start the system biometric prompt. The result is polled with
/// [`android_poll_biometric`]; the prompt does not block the UI thread.
#[cfg(target_os = "android")]
pub fn android_start_biometric() -> BiometricStatus {
    android_intent::start_biometric()
}

#[cfg(not(target_os = "android"))]
pub fn android_start_biometric() -> BiometricStatus {
    BiometricStatus::Unavailable
}

#[cfg(target_os = "android")]
pub fn android_poll_biometric() -> BiometricStatus {
    android_intent::poll_biometric()
}

#[cfg(not(target_os = "android"))]
pub fn android_poll_biometric() -> BiometricStatus {
    BiometricStatus::Unavailable
}

/// Actively ask Android for a fix when no provider has a recent last-known
/// location. Blocks up to `timeout` on the calling thread; call it off the UI
/// thread.
#[cfg(target_os = "android")]
pub fn request_single_fix(timeout: std::time::Duration) -> Option<(f64, f64, f64)> {
    android_intent::request_single_fix(timeout)
}

#[cfg(not(target_os = "android"))]
pub fn request_single_fix(_timeout: std::time::Duration) -> Option<(f64, f64, f64)> {
    None
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn valid_fix(lat: f64, lon: f64, accuracy: f64) -> Option<(f64, f64, f64)> {
    if lat.is_finite()
        && lon.is_finite()
        && accuracy.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
        && accuracy > 0.0
        && accuracy <= 5_000.0
    {
        Some((lat, lon, accuracy))
    } else {
        None
    }
}

/// A last-known fix is only useful if it is recent enough to describe where
/// the user is now.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn fix_is_recent(fix_time_ms: i64, now_ms: i64) -> bool {
    fix_time_ms > 0 && now_ms >= fix_time_ms && now_ms - fix_time_ms <= 10 * 60 * 1000
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

/// Only these schemes may leave the app through the OS URL handler.
/// Everything else (file:, content:, intent:, javascript:, http/https, ...)
/// is rejected so a future caller cannot launch arbitrary components.
pub fn is_allowed_external_url(url: &str) -> bool {
    // No whitespace/control characters: they are not valid in these URIs and
    // can be used to confuse Intent/URL parsing.
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let Some(colon) = url.find(':') else {
        return false;
    };
    let scheme = &url[..colon];
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return false;
    }
    if url[colon + 1..].trim().is_empty() {
        return false;
    }
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "mailto" | "tel" | "geo"
    )
}

fn reject_unsupported_url(url: &str) -> Result<(), String> {
    if is_allowed_external_url(url) {
        Ok(())
    } else {
        Err(format!(
            "unsupported URL scheme; allowed schemes are mailto:, tel: and geo: ({url})"
        ))
    }
}

/// Open an allowlisted external URL. Android sends it through an explicit
/// Intent because `webbrowser` deliberately rejects non-HTTP schemes there.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn open_url(url: &str) -> Result<(), String> {
    reject_unsupported_url(url)?;
    webbrowser::open(url).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn open_url(_url: &str) -> Result<(), String> {
    Err("opening URLs is unavailable in the wasm stub".to_string())
}

#[cfg(target_os = "android")]
pub fn open_url(url: &str) -> Result<(), String> {
    reject_unsupported_url(url)?;
    android_intent::open(url)
}

/// URI from the Activity launch intent (`pinpoint://...` or `geo:...`).
#[cfg(target_os = "android")]
pub fn android_launch_uri() -> Option<String> {
    android_intent::launch_uri()
}

#[cfg(not(target_os = "android"))]
pub fn android_launch_uri() -> Option<String> {
    None
}

/// Open the system app-settings page (permission recovery on Android).
#[cfg(target_os = "android")]
pub fn open_app_settings() -> Result<(), String> {
    android_intent::open_settings()
}

#[cfg(not(target_os = "android"))]
pub fn open_app_settings() -> Result<(), String> {
    Err("app settings are Android-only".to_string())
}

/// Text shared into the app with Android's `ACTION_SEND` chooser.
#[cfg(target_os = "android")]
pub fn android_shared_text() -> Option<String> {
    android_intent::shared_text()
}

/// Consume the newest share captured by `onNewIntent` (or the cold-start
/// intent) exactly once. This is the repeat-share path; [`android_shared_text`]
/// is the legacy read-only helper.
#[cfg(target_os = "android")]
pub fn android_take_shared_text() -> Option<String> {
    android_intent::take_shared_text()
}

#[cfg(not(target_os = "android"))]
pub fn android_take_shared_text() -> Option<String> {
    None
}

#[cfg(not(target_os = "android"))]
pub fn android_shared_text() -> Option<String> {
    None
}

/// Open the system share sheet for plain text.
#[cfg(target_os = "android")]
pub fn share_text(text: &str) -> Result<(), String> {
    android_intent::share_text(text)
}

#[cfg(not(target_os = "android"))]
pub fn share_text(_text: &str) -> Result<(), String> {
    Err("sharing is handled by the UI on desktop".to_string())
}

/// Register the live `NativeActivity` for JNI calls. `ndk_context` exposes the
/// Application context, which cannot start activities, request permissions or
/// read the launch intent.
#[cfg(target_os = "android")]
pub fn set_android_activity(ptr: *mut std::ffi::c_void) {
    android_intent::set_activity(ptr);
}

#[cfg(not(target_os = "android"))]
pub fn set_android_activity(_ptr: *mut std::ffi::c_void) {}

/// Give the Android foreground service a handle to the active app so a fix is
/// published through the same session, consent and validation paths as the UI.
#[cfg(target_os = "android")]
pub fn register_android_app(app: std::sync::Weak<crate::app::PinpointApp>) {
    android_service::register(app);
}

#[cfg(target_os = "android")]
pub fn android_start_background_location(interval_secs: u64) -> Result<(), String> {
    android_intent::start_location_service(interval_secs)
}

#[cfg(not(target_os = "android"))]
pub fn android_start_background_location(_interval_secs: u64) -> Result<(), String> {
    Err("background location is Android-only".to_string())
}

#[cfg(target_os = "android")]
pub fn android_stop_background_location() -> Result<(), String> {
    android_intent::stop_location_service()
}

#[cfg(not(target_os = "android"))]
pub fn android_stop_background_location() -> Result<(), String> {
    Err("background location is Android-only".to_string())
}

#[cfg(target_os = "android")]
pub fn android_background_location_running() -> bool {
    android_intent::location_service_running()
}

#[cfg(not(target_os = "android"))]
pub fn android_background_location_running() -> bool {
    false
}

/// Whether this platform can host the shared MapLibre page in an app WebView.
/// Today that is Android only; desktop and iOS keep the egui/native map.
pub fn map_webview_supported() -> bool {
    cfg!(target_os = "android")
}

/// Whether the full-screen MapLibre WebView is currently open.
#[cfg(target_os = "android")]
pub fn map_webview_visible() -> bool {
    android_webview_map::visible()
}

#[cfg(not(target_os = "android"))]
pub fn map_webview_visible() -> bool {
    false
}

/// Start the loopback server (once per process) and show the MapLibre page in
/// a full-screen WebView above the egui surface.
#[cfg(target_os = "android")]
pub fn show_map_webview() -> Result<(), String> {
    android_webview_map::show()
}

#[cfg(not(target_os = "android"))]
pub fn show_map_webview() -> Result<(), String> {
    Err("the MapLibre WebView is Android-only".to_string())
}

#[cfg(target_os = "android")]
pub fn hide_map_webview() -> Result<(), String> {
    android_webview_map::hide()
}

#[cfg(not(target_os = "android"))]
pub fn hide_map_webview() -> Result<(), String> {
    Ok(())
}

/// Consume the newest map click reported by the WebView bridge.
pub fn take_map_webview_click() -> Option<(f64, f64)> {
    #[cfg(target_os = "android")]
    {
        android_webview_map::take_click()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Ask Android's share sheet for a point selected in the WebView map.
#[cfg(target_os = "android")]
pub fn share_map_webview_point(lat: f64, lon: f64, label: &str) -> Result<(), String> {
    android_webview_map::share(lat, lon, label)
}

#[cfg(not(target_os = "android"))]
pub fn share_map_webview_point(_lat: f64, _lon: f64, _label: &str) -> Result<(), String> {
    Err("the MapLibre WebView is Android-only".to_string())
}

/// JNI callback from `PinpointActivity`'s `PinpointBridge` JavaScript
/// interface. Runs on the WebView bridge thread; it only stores validated
/// coordinates and wakes the UI.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_pinpoint_desktop_PinpointActivity_nativeMapClick<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _this: jni::objects::JObject<'caller>,
    latitude: f64,
    longitude: f64,
) {
    let _ = unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            android_webview_map::record_click(latitude, longitude);
            crate::ui::android_wake();
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// JNI callback from the Java close control / back gesture, so the egui map
/// screen updates its show/hide button even when the WebView closes itself.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_pinpoint_desktop_PinpointActivity_nativeMapWebViewClosed<
    'caller,
>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _this: jni::objects::JObject<'caller>,
) {
    let _ = unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            android_webview_map::mark_closed();
            crate::ui::android_wake();
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// JNI callback from `PinpointActivity.onNewIntent`: wake an idle egui loop so
/// the new share is consumed immediately, not on the next user interaction.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_pinpoint_desktop_PinpointActivity_nativeWakeUi<'caller>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _this: jni::objects::JObject<'caller>,
) {
    let _ = unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            crate::ui::android_wake();
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// JNI callback from the foreground location service. The service only passes
/// the latest fix; Rust validates and publishes it through the active session.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_pinpoint_desktop_PinpointLocationService_nativePublishLocation<
    'caller,
>(
    mut unowned_env: jni::EnvUnowned<'caller>,
    _class: jni::objects::JClass<'caller>,
    latitude: f64,
    longitude: f64,
    accuracy: f64,
) {
    let _ = unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            android_service::publish(latitude, longitude, accuracy);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Prevent Android screenshots/screen recording of location data.
#[cfg(target_os = "android")]
pub fn enable_secure_window() -> Result<(), String> {
    android_intent::enable_secure_window()
}

#[cfg(not(target_os = "android"))]
pub fn enable_secure_window() -> Result<(), String> {
    Ok(())
}

/// Put text on the real system clipboard. Android's egui fallback is
/// in-process only, so the report copy path must use JNI.
#[cfg(target_os = "android")]
pub fn copy_text(text: &str) -> Result<(), String> {
    android_intent::copy_text(text)
}

#[cfg(not(target_os = "android"))]
pub fn copy_text(_text: &str) -> Result<(), String> {
    Err("use the UI clipboard on desktop".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        backend_url_from_env_or_default, default_backend_url, default_backend_url_for, Platform,
    };

    #[test]
    fn platform_current_never_panics() {
        let _ = Platform::current();
    }

    #[test]
    fn android_release_defaults_to_render() {
        assert_eq!(
            default_backend_url_for(Platform::Android, false),
            "https://pinpoint-uj4y.onrender.com"
        );
        assert_eq!(
            default_backend_url_for(Platform::Android, true),
            "http://10.0.2.2:8080"
        );
        assert_eq!(
            default_backend_url_for(Platform::Windows, false),
            "http://localhost:8000"
        );
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

    #[test]
    fn gps_fix_validation_rejects_bad_values() {
        assert!(super::valid_fix(47.0, 28.0, 5.0).is_some());
        assert!(super::valid_fix(120.0, 28.0, 5.0).is_none());
        assert!(super::valid_fix(47.0, 220.0, 5.0).is_none());
        assert!(super::valid_fix(47.0, 28.0, 0.0).is_none());
        assert!(super::valid_fix(47.0, 28.0, 50_000.0).is_none());
    }

    #[test]
    fn gps_fix_age_must_be_recent() {
        let now = 1_700_000_000_000;
        assert!(super::fix_is_recent(now - 60_000, now));
        assert!(!super::fix_is_recent(now - 11 * 60_000, now));
        assert!(!super::fix_is_recent(0, now));
        assert!(!super::fix_is_recent(now + 60_000, now));
    }
}

#[cfg(test)]
mod intent_tests {
    use super::is_allowed_external_url;
    use super::is_http_url;

    #[test]
    fn allowed_external_urls_are_mailto_tel_geo_only() {
        assert!(is_allowed_external_url("mailto:test@example.com"));
        assert!(is_allowed_external_url("MAILTO:test@example.com"));
        assert!(is_allowed_external_url("tel:+37360000000"));
        assert!(is_allowed_external_url("geo:47.0105,28.8638"));
        assert!(is_allowed_external_url("geo:47.0105,28.8638?q=Home"));
        for rejected in [
            "http://example.com",
            "https://example.com",
            "file:///etc/passwd",
            "content://contacts/1",
            "intent://scan/#Intent;scheme=zxing;end",
            "javascript:alert(1)",
            "pinpoint://admin",
            "mailto:",
            "mailto",
            " mailto:test@example.com",
            "mailto:\u{0}test@example.com",
        ] {
            assert!(
                !is_allowed_external_url(rejected),
                "should reject {rejected:?}"
            );
        }
    }

    #[test]
    fn only_http_urls_use_the_webbrowser_path() {
        assert!(is_http_url("https://example.com/x"));
        assert!(is_http_url("http://localhost:8080"));
        assert!(!is_http_url("mailto:test@example.com"));
        assert!(!is_http_url("geo:47.0,28.8"));
    }

    #[test]
    fn android_only_bridges_fail_closed_elsewhere() {
        #[cfg(not(target_os = "android"))]
        {
            use super::{
                android_background_location_running, android_biometric_supported,
                android_poll_biometric, android_start_background_location, android_start_biometric,
                android_stop_background_location, android_take_shared_text, BiometricStatus,
            };
            assert!(!android_biometric_supported());
            assert_eq!(android_start_biometric(), BiometricStatus::Unavailable);
            assert_eq!(android_poll_biometric(), BiometricStatus::Unavailable);
            assert!(android_start_background_location(60).is_err());
            assert!(android_stop_background_location().is_err());
            assert!(!android_background_location_running());
            assert!(android_take_shared_text().is_none());
        }
    }
}

#[cfg(target_os = "android")]
mod android_webview_map {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    static VISIBLE: AtomicBool = AtomicBool::new(false);
    static LAST_CLICK: Mutex<Option<(f64, f64)>> = Mutex::new(None);

    pub(super) fn visible() -> bool {
        VISIBLE.load(Ordering::Acquire)
    }

    pub(super) fn show() -> Result<(), String> {
        let server = crate::mobile::map_server::shared()?;
        super::android_intent::show_map_webview(&server.map_url())?;
        VISIBLE.store(true, Ordering::Release);
        Ok(())
    }

    pub(super) fn hide() -> Result<(), String> {
        super::android_intent::hide_map_webview()?;
        VISIBLE.store(false, Ordering::Release);
        Ok(())
    }

    pub(super) fn mark_closed() {
        VISIBLE.store(false, Ordering::Release);
    }

    pub(super) fn record_click(latitude: f64, longitude: f64) {
        if crate::security::validation::validate_location(latitude, longitude, None).is_valid() {
            if let Ok(mut last) = LAST_CLICK.lock() {
                *last = Some((latitude, longitude));
            }
        } else {
            tracing::warn!("map bridge: rejected an out-of-range click");
        }
    }

    pub(super) fn take_click() -> Option<(f64, f64)> {
        LAST_CLICK.lock().ok().and_then(|mut last| last.take())
    }

    pub(super) fn share(lat: f64, lon: f64, label: &str) -> Result<(), String> {
        if !crate::security::validation::validate_location(lat, lon, None).is_valid() {
            return Err("invalid map coordinates".to_string());
        }
        super::android_intent::share_geo(lat, lon, label)
    }
}

#[cfg(target_os = "android")]
mod android_service {
    use crate::app::PinpointApp;
    use std::sync::{Mutex, OnceLock, Weak};

    /// App handle registered by `PinpointAppUI::new`; the foreground service
    /// publishes through the same backend, session and consent checks as the UI.
    static APP: Mutex<Option<Weak<PinpointApp>>> = Mutex::new(None);
    /// A dedicated single-worker runtime; service callbacks can arrive from any
    /// Java thread and must not block it.
    static RUNTIME: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();

    pub(super) fn register(app: Weak<PinpointApp>) {
        if let Ok(mut guard) = APP.lock() {
            *guard = Some(app);
        }
    }

    pub(super) fn publish(latitude: f64, longitude: f64, accuracy: f64) {
        let app = APP
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().and_then(Weak::upgrade));
        let Some(app) = app else {
            tracing::warn!("android service: no app handle for a background fix");
            return;
        };
        let runtime = RUNTIME.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .ok()
        });
        let Some(runtime) = runtime.as_ref() else {
            tracing::warn!("android service: could not start the publish runtime");
            return;
        };
        runtime.spawn(async move {
            if let Err(error) = app.publish_location(latitude, longitude, accuracy).await {
                tracing::warn!(%error, "android service: background publish failed");
            }
        });
    }
}

#[cfg(target_os = "android")]
mod android_intent {
    use anyhow::anyhow;
    use jni::objects::{JObject, JString};
    use jni::refs::Global;
    use jni::JValue;
    use std::sync::Mutex;

    /// Owned JNI global reference to the current `NativeActivity`. Storing an
    /// owned ref keeps it valid across Activity recreation; a new android_main
    /// replaces it and dropping the old one releases the JNI reference.
    static ACTIVITY: Mutex<Option<Global<JObject<'static>>>> = Mutex::new(None);

    pub(super) fn set_activity(ptr: *mut std::ffi::c_void) {
        let ctx = ndk_context::android_context();
        if ctx.vm().is_null() {
            tracing::warn!("android: cannot register activity without a JVM");
            return;
        }
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) };
        let result = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let raw = ptr as jni::sys::jobject;
            let local = unsafe { JObject::from_raw(&*env, raw) };
            let global = env.new_global_ref(&local)?;
            *ACTIVITY.lock().unwrap() = Some(global);
            Ok(())
        });
        if let Err(error) = result {
            tracing::warn!(%error, "android: failed to register the activity");
        }
    }

    jni::bind_java_type! {
        AUri => "android.net.Uri",
        methods { static fn parse(uri: JString) -> AUri }
    }

    jni::bind_java_type! {
        AIntent => "android.content.Intent",
        type_map { AUri => "android.net.Uri" },
        constructors {
            fn new_with_url(action: JString, uri: AUri),
            fn new_with_action(action: JString)
        },
        fields {
            #[allow(non_snake_case)]
            static ACTION_VIEW: JString,
            #[allow(non_snake_case)]
            static FLAG_ACTIVITY_NEW_TASK: jint
        },
        methods {
            fn add_flags(flags: jint) -> AIntent,
            fn get_data_string() -> JString,
            fn get_action() -> JString,
            fn get_string_extra(name: JString) -> JString,
            fn set_type(mime: JString) -> AIntent
        }
    }

    jni::bind_java_type! {
        AContext => "android.content.Context",
        type_map { AIntent => "android.content.Intent" },
        methods {
            fn start_activity(intent: AIntent),
            fn get_system_service(name: JString) -> JObject,
            fn check_self_permission(permission: JString) -> jint
        }
    }

    jni::bind_java_type! {
        AWindow => "android.view.Window",
        methods { fn set_flags(flags: jint, mask: jint) }
    }

    jni::bind_java_type! {
        AActivity => "android.app.Activity",
        type_map {
            AContext => "android.content.Context",
            AIntent => "android.content.Intent",
            AWindow => "android.view.Window"
        },
        is_instance_of { activity: AContext },
        methods {
            fn get_intent() -> AIntent,
            fn get_window() -> AWindow
        }
    }

    pub(super) fn open(url: &str) -> Result<(), String> {
        with_activity(|env, activity| {
            let action =
                AIntent::ACTION_VIEW(env).map_err(|error| anyhow!("ACTION_VIEW: {error}"))?;
            let url = JString::new(env, url).map_err(|error| anyhow!("URL string: {error}"))?;
            let uri = AUri::parse(env, url).map_err(|error| anyhow!("URI parse: {error}"))?;
            let intent = AIntent::new_with_url(env, &action, &uri)
                .map_err(|error| anyhow!("Intent: {error}"))?;
            let context: &AContext = activity.as_ref();
            context
                .start_activity(env, &intent)
                .map_err(|error| anyhow!("startActivity: {error}"))?;
            Ok(())
        })
        .map_err(|error| format!("Android Intent failed: {error}"))
    }

    pub(super) fn launch_uri() -> Option<String> {
        with_activity(|env, activity| -> anyhow::Result<Option<String>> {
            let intent = activity
                .get_intent(env)
                .map_err(|error| anyhow!("getIntent: {error}"))?;
            let Ok(data) = intent.get_data_string(env) else {
                return Ok(None);
            };
            if data.is_null() {
                return Ok(None);
            }
            Ok(Some(data.to_string()))
        })
        .ok()
        .flatten()
    }

    pub(super) fn open_settings() -> Result<(), String> {
        with_activity(|env, activity| {
            let action = JString::new(env, "android.settings.APPLICATION_DETAILS_SETTINGS")?;
            let uri_text = JString::new(env, "package:com.pinpoint.desktop")?;
            let uri = AUri::parse(env, uri_text).map_err(|error| anyhow!("URI: {error}"))?;
            let intent = AIntent::new_with_url(env, &action, &uri)
                .map_err(|error| anyhow!("Intent: {error}"))?;
            let context: &AContext = activity.as_ref();
            context
                .start_activity(env, &intent)
                .map_err(|error| anyhow!("startActivity: {error}"))?;
            Ok(())
        })
        .map_err(|error| format!("Android settings failed: {error}"))
    }

    pub(super) fn shared_text() -> Option<String> {
        let result = with_activity(|env, activity| -> anyhow::Result<Option<String>> {
            let intent = activity
                .get_intent(env)
                .map_err(|error| anyhow!("getIntent: {error}"))?;
            if intent.is_null() {
                return Ok(None);
            }
            let action = intent
                .get_action(env)
                .map_err(|error| anyhow!("getAction: {error}"))?;
            if action.is_null() || action.to_string() != "android.intent.action.SEND" {
                return Ok(None);
            }
            let key = JString::new(env, "android.intent.extra.TEXT")?;
            let text = intent
                .get_string_extra(env, &key)
                .map_err(|error| anyhow!("getStringExtra: {error}"))?;
            if text.is_null() {
                return Ok(None);
            }
            Ok(Some(text.to_string()))
        });
        match result {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("android share: {error}");
                None
            }
        }
    }

    /// Consume the newest `ACTION_SEND` text captured by `PinpointActivity`.
    /// The Java side clears it so a repeated frame cannot re-open the same
    /// location.
    pub(super) fn take_shared_text() -> Option<String> {
        let result = with_activity(|env, activity| -> anyhow::Result<Option<String>> {
            let activity_obj: &JObject = activity.as_ref();
            let runtime = jni::signature::RuntimeMethodSignature::from_str("()Ljava/lang/String;")?;
            let signature = jni::signature::MethodSignature::from(&runtime);
            let text = env
                .call_method(
                    activity_obj,
                    jni::strings::JNIString::from("consumeShare"),
                    signature,
                    &[],
                )
                .map_err(|error| anyhow!("consumeShare: {error}"))?
                .l()?;
            if text.is_null() {
                return Ok(None);
            }
            let text = env
                .as_cast::<JString>(&text)
                .map_err(|error| anyhow!("consumeShare string: {error}"))?;
            Ok(Some(text.to_string()))
        });
        match result {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("android share: {error}");
                None
            }
        }
    }

    pub(super) fn biometric_supported() -> bool {
        with_activity(|env, activity| -> anyhow::Result<bool> {
            let activity_obj: &JObject = activity.as_ref();
            Ok(call_int(env, activity_obj, "isBiometricSupported")? != 0)
        })
        .unwrap_or(false)
    }

    pub(super) fn start_biometric() -> super::BiometricStatus {
        let started = with_activity(|env, activity| -> anyhow::Result<i32> {
            let activity_obj: &JObject = activity.as_ref();
            call_int(env, activity_obj, "startBiometric")
        })
        .unwrap_or(0);
        if started == 1 {
            super::BiometricStatus::Pending
        } else {
            super::BiometricStatus::Unavailable
        }
    }

    pub(super) fn poll_biometric() -> super::BiometricStatus {
        match with_activity(|env, activity| -> anyhow::Result<i32> {
            let activity_obj: &JObject = activity.as_ref();
            call_int(env, activity_obj, "pollBiometric")
        }) {
            Ok(0) => super::BiometricStatus::Idle,
            Ok(1) => super::BiometricStatus::Pending,
            Ok(2) => super::BiometricStatus::Success,
            Ok(3) => super::BiometricStatus::Failed,
            _ => super::BiometricStatus::Unavailable,
        }
    }

    pub(super) fn start_location_service(interval_secs: u64) -> Result<(), String> {
        let interval = interval_secs.clamp(15, 3600) as i32;
        with_activity(|env, activity| -> anyhow::Result<()> {
            let activity_obj: &JObject = activity.as_ref();
            let started = call_int_arg(env, activity_obj, "startLocationService", interval)?;
            if started == 1 {
                Ok(())
            } else {
                anyhow::bail!("startLocationService returned 0")
            }
        })
        .map_err(|error| format!("Android background location failed: {error}"))
    }

    pub(super) fn stop_location_service() -> Result<(), String> {
        with_activity(|env, activity| -> anyhow::Result<()> {
            let activity_obj: &JObject = activity.as_ref();
            let stopped = call_int(env, activity_obj, "stopLocationService")?;
            if stopped == 1 {
                Ok(())
            } else {
                anyhow::bail!("stopLocationService returned 0")
            }
        })
        .map_err(|error| format!("Android background location stop failed: {error}"))
    }

    pub(super) fn location_service_running() -> bool {
        with_activity(|env, activity| -> anyhow::Result<bool> {
            let activity_obj: &JObject = activity.as_ref();
            Ok(call_int(env, activity_obj, "isLocationServiceRunning")? != 0)
        })
        .unwrap_or(false)
    }

    pub(super) fn show_map_webview(url: &str) -> Result<(), String> {
        with_activity(|env, activity| -> anyhow::Result<()> {
            let activity_obj: &JObject = activity.as_ref();
            let url = JString::new(env, url)?;
            let runtime =
                jni::signature::RuntimeMethodSignature::from_str("(Ljava/lang/String;)V")?;
            let signature = jni::signature::MethodSignature::from(&runtime);
            env.call_method(
                activity_obj,
                jni::strings::JNIString::from("showMapWebView"),
                signature,
                &[JValue::Object(url.as_ref())],
            )
            .map_err(|error| anyhow!("showMapWebView: {error}"))?;
            Ok(())
        })
        .map_err(|error| format!("Android map WebView failed: {error}"))
    }

    pub(super) fn hide_map_webview() -> Result<(), String> {
        with_activity(|env, activity| -> anyhow::Result<()> {
            let activity_obj: &JObject = activity.as_ref();
            let runtime = jni::signature::RuntimeMethodSignature::from_str("()V")?;
            let signature = jni::signature::MethodSignature::from(&runtime);
            env.call_method(
                activity_obj,
                jni::strings::JNIString::from("hideMapWebView"),
                signature,
                &[],
            )
            .map_err(|error| anyhow!("hideMapWebView: {error}"))?;
            Ok(())
        })
        .map_err(|error| format!("Android map WebView close failed: {error}"))
    }

    pub(super) fn share_geo(lat: f64, lon: f64, label: &str) -> Result<(), String> {
        with_activity(|env, activity| -> anyhow::Result<()> {
            let activity_obj: &JObject = activity.as_ref();
            let label = JString::new(env, label)?;
            let runtime =
                jni::signature::RuntimeMethodSignature::from_str("(DDLjava/lang/String;)I")?;
            let signature = jni::signature::MethodSignature::from(&runtime);
            let shared = env
                .call_method(
                    activity_obj,
                    jni::strings::JNIString::from("shareGeo"),
                    signature,
                    &[
                        JValue::Double(lat),
                        JValue::Double(lon),
                        JValue::Object(label.as_ref()),
                    ],
                )
                .map_err(|error| anyhow!("shareGeo: {error}"))?
                .i()?;
            if shared != 1 {
                anyhow::bail!("shareGeo returned {shared}");
            }
            Ok(())
        })
        .map_err(|error| format!("Android map share failed: {error}"))
    }

    pub(super) fn share_text(text: &str) -> Result<(), String> {
        with_activity(|env, activity| {
            let action = JString::new(env, "android.intent.action.SEND")?;
            let intent = AIntent::new_with_action(env, &action)
                .map_err(|error| anyhow!("Intent: {error}"))?;
            let mime = JString::new(env, "text/plain")?;
            let intent = intent
                .set_type(env, &mime)
                .map_err(|error| anyhow!("setType: {error}"))?;
            let key = JString::new(env, "android.intent.extra.TEXT")?;
            let value = JString::new(env, text)?;
            let name = jni::strings::JNIString::from("putExtra");
            let runtime = jni::signature::RuntimeMethodSignature::from_str(
                "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
            )
            .map_err(|error| anyhow!("putExtra signature: {error}"))?;
            let signature = jni::signature::MethodSignature::from(&runtime);
            let intent_obj: &JObject = intent.as_ref();
            env.call_method(
                intent_obj,
                name,
                signature,
                &[JValue::Object(key.as_ref()), JValue::Object(value.as_ref())],
            )
            .map_err(|error| anyhow!("putExtra: {error}"))?;
            let context: &AContext = activity.as_ref();
            context
                .start_activity(env, &intent)
                .map_err(|error| anyhow!("startActivity: {error}"))?;
            Ok(())
        })
        .map_err(|error| format!("Android share failed: {error}"))
    }

    pub(super) fn enable_secure_window() -> Result<(), String> {
        with_activity(|env, activity| {
            let window = activity
                .get_window(env)
                .map_err(|error| anyhow!("getWindow: {error}"))?;
            // WindowManager.LayoutParams.FLAG_SECURE
            window
                .set_flags(env, 0x0000_2000, 0x0000_2000)
                .map_err(|error| anyhow!("setFlags: {error}"))?;
            Ok(())
        })
        .map_err(|error| format!("FLAG_SECURE failed: {error}"))
    }

    pub(super) fn copy_text(text: &str) -> Result<(), String> {
        with_activity(|env, activity| {
            let context: &AContext = activity.as_ref();
            let service_name = JString::new(env, "clipboard")?;
            let service = context
                .get_system_service(env, service_name)
                .map_err(|error| anyhow!("getSystemService: {error}"))?;
            if service.is_null() {
                anyhow::bail!("clipboard service unavailable");
            }
            let text = JString::new(env, text)?;
            let name = jni::strings::JNIString::from("setText");
            let signature =
                jni::signature::RuntimeMethodSignature::from_str("(Ljava/lang/CharSequence;)V")
                    .map_err(|error| anyhow!("setText signature: {error}"))?;
            let signature = jni::signature::MethodSignature::from(&signature);
            env.call_method(&service, name, signature, &[JValue::Object(text.as_ref())])
                .map_err(|error| anyhow!("setText: {error}"))?;
            Ok(())
        })
        .map_err(|error| format!("Clipboard failed: {error}"))
    }

    fn with_activity<T>(
        f: impl FnOnce(&mut jni::Env, &AActivity) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let ctx = ndk_context::android_context();
        if ctx.vm().is_null() {
            anyhow::bail!("Android JVM is not available");
        }
        let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) };
        vm.attach_current_thread(|env| -> anyhow::Result<T> {
            let guard = ACTIVITY
                .lock()
                .map_err(|_| anyhow!("activity lock poisoned"))?;
            let Some(global) = guard.as_ref() else {
                anyhow::bail!("Android Activity is not registered");
            };
            let raw = global.as_raw();
            let activity = unsafe { env.as_cast_raw::<Global<AActivity>>(&raw) }
                .map_err(|error| anyhow!("activity: {error}"))?;
            let activity: &AActivity = &activity;
            f(env, activity)
        })
    }

    fn has_location_permission(env: &mut jni::Env, activity: &AActivity) -> anyhow::Result<bool> {
        let context: &AContext = activity.as_ref();
        for name in [
            "android.permission.ACCESS_FINE_LOCATION",
            "android.permission.ACCESS_COARSE_LOCATION",
        ] {
            let permission = JString::new(env, name)?;
            if context.check_self_permission(env, permission)? == 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn has_permission() -> bool {
        with_activity(|env, activity| -> anyhow::Result<bool> {
            has_location_permission(env, activity)
        })
        .unwrap_or(false)
    }

    pub(super) fn ensure_permission() -> super::LocationPermission {
        let granted = with_activity(|env, activity| -> anyhow::Result<bool> {
            has_location_permission(env, activity)
        })
        .unwrap_or(false);
        if granted {
            return super::LocationPermission::Granted;
        }
        let requested = with_activity(|env, activity| -> anyhow::Result<()> {
            let class_name = jni::strings::JNIString::from("java/lang/String");
            let class = env.find_class(class_name)?;
            // Android 12+ ignores a fine-location request that does not also
            // ask for coarse; request both in one dialog.
            let array = env.new_object_array(2, &class, JObject::null())?;
            for (index, permission) in [
                "android.permission.ACCESS_FINE_LOCATION",
                "android.permission.ACCESS_COARSE_LOCATION",
            ]
            .iter()
            .enumerate()
            {
                let permission = JString::new(env, *permission)?;
                array.set_element(env, index, &permission)?;
            }
            let name = jni::strings::JNIString::from("requestPermissions");
            let runtime =
                jni::signature::RuntimeMethodSignature::from_str("([Ljava/lang/String;I)V")?;
            let signature = jni::signature::MethodSignature::from(&runtime);
            let activity_obj: &JObject = activity.as_ref();
            env.call_method(
                activity_obj,
                name,
                signature,
                &[JValue::Object(array.as_ref()), JValue::Int(1)],
            )?;
            Ok(())
        });
        if requested.is_ok() {
            super::LocationPermission::Requested
        } else {
            super::LocationPermission::Unsupported
        }
    }

    pub(super) fn ensure_background_permission() -> super::LocationPermission {
        let granted = with_activity(|env, activity| -> anyhow::Result<bool> {
            let context: &AContext = activity.as_ref();
            let permission = JString::new(env, "android.permission.ACCESS_BACKGROUND_LOCATION")?;
            Ok(context.check_self_permission(env, permission)? == 0)
        })
        .unwrap_or(false);
        if granted {
            return super::LocationPermission::Granted;
        }
        let requested = with_activity(|env, activity| -> anyhow::Result<()> {
            let class_name = jni::strings::JNIString::from("java/lang/String");
            let class = env.find_class(class_name)?;
            let array = env.new_object_array(2, &class, JObject::null())?;
            for (index, name) in [
                "android.permission.ACCESS_BACKGROUND_LOCATION",
                "android.permission.POST_NOTIFICATIONS",
            ]
            .iter()
            .enumerate()
            {
                let permission = JString::new(env, *name)?;
                array.set_element(env, index, &permission)?;
            }
            let name = jni::strings::JNIString::from("requestPermissions");
            let runtime =
                jni::signature::RuntimeMethodSignature::from_str("([Ljava/lang/String;I)V")?;
            let signature = jni::signature::MethodSignature::from(&runtime);
            let activity_obj: &JObject = activity.as_ref();
            env.call_method(
                activity_obj,
                name,
                signature,
                &[JValue::Object(array.as_ref()), JValue::Int(2)],
            )?;
            Ok(())
        });
        if requested.is_ok() {
            super::LocationPermission::Requested
        } else {
            super::LocationPermission::Unsupported
        }
    }

    pub(super) fn last_known() -> Option<(f64, f64, f64)> {
        with_activity(|env, activity| -> anyhow::Result<Option<(f64, f64, f64)>> {
            let granted = has_location_permission(env, activity)?;
            tracing::debug!(granted, "android GPS: permission");
            if !granted {
                return Ok(None);
            }
            let context: &AContext = activity.as_ref();
            let service_name = JString::new(env, "location")?;
            let service = context.get_system_service(env, service_name)?;
            if service.is_null() {
                tracing::warn!("android GPS: no location service");
                return Ok(None);
            }
            let mut providers = provider_names(env, &service).unwrap_or_default();
            if providers.is_empty() {
                providers = vec![
                    "gps".to_string(),
                    "fused".to_string(),
                    "network".to_string(),
                    "passive".to_string(),
                ];
            }
            providers.sort_by_key(|name| match name.as_str() {
                "gps" => 0,
                "fused" => 1,
                "network" => 2,
                "passive" => 3,
                _ => 4,
            });
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            for provider_name in providers {
                // A single provider throwing (SecurityException, dead binder)
                // must not hide the remaining providers.
                let candidate = (|| -> anyhow::Result<Option<(f64, f64, f64)>> {
                    let provider = JString::new(env, &provider_name)?;
                    let name = jni::strings::JNIString::from("getLastKnownLocation");
                    let runtime = jni::signature::RuntimeMethodSignature::from_str(
                        "(Ljava/lang/String;)Landroid/location/Location;",
                    )?;
                    let signature = jni::signature::MethodSignature::from(&runtime);
                    let location = env
                        .call_method(
                            &service,
                            name,
                            signature,
                            &[JValue::Object(provider.as_ref())],
                        )?
                        .l()?;
                    if location.is_null() {
                        tracing::debug!(provider = provider_name, "android GPS: no last fix");
                        return Ok(None);
                    }
                    let lat = call_double(env, &location, "getLatitude", "()D")?;
                    let lon = call_double(env, &location, "getLongitude", "()D")?;
                    let accuracy = call_float(env, &location, "getAccuracy", "()F")? as f64;
                    let time = call_long(env, &location, "getTime", "()J").unwrap_or(0);
                    let recent = super::fix_is_recent(time, now_ms);
                    tracing::debug!(
                        provider = provider_name,
                        accuracy,
                        recent,
                        "android GPS: last fix"
                    );
                    if !recent {
                        return Ok(None);
                    }
                    Ok(super::valid_fix(lat, lon, accuracy))
                })();
                match candidate {
                    Ok(Some(fix)) => return Ok(Some(fix)),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            provider = provider_name,
                            %error,
                            "android GPS: provider failed"
                        );
                    }
                }
            }
            Ok(None)
        })
        .ok()
        .flatten()
    }

    pub(super) fn request_single_fix(timeout: std::time::Duration) -> Option<(f64, f64, f64)> {
        if let Some(fix) = last_known() {
            return Some(fix);
        }
        let pending = match start_updates() {
            Ok(pending) => pending,
            Err(error) => {
                tracing::warn!(%error, "android GPS: could not start updates");
                return None;
            }
        };
        let started = std::time::Instant::now();
        let mut fix = None;
        while started.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Some(candidate) = last_known() {
                fix = Some(candidate);
                break;
            }
        }
        stop_updates(&pending);
        fix
    }

    fn start_updates() -> anyhow::Result<Global<JObject<'static>>> {
        with_activity(|env, activity| {
            if !has_location_permission(env, activity)? {
                anyhow::bail!("location permission not granted");
            }
            let context: &AContext = activity.as_ref();
            let service_name = JString::new(env, "location")?;
            let service = context.get_system_service(env, service_name)?;
            if service.is_null() {
                anyhow::bail!("no location service");
            }
            let action = JString::new(env, "com.pinpoint.desktop.LOCATION_WARMUP")?;
            let intent_class =
                env.find_class(jni::strings::JNIString::from("android/content/Intent"))?;
            let ctor_runtime =
                jni::signature::RuntimeMethodSignature::from_str("(Ljava/lang/String;)V")?;
            let ctor_sig = jni::signature::MethodSignature::from(&ctor_runtime);
            let intent =
                env.new_object(&intent_class, ctor_sig, &[JValue::Object(action.as_ref())])?;
            let package = JString::new(env, "com.pinpoint.desktop")?;
            let set_pkg_runtime = jni::signature::RuntimeMethodSignature::from_str(
                "(Ljava/lang/String;)Landroid/content/Intent;",
            )?;
            let set_pkg_sig = jni::signature::MethodSignature::from(&set_pkg_runtime);
            env.call_method(
                &intent,
                jni::strings::JNIString::from("setPackage"),
                set_pkg_sig,
                &[JValue::Object(package.as_ref())],
            )
            .map_err(|error| anyhow!("setPackage: {error}"))?;
            let pi_class =
                env.find_class(jni::strings::JNIString::from("android/app/PendingIntent"))?;
            let get_runtime = jni::signature::RuntimeMethodSignature::from_str(
                "(Landroid/content/Context;ILandroid/content/Intent;I)Landroid/app/PendingIntent;",
            )?;
            let get_sig = jni::signature::MethodSignature::from(&get_runtime);
            let activity_obj: &JObject = activity.as_ref();
            // FLAG_UPDATE_CURRENT | FLAG_MUTABLE: the system attaches the
            // location extra before delivering the private broadcast.
            let pending = env
                .call_static_method(
                    &pi_class,
                    jni::strings::JNIString::from("getBroadcast"),
                    get_sig,
                    &[
                        JValue::Object(activity_obj),
                        JValue::Int(0),
                        JValue::Object(intent.as_ref()),
                        JValue::Int(0x0A00_0000),
                    ],
                )?
                .l()?;
            if pending.is_null() {
                anyhow::bail!("PendingIntent.getBroadcast returned null");
            }
            for provider in ["gps", "fused", "network"] {
                let provider = JString::new(env, provider)?;
                let runtime = jni::signature::RuntimeMethodSignature::from_str(
                    "(Ljava/lang/String;JFLandroid/app/PendingIntent;)V",
                )?;
                let signature = jni::signature::MethodSignature::from(&runtime);
                // A missing provider throws; try the next one.
                let _ = env.call_method(
                    &service,
                    jni::strings::JNIString::from("requestLocationUpdates"),
                    signature,
                    &[
                        JValue::Object(provider.as_ref()),
                        JValue::Long(1000),
                        JValue::Float(0.0),
                        JValue::Object(pending.as_ref()),
                    ],
                );
            }
            Ok(env.new_global_ref(&pending)?)
        })
    }

    fn stop_updates(pending: &Global<JObject<'static>>) {
        let _ = with_activity(|env, activity| -> anyhow::Result<()> {
            let context: &AContext = activity.as_ref();
            let service_name = JString::new(env, "location")?;
            let service = context.get_system_service(env, service_name)?;
            let pending_obj: &JObject = pending.as_ref();
            if !service.is_null() {
                let runtime = jni::signature::RuntimeMethodSignature::from_str(
                    "(Landroid/app/PendingIntent;)V",
                )?;
                let signature = jni::signature::MethodSignature::from(&runtime);
                let _ = env.call_method(
                    &service,
                    jni::strings::JNIString::from("removeUpdates"),
                    signature,
                    &[JValue::Object(pending_obj)],
                );
            }
            let _ = env.call_method(
                pending_obj,
                jni::strings::JNIString::from("cancel"),
                jni::signature::MethodSignature::from(
                    &jni::signature::RuntimeMethodSignature::from_str("()V")?,
                ),
                &[],
            );
            Ok(())
        });
    }

    fn provider_names(env: &mut jni::Env, service: &JObject) -> anyhow::Result<Vec<String>> {
        let name = jni::strings::JNIString::from("getAllProviders");
        let runtime = jni::signature::RuntimeMethodSignature::from_str("()Ljava/util/List;")?;
        let signature = jni::signature::MethodSignature::from(&runtime);
        let list = env.call_method(service, name, signature, &[])?.l()?;
        if list.is_null() {
            return Ok(Vec::new());
        }
        let size_runtime = jni::signature::RuntimeMethodSignature::from_str("()I")?;
        let size_sig = jni::signature::MethodSignature::from(&size_runtime);
        let size = env
            .call_method(&list, jni::strings::JNIString::from("size"), size_sig, &[])?
            .i()?;
        let mut names = Vec::new();
        for index in 0..size {
            let get_runtime =
                jni::signature::RuntimeMethodSignature::from_str("(I)Ljava/lang/Object;")?;
            let get_sig = jni::signature::MethodSignature::from(&get_runtime);
            let item = env
                .call_method(
                    &list,
                    jni::strings::JNIString::from("get"),
                    get_sig,
                    &[JValue::Int(index)],
                )?
                .l()?;
            if item.is_null() {
                continue;
            }
            if let Ok(text) = env.as_cast::<JString>(&item) {
                names.push(text.to_string());
            }
        }
        Ok(names)
    }

    fn call_int(env: &mut jni::Env, object: &JObject, name: &str) -> anyhow::Result<i32> {
        let name = jni::strings::JNIString::from(name);
        let runtime = jni::signature::RuntimeMethodSignature::from_str("()I")?;
        let signature = jni::signature::MethodSignature::from(&runtime);
        Ok(env.call_method(object, name, signature, &[])?.i()?)
    }

    fn call_int_arg(
        env: &mut jni::Env,
        object: &JObject,
        name: &str,
        arg: i32,
    ) -> anyhow::Result<i32> {
        let name = jni::strings::JNIString::from(name);
        let runtime = jni::signature::RuntimeMethodSignature::from_str("(I)I")?;
        let signature = jni::signature::MethodSignature::from(&runtime);
        Ok(env
            .call_method(object, name, signature, &[JValue::Int(arg)])?
            .i()?)
    }

    fn call_double(
        env: &mut jni::Env,
        object: &JObject,
        name: &str,
        signature: &str,
    ) -> anyhow::Result<f64> {
        let name = jni::strings::JNIString::from(name);
        let runtime = jni::signature::RuntimeMethodSignature::from_str(signature)?;
        let signature = jni::signature::MethodSignature::from(&runtime);
        Ok(env.call_method(object, name, signature, &[])?.d()?)
    }

    fn call_long(
        env: &mut jni::Env,
        object: &JObject,
        name: &str,
        signature: &str,
    ) -> anyhow::Result<i64> {
        let name = jni::strings::JNIString::from(name);
        let runtime = jni::signature::RuntimeMethodSignature::from_str(signature)?;
        let signature = jni::signature::MethodSignature::from(&runtime);
        Ok(env.call_method(object, name, signature, &[])?.j()?)
    }

    fn call_float(
        env: &mut jni::Env,
        object: &JObject,
        name: &str,
        signature: &str,
    ) -> anyhow::Result<f32> {
        let name = jni::strings::JNIString::from(name);
        let runtime = jni::signature::RuntimeMethodSignature::from_str(signature)?;
        let signature = jni::signature::MethodSignature::from(&runtime);
        Ok(env.call_method(object, name, signature, &[])?.f()?)
    }
}

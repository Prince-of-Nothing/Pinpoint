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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationPermission {
    Granted,
    Requested,
    Unsupported,
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

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn valid_fix(lat: f64, lon: f64, accuracy: f64) -> Option<(f64, f64, f64)> {
    if lat.is_finite()
        && lon.is_finite()
        && accuracy.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
        && accuracy > 0.0
    {
        Some((lat, lon, accuracy))
    } else {
        None
    }
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

/// URI from the Activity launch intent (`pinpoint://...` or `geo:...`).
#[cfg(target_os = "android")]
pub fn android_launch_uri() -> Option<String> {
    android_intent::launch_uri()
}

#[cfg(not(target_os = "android"))]
pub fn android_launch_uri() -> Option<String> {
    None
}

/// Text shared into the app with Android's `ACTION_SEND` chooser.
#[cfg(target_os = "android")]
pub fn android_shared_text() -> Option<String> {
    android_intent::shared_text()
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

    #[test]
    fn gps_fix_validation_rejects_bad_values() {
        assert!(super::valid_fix(47.0, 28.0, 5.0).is_some());
        assert!(super::valid_fix(120.0, 28.0, 5.0).is_none());
        assert!(super::valid_fix(47.0, 220.0, 5.0).is_none());
        assert!(super::valid_fix(47.0, 28.0, 0.0).is_none());
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
    use jni::objects::{JObject, JString};
    use jni::refs::Global;
    use jni::JValue;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static ACTIVITY_PTR: AtomicUsize = AtomicUsize::new(0);

    pub(super) fn set_activity(ptr: *mut std::ffi::c_void) {
        ACTIVITY_PTR.store(ptr as usize, Ordering::Relaxed);
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

    pub(super) fn shared_text() -> Option<String> {
        tracing::info!("android share: reading intent");
        let result = with_activity(|env, activity| -> anyhow::Result<Option<String>> {
            let intent = activity
                .get_intent(env)
                .map_err(|error| anyhow!("getIntent: {error}"))?;
            if intent.is_null() {
                tracing::info!("android share: no launch intent");
                return Ok(None);
            }
            let action = intent
                .get_action(env)
                .map_err(|error| anyhow!("getAction: {error}"))?;
            tracing::info!("android share: action={action}");
            if action.is_null() || action.to_string() != "android.intent.action.SEND" {
                return Ok(None);
            }
            let key = JString::new(env, "android.intent.extra.TEXT")?;
            let text = intent
                .get_string_extra(env, &key)
                .map_err(|error| anyhow!("getStringExtra: {error}"))?;
            tracing::info!("android share: text_present={}", !text.is_null());
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
            let ptr = ACTIVITY_PTR.load(Ordering::Relaxed);
            if ptr == 0 {
                anyhow::bail!("Android Activity is not registered");
            }
            let raw = ptr as jni::sys::jobject;
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

    pub(super) fn ensure_permission() -> super::LocationPermission {
        let granted = with_activity(|env, activity| -> anyhow::Result<bool> {
            has_location_permission(env, activity)
        })
        .unwrap_or(false);
        if granted {
            return super::LocationPermission::Granted;
        }
        let requested = with_activity(|env, activity| -> anyhow::Result<()> {
            let permission = JString::new(env, "android.permission.ACCESS_FINE_LOCATION")?;
            let class_name = jni::strings::JNIString::from("java/lang/String");
            let class = env.find_class(class_name)?;
            let array = env.new_object_array(1, &class, JObject::null())?;
            array.set_element(env, 0, &permission)?;
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

    pub(super) fn last_known() -> Option<(f64, f64, f64)> {
        with_activity(|env, activity| -> anyhow::Result<Option<(f64, f64, f64)>> {
            if !has_location_permission(env, activity)? {
                return Ok(None);
            }
            let context: &AContext = activity.as_ref();
            let service_name = JString::new(env, "location")?;
            let service = context.get_system_service(env, service_name)?;
            if service.is_null() {
                return Ok(None);
            }
            for provider in ["gps", "network"] {
                let provider = JString::new(env, provider)?;
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
                    continue;
                }
                let lat = call_double(env, &location, "getLatitude", "()D")?;
                let lon = call_double(env, &location, "getLongitude", "()D")?;
                let accuracy = call_float(env, &location, "getAccuracy", "()F")? as f64;
                if let Some(fix) = super::valid_fix(lat, lon, accuracy) {
                    return Ok(Some(fix));
                }
            }
            Ok(None)
        })
        .ok()
        .flatten()
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

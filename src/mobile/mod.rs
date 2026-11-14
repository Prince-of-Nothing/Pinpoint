//! Mobile (Android/iOS) app bootstrap — modular and backend-adaptable.
//!
//! Both mobile targets share the same core, UI, and [`crate::backend::Backend`]
//! abstraction as desktop. Platform-specific integration (activity lifecycle,
//! storage, permissions) belongs behind `cfg(target_os)` and thin wrappers so
//! backend architecture changes never require mobile-only rewrites.

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;

use crate::backend::BackendConfig;

/// Shared mobile launch options derived from host/platform config.
#[derive(Debug, Clone)]
pub struct MobileLaunchConfig {
    pub backend: BackendConfig,
    pub offline_first: bool,
    pub app_kind: crate::webapp::WebAppKind,
}

impl Default for MobileLaunchConfig {
    fn default() -> Self {
        Self {
            backend: BackendConfig::from_env(),
            offline_first: false,
            app_kind: crate::webapp::WebAppKind::Web,
        }
    }
}

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

/// Resolved backend config for mobile launches (env/host override > platform
/// default). Adapters enforce transport policy (ADR-0006).
pub fn mobile_backend_config() -> BackendConfig {
    BackendConfig::from_env()
}

/// Build a backend description for diagnostics (no network side effects beyond
/// what the adapter constructor does).
pub fn mobile_backend_label(cfg: &BackendConfig) -> String {
    format!("{:?}@{}", cfg.kind, cfg.base_url)
}

/// Touch-friendly viewport metrics for mobile screens (Android / iOS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MobileViewportProfile {
    pub pixels_per_point: f32,
    pub min_tap_target_pt: f32,
    pub portrait_default: bool,
}

impl Default for MobileViewportProfile {
    fn default() -> Self {
        Self {
            pixels_per_point: 2.0,
            min_tap_target_pt: 44.0,
            portrait_default: true,
        }
    }
}

/// Mobile OS lifecycle events bridged from Android `Activity` / iOS `UIApplication`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileLifecycleEvent {
    Resumed,
    Paused,
    LowMemory,
    BackPressed,
}

/// Parsed mobile deep link (`geo:lat,lon` or `pinpoint://<screen>`).
#[derive(Debug, Clone, PartialEq)]
pub enum MobileDeepLink {
    /// Standard RFC 5870 `geo:lat,lon` (optional `?q=label`) navigation intent.
    Geo {
        latitude: f64,
        longitude: f64,
        label: Option<String>,
    },
    /// App-internal route (`pinpoint://map`, `pinpoint://emergency`, etc.).
    Screen(crate::app::CurrentScreen),
    /// Kiosk share-link deep link (`pinpoint://kiosk?token=XYZ` or `pinpoint://share?token=XYZ`).
    Kiosk { token: Option<String> },
}

impl MobileDeepLink {
    /// Parse a `geo:` or `pinpoint://` URI fail-closed. Out-of-range
    /// coordinates or unknown screens return `None`.
    pub fn parse(uri: &str) -> Option<Self> {
        let s = uri.trim();
        if let Some(rest) = s.strip_prefix("geo:") {
            let (coords_part, query_part) = match rest.split_once('?') {
                Some((c, q)) => (c, Some(q)),
                None => (rest, None),
            };
            let coords_only = coords_part.split(';').next().unwrap_or(coords_part);
            let mut it = coords_only.split(',');
            let lat = it.next()?.trim().parse::<f64>().ok()?;
            let lon = it.next()?.trim().parse::<f64>().ok()?;
            if !crate::security::validation::validate_location(lat, lon, None).is_valid() {
                return None;
            }
            let label = query_part.and_then(|q| {
                q.split('&')
                    .find_map(|kv| kv.strip_prefix("q="))
                    .map(|v| v.replace('+', " "))
                    .filter(|v| !v.trim().is_empty())
            });
            return Some(Self::Geo {
                latitude: lat,
                longitude: lon,
                label,
            });
        }
        if let Some(rest) = s.strip_prefix("pinpoint://") {
            let (slug_raw, query_part) = match rest.split_once('?') {
                Some((r, q)) => (r.trim_matches('/'), Some(q)),
                None => (rest.trim_matches('/'), None),
            };
            if slug_raw == "kiosk" || slug_raw == "share" || slug_raw == "webk" {
                let token = query_part.and_then(|q| {
                    q.split('&')
                        .find_map(|kv| kv.strip_prefix("token="))
                        .map(|v| v.to_string())
                        .filter(|t| !t.trim().is_empty())
                });
                return Some(Self::Kiosk { token });
            }
            let screen = match slug_raw {
                "map" => crate::app::CurrentScreen::Map,
                "friends" => crate::app::CurrentScreen::Friends,
                "sharing" => crate::app::CurrentScreen::Sharing,
                "places" => crate::app::CurrentScreen::Places,
                "bookmarks" | "saved" => crate::app::CurrentScreen::Bookmarks,
                "trips" => crate::app::CurrentScreen::Trips,
                "emergency" => crate::app::CurrentScreen::Emergency,
                "profile" => crate::app::CurrentScreen::Profile,
                "admin" | "weba" => crate::app::CurrentScreen::Admin,
                _ => return None,
            };
            return Some(Self::Screen(screen));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CurrentScreen;

    #[test]
    fn mobile_launch_config_defaults_to_online_first() {
        let cfg = MobileLaunchConfig::default();
        assert!(
            !cfg.offline_first,
            "mobile should use the live backend and cached tiles when available"
        );
        assert_eq!(cfg.app_kind, crate::webapp::WebAppKind::Web);
        let label = mobile_backend_label(&cfg.backend);
        assert!(label.contains('@'));
    }

    #[test]
    fn geo_deep_link_parses_and_validates_coordinates() {
        let link = MobileDeepLink::parse("geo:47.0105,28.8638?q=Chisinau+Center").unwrap();
        assert_eq!(
            link,
            MobileDeepLink::Geo {
                latitude: 47.0105,
                longitude: 28.8638,
                label: Some("Chisinau Center".to_string()),
            }
        );
        // Invalid coordinates fail closed.
        assert!(MobileDeepLink::parse("geo:147.0,28.8").is_none());
        assert!(MobileDeepLink::parse("geo:not,numbers").is_none());
    }

    #[test]
    fn pinpoint_scheme_routes_to_screens() {
        assert_eq!(
            MobileDeepLink::parse("pinpoint://emergency"),
            Some(MobileDeepLink::Screen(CurrentScreen::Emergency))
        );
        assert_eq!(
            MobileDeepLink::parse("pinpoint://saved"),
            Some(MobileDeepLink::Screen(CurrentScreen::Bookmarks))
        );
        assert_eq!(
            MobileDeepLink::parse("pinpoint://admin"),
            Some(MobileDeepLink::Screen(CurrentScreen::Admin))
        );
        assert_eq!(
            MobileDeepLink::parse("pinpoint://kiosk?token=abc12345"),
            Some(MobileDeepLink::Kiosk {
                token: Some("abc12345".to_string())
            })
        );
        assert_eq!(
            MobileDeepLink::parse("pinpoint://share?token=xyz789"),
            Some(MobileDeepLink::Kiosk {
                token: Some("xyz789".to_string())
            })
        );
        assert!(MobileDeepLink::parse("pinpoint://unknown").is_none());
    }
}

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

//! Backend abstraction layer — makes the client modular and adaptable to
//! changed backend architectures.
//!
//! Design goals:
//! - `PinpointApp` and UI code depend on [`Backend`], not on a concrete HTTP
//!   client or wire format.
//! - The current REST/session-cookie implementation lives in [`crate::api`] and
//!   implements [`Backend`]. Swapping to a different backend architecture (new
//!   endpoints, auth scheme, protocol, or API version) means adding/adjusting an
//!   adapter behind this trait, not rewriting app logic.
//! - [`BackendConfig`] + [`build_backend`] provide a single construction point
//!   driven by config/env, so deployment targets can select the backend adapter.
//!
//! Security invariants (must hold for every adapter):
//! - Fail closed on transport/auth/session errors (ADR-0006 semantics).
//! - Never leak secrets/tokens in error messages or logs.
//! - Server-side enforcement only; clients are untrusted.

#[cfg(not(target_arch = "wasm32"))]
pub mod failover;
#[cfg(not(target_arch = "wasm32"))]
pub mod health;
#[cfg(not(target_arch = "wasm32"))]
pub mod rest;

use crate::models::{
    AdminAuditEntry, CheckIn, DiagnosticReport, EmergencyAlert, FriendLocation, Friendship,
    LocationShare, SessionInfo, User,
};
use crate::osm::{MapCapabilities, Route, SearchPlace};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Boxed future used by [`Backend`] for dyn-compatibility without extra
/// dependencies. Native targets require `Send`; wasm runs single-threaded.
#[cfg(not(target_arch = "wasm32"))]
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + 'a>>;

/// Bounds required for backend trait objects: `Send + Sync` on native, relaxed on wasm.
#[cfg(not(target_arch = "wasm32"))]
pub trait BackendBounds: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> BackendBounds for T {}

#[cfg(target_arch = "wasm32")]
pub trait BackendBounds {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> BackendBounds for T {}

/// Shared backend handle used across app/UI/webapp layers.
pub type DynBackend = Arc<dyn Backend>;

/// Abstracts all backend operations the Pinpoint clients need.
///
/// Implementations must preserve the security/contract expectations documented
/// on each method. The default REST adapter (`api::ApiClient`) maps these to
/// the current `/api/v1/*` contract; future adapters may target a different
/// backend architecture while keeping this surface stable.
pub trait Backend: BackendBounds {
    /// Base URL / identifier for diagnostics (never contains secrets).
    fn backend_url(&self) -> &str;
    /// Whether an authenticated session/state is currently held.
    fn session_active(&self) -> bool;
    /// Export opaque session material for keychain persistence.
    fn export_session(&self) -> Option<String>;
    /// Import opaque session material; fail closed on malformed data.
    fn import_session(&self, raw: &str) -> anyhow::Result<()>;

    fn login<'a>(&'a self, email: &'a str, password: &'a str) -> BackendFuture<'a, User>;
    /// Login with an optional TOTP code for MFA-enabled accounts.
    fn login_with_totp<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        totp: Option<&'a str>,
    ) -> BackendFuture<'a, User>;
    fn register<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        full_name: &'a str,
    ) -> BackendFuture<'a, User>;
    /// Confirm logout with the backend before dropping local session state.
    fn logout<'a>(&'a self) -> BackendFuture<'a, ()>;
    fn me(&self) -> BackendFuture<'_, User>;
    fn anonymous<'a>(&'a self, device_name: &'a str) -> BackendFuture<'a, (User, String)>;
    fn recover_account<'a>(&'a self, code: &'a str) -> BackendFuture<'a, (User, String)>;
    fn list_sessions(&self) -> BackendFuture<'_, Vec<SessionInfo>>;
    fn revoke_session<'a>(&'a self, id: &'a str) -> BackendFuture<'a, ()>;
    fn logout_all(&self) -> BackendFuture<'_, ()>;
    fn submit_diagnostic<'a>(
        &'a self,
        category: &'a str,
        message: &'a str,
        platform: &'a str,
    ) -> BackendFuture<'a, i64>;

    fn mfa_enroll<'a>(&'a self) -> BackendFuture<'a, (String, String)>;
    fn mfa_verify<'a>(&'a self, code: &'a str) -> BackendFuture<'a, ()>;
    fn mfa_status<'a>(&'a self) -> BackendFuture<'a, bool>;
    fn mfa_disable<'a>(&'a self) -> BackendFuture<'a, ()>;

    /// Returns `Ok(true)` when the backend revoked sessions (regular change),
    /// `Ok(false)` when the current session remains valid (forced change).
    fn change_password<'a>(
        &'a self,
        current_password: Option<&'a str>,
        new_password: &'a str,
        confirm_password: &'a str,
    ) -> BackendFuture<'a, bool>;

    fn get_friendships<'a>(&'a self) -> BackendFuture<'a, Vec<Friendship>>;
    fn send_friend_request<'a>(&'a self, email: &'a str) -> BackendFuture<'a, ()>;
    fn respond_to_friendship<'a>(&'a self, id: i64, action: &'a str) -> BackendFuture<'a, ()>;
    fn delete_friendship<'a>(&'a self, id: i64) -> BackendFuture<'a, ()>;

    fn get_location_shares<'a>(&'a self) -> BackendFuture<'a, Vec<LocationShare>>;
    fn create_location_share<'a>(
        &'a self,
        friendship_id: i64,
        direction: &'a str,
        precision_level: &'a str,
    ) -> BackendFuture<'a, ()>;
    fn create_share_with_expiry<'a>(
        &'a self,
        friendship_id: i64,
        direction: &'a str,
        precision_level: &'a str,
        duration_hours: Option<i64>,
        label: Option<&'a str>,
    ) -> BackendFuture<'a, ()>;
    fn update_location_share<'a>(
        &'a self,
        share_id: i64,
        is_active: Option<bool>,
        precision: Option<String>,
        duration_hours: Option<i64>,
    ) -> BackendFuture<'a, ()>;
    fn delete_share<'a>(&'a self, share_id: i64) -> BackendFuture<'a, ()>;

    fn get_friend_locations<'a>(&'a self) -> BackendFuture<'a, Vec<FriendLocation>>;
    fn publish_location<'a>(
        &'a self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> BackendFuture<'a, ()>;
    fn get_my_location<'a>(&'a self) -> BackendFuture<'a, FriendLocation>;

    fn map_capabilities(&self) -> BackendFuture<'_, MapCapabilities>;
    fn map_search<'a>(
        &'a self,
        q: &'a str,
        lat: Option<f64>,
        lon: Option<f64>,
        limit: usize,
    ) -> BackendFuture<'a, Vec<SearchPlace>>;
    fn map_reverse(&self, lat: f64, lon: f64) -> BackendFuture<'_, SearchPlace>;
    fn map_route<'a>(
        &'a self,
        from: (f64, f64),
        to: (f64, f64),
        mode: &'a str,
        via: &'a [(f64, f64)],
        avoid: &'a [String],
        alternatives: bool,
    ) -> BackendFuture<'a, Vec<Route>>;

    fn list_users<'a>(&'a self) -> BackendFuture<'a, Vec<User>>;
    fn search_users<'a>(&'a self, q: &'a str) -> BackendFuture<'a, Vec<User>>;
    fn update_user<'a>(&'a self, id: i64, body: serde_json::Value) -> BackendFuture<'a, ()>;
    fn delete_user<'a>(&'a self, id: i64) -> BackendFuture<'a, ()>;
    fn admin_overview(&self) -> BackendFuture<'_, serde_json::Value>;
    fn admin_users(&self) -> BackendFuture<'_, Vec<serde_json::Value>>;
    fn admin_update_user<'a>(&'a self, id: i64, body: serde_json::Value) -> BackendFuture<'a, ()>;
    fn admin_delete_user<'a>(&'a self, id: i64) -> BackendFuture<'a, ()>;
    fn admin_diagnostics<'a>(
        &'a self,
        status: Option<&'a str>,
    ) -> BackendFuture<'a, (Vec<DiagnosticReport>, i64, i64)>;
    fn admin_update_report<'a>(&'a self, id: i64, status: &'a str) -> BackendFuture<'a, ()>;
    fn admin_audit(&self) -> BackendFuture<'_, Vec<AdminAuditEntry>>;

    fn create_check_in<'a>(
        &'a self,
        lat: f64,
        lon: f64,
        note: &'a str,
        kind: &'a str,
    ) -> BackendFuture<'a, ()>;
    fn list_check_ins<'a>(&'a self) -> BackendFuture<'a, Vec<CheckIn>>;

    fn trigger_emergency<'a>(&'a self, lat: f64, lon: f64) -> BackendFuture<'a, ()>;
    fn list_emergencies<'a>(&'a self) -> BackendFuture<'a, Vec<EmergencyAlert>>;
}

/// Selects which backend adapter to construct. Add variants here when the
/// backend architecture changes (e.g. a new API version, protocol, or auth
/// model) and implement the adapter behind [`Backend`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendKind {
    /// Current REST contract (`/api/v1/*`, session cookie + CSRF double-submit).
    RestV1,
    /// Browser-adapted REST variant (fetch credentials/CORS semantics handled by
    /// adapter; endpoint/auth mapping may differ from native cookie handling).
    BrowserRestV1,
    /// Failover across multiple REST v1 URLs with circuit breakers, health
    /// monitoring, and exponential backoff retry. `base_url` is the primary;
    /// additional URLs come from `options["failover_urls"]` (comma-separated).
    FailoverV1,
    // Future: RestV2, GraphQL, GrpcWebBridge, Mock, etc.
}

/// Declares capabilities a backend/adapter may support. Useful for
/// feature-detection when backend architecture varies across deployments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub session_auth: bool,
    pub mfa: bool,
    pub friendships: bool,
    pub location_shares: bool,
    pub live_locations: bool,
    pub admin_users: bool,
    pub kiosk_share_links: bool,
    pub check_ins: bool,
    pub emergency: bool,
}

impl BackendCapabilities {
    pub fn rest_v1() -> Self {
        Self {
            session_auth: true,
            mfa: true,
            friendships: true,
            location_shares: true,
            live_locations: true,
            admin_users: true,
            kiosk_share_links: false,
            check_ins: true,
            emergency: true,
        }
    }
}

/// Configuration for constructing a backend client.
#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub kind: BackendKind,
    /// Base URL for network backends (validated per adapter policy).
    pub base_url: String,
    /// Optional request timeout override in seconds.
    pub timeout_secs: Option<u64>,
    /// Free-form adapter options for future backend-specific settings
    /// (auth mode, tenant, API version pin, feature flags). Keys/values must
    /// never contain secrets in logs; adapters should treat unknown keys as
    /// non-fatal.
    pub options: std::collections::BTreeMap<String, String>,
}

impl BackendConfig {
    pub fn rest_v1(base_url: impl Into<String>) -> Self {
        Self {
            kind: BackendKind::RestV1,
            base_url: base_url.into(),
            timeout_secs: None,
            options: std::collections::BTreeMap::new(),
        }
    }

    pub fn browser_rest_v1(base_url: impl Into<String>) -> Self {
        Self {
            kind: BackendKind::BrowserRestV1,
            base_url: base_url.into(),
            timeout_secs: None,
            options: std::collections::BTreeMap::new(),
        }
    }

    /// Failover config: primary URL plus comma-separated replica URLs in
    /// `PINPOINT_FAILOVER_URLS` (or passed via `options["failover_urls"]`).
    pub fn failover_v1(base_url: impl Into<String>, failover_urls: Vec<String>) -> Self {
        let mut options = std::collections::BTreeMap::new();
        if !failover_urls.is_empty() {
            options.insert("failover_urls".to_string(), failover_urls.join(","));
        }
        Self {
            kind: BackendKind::FailoverV1,
            base_url: base_url.into(),
            timeout_secs: None,
            options,
        }
    }

    /// Builder-style option setter (non-secret options only).
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    pub fn capabilities(&self) -> BackendCapabilities {
        match self.kind {
            BackendKind::RestV1 | BackendKind::BrowserRestV1 | BackendKind::FailoverV1 => {
                BackendCapabilities::rest_v1()
            }
        }
    }

    /// Resolve config from environment/platform defaults, preserving the
    /// existing resolution order: `PINPOINT_BACKEND` > platform default.
    /// Reads `PINPOINT_FAILOVER_URLS` (comma-separated) for redundancy.
    pub fn from_env() -> Self {
        let base_url = crate::platform::backend_url_from_env_or_default();
        let failover_urls: Vec<String> = std::env::var("PINPOINT_FAILOVER_URLS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if !failover_urls.is_empty() {
            Self::failover_v1(base_url, failover_urls)
        } else {
            Self::rest_v1(base_url)
        }
    }
}

/// Build a backend from config. This is the single extension point for changed
/// backend architectures. Native only: wasm web stubs use their host's fetch
/// adapter instead of the cookie/CSRF REST client.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_backend(config: &BackendConfig) -> anyhow::Result<DynBackend> {
    use crate::backend::failover::FailoverBackend;
    match config.kind {
        BackendKind::RestV1 | BackendKind::BrowserRestV1 => rest::build_rest_backend(config),
        BackendKind::FailoverV1 => {
            let primary = config.base_url.clone();
            let mut urls = vec![primary.clone()];
            if let Some(extra) = config.options.get("failover_urls") {
                for u in extra.split(',') {
                    let u = u.trim();
                    if !u.is_empty() && u != primary {
                        urls.push(u.to_string());
                    }
                }
            }
            let fb = FailoverBackend::new(urls)?
                .with_max_retries(3)
                .with_base_backoff_ms(500);
            Ok(Arc::new(fb) as DynBackend)
        } // Note: BrowserRestV1 currently shares the REST adapter; a dedicated
          // browser fetch/credential adapter can be added here without changing callers.
    }
}

/// Convenience: build the default REST backend from a base URL.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_default_backend(base_url: impl Into<String>) -> anyhow::Result<DynBackend> {
    build_backend(&BackendConfig::rest_v1(base_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_has_non_empty_url() {
        let cfg = BackendConfig::from_env();
        assert!(!cfg.base_url.is_empty());
        assert_eq!(cfg.kind, BackendKind::RestV1);
    }
}

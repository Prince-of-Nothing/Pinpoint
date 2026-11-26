//! `webk` — Kiosk/check-in webapp logic (modular, backend-adaptable).
//!
//! Kiosk flows are designed for minimal-friction, consent-checked views (e.g.
//! share-link/QR, short-lived token). The exact backend endpoints may evolve;
//! this module isolates kiosk-specific behavior behind adapter points so the UI
//! can remain stable when the backend architecture changes.

use crate::backend::{BackendFuture, DynBackend};
use crate::models::FriendLocation;
use crate::webapp::config::WebAppConfig;
use serde::{Deserialize, Serialize};

/// Token/share-link claims as understood by the kiosk client. The backend may
/// issue different shapes; adapters normalize into this view model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KioskShareView {
    pub token: String,
    pub label: Option<String>,
    pub expires_at: Option<String>,
    pub location: Option<FriendLocation>,
    pub consent_required: bool,
}

/// Check-in payload for kiosk flows (backend-adapter-specific mapping).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KioskCheckInRequest {
    pub token: String,
    pub note: Option<String>,
    /// Optional coarse/manual coordinates if the kiosk flow allows them.
    /// Server must validate/authorize; clients must not bypass consent.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

/// REST endpoint mapping for kiosk operations. Centralized so a backend
/// architecture change (path/method/shape) only touches this config/adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KioskRestRoutes {
    /// Resolve/validate a share token (GET/POST depending on backend version).
    pub resolve_share: String,
    /// Submit a kiosk check-in.
    pub submit_check_in: String,
}

impl Default for KioskRestRoutes {
    fn default() -> Self {
        Self {
            // Placeholder defaults; override per backend architecture/version.
            resolve_share: "/api/v1/share-links/{token}".to_string(),
            submit_check_in: "/api/v1/kiosk/check-ins".to_string(),
        }
    }
}

impl KioskRestRoutes {
    pub fn resolve_share_path(&self, token: &str) -> String {
        self.resolve_share
            .replace("{token}", &url_encode_component(token))
    }
}

fn url_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Optional kiosk-specific backend capability.
///
/// Not every backend architecture exposes these operations the same way. When a
/// backend supports kiosk/share-link flows, implement this trait and expose it
/// alongside [`DynBackend`] (or via a dedicated adapter). Callers must handle
/// unsupported/unwired adapters gracefully (fail closed with clear errors).
#[cfg(not(target_arch = "wasm32"))]
pub trait KioskBackend: Send + Sync {
    fn resolve_share_token<'a>(&'a self, token: &'a str) -> BackendFuture<'a, KioskShareView>;
    fn submit_check_in<'a>(&'a self, request: KioskCheckInRequest) -> BackendFuture<'a, ()>;
}
#[cfg(target_arch = "wasm32")]
pub trait KioskBackend {
    fn resolve_share_token<'a>(&'a self, token: &'a str) -> BackendFuture<'a, KioskShareView>;
    fn submit_check_in<'a>(&'a self, request: KioskCheckInRequest) -> BackendFuture<'a, ()>;
}

/// Portable kiosk controller state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KioskState {
    pub token_input: String,
    pub resolved: Option<KioskShareView>,
    pub consent_accepted: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

impl KioskState {
    pub fn clear_messages(&mut self) {
        self.notice = None;
        self.error = None;
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
        self.notice = None;
        self.loading = false;
    }

    pub fn can_view(&self) -> bool {
        match &self.resolved {
            Some(v) if v.consent_required => self.consent_accepted,
            Some(_) => true,
            None => false,
        }
    }
}

/// High-level kiosk app controller (UI-framework agnostic).
#[derive(Debug, Clone)]
pub struct KioskApp {
    pub config: WebAppConfig,
    pub routes: KioskRestRoutes,
    pub state: KioskState,
}

impl KioskApp {
    pub fn new(config: WebAppConfig) -> Self {
        let token = config.share_token.clone().unwrap_or_default();
        Self {
            config,
            routes: KioskRestRoutes::default(),
            state: KioskState {
                token_input: token,
                ..Default::default()
            },
        }
    }

    /// Basic client-side token sanity check (never a substitute for server
    /// validation; fail closed on obviously invalid input).
    pub fn validate_token(token: &str) -> Result<(), String> {
        let t = token.trim();
        if t.is_empty() {
            return Err("Enter a share token or check-in code.".into());
        }
        if t.len() > crate::webapp::validation::MAX_TOKEN_LEN {
            return Err("Token is too long.".into());
        }
        if t.chars().any(|c| c.is_control()) {
            return Err("Token contains invalid characters.".into());
        }
        Ok(())
    }

    pub fn prepare_resolve(&mut self) -> Result<String, String> {
        self.state.clear_messages();
        let token = self.state.token_input.trim().to_string();
        Self::validate_token(&token)?;
        self.state.loading = true;
        Ok(token)
    }

    pub fn apply_resolved(&mut self, view: KioskShareView) {
        self.state.loading = false;
        self.state.resolved = Some(view);
        self.state.notice = Some("Share token resolved. Review consent before viewing.".into());
    }

    pub fn prepare_check_in(
        &mut self,
        note: Option<String>,
    ) -> Result<KioskCheckInRequest, String> {
        self.state.clear_messages();
        let token = self
            .state
            .resolved
            .as_ref()
            .map(|v| v.token.clone())
            .or_else(|| {
                let t = self.state.token_input.trim();
                (!t.is_empty()).then(|| t.to_string())
            })
            .ok_or_else(|| "No token available for check-in.".to_string())?;
        Self::validate_token(&token)?;
        let note = crate::webapp::validation::validate_note(note.as_deref())?;
        if let Some(v) = &self.state.resolved {
            if v.consent_required && !self.state.consent_accepted {
                return Err("Consent is required before check-in.".into());
            }
        }
        self.state.loading = true;
        Ok(KioskCheckInRequest {
            token,
            note,
            latitude: None,
            longitude: None,
        })
    }

    pub fn snapshot_json(&self) -> String {
        serde_json::json!({
            "app": crate::webapp::WebAppKind::Kiosk.slug(),
            "backend_url": self.config.backend.base_url,
            "backend_kind": format!("{:?}", self.config.backend.kind),
            "has_token": !self.state.token_input.trim().is_empty(),
            "has_resolved": self.state.resolved.is_some(),
            "can_view": self.state.can_view(),
            "loading": self.state.loading,
            "notice": self.state.notice,
            "error": self.state.error,
        })
        .to_string()
    }
}

/// Attempt to resolve a kiosk token using a capability if available.
/// Returns a user-facing error when the current backend adapter doesn't support
/// kiosk operations (so UI can degrade cleanly during backend transitions).
pub async fn resolve_with_backend(
    _backend: &DynBackend,
    kiosk: Option<&dyn KioskBackend>,
    token: &str,
) -> anyhow::Result<KioskShareView> {
    if let Some(k) = kiosk {
        return k.resolve_share_token(token).await;
    }
    Err(anyhow::anyhow!(
        "Kiosk/share-link backend capability is not configured for this backend adapter. \
         Configure a kiosk-capable backend or update the adapter for the current backend architecture."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_validation_fail_closed() {
        assert!(KioskApp::validate_token("").is_err());
        assert!(KioskApp::validate_token("   ").is_err());
        assert!(KioskApp::validate_token("ok-token_123").is_ok());
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn consent_gates_view() {
        let mut state = KioskState::default();
        state.resolved = Some(KioskShareView {
            token: "t".into(),
            consent_required: true,
            ..Default::default()
        });
        assert!(!state.can_view());
        state.consent_accepted = true;
        assert!(state.can_view());
    }
}

//! Shared webapp modules for Pinpoint browser clients.
//!
//! `web/` (general), `web/weba/` (admin console), and `web/webk/` (kiosk
//! check-in) share this core so UI and backend integration stay modular.
//! Backend access goes through [`crate::backend::Backend`] adapters, making the
//! webapps adaptable to changed backend architectures without rewriting screens.
//!
//! Web/WASM UI rendering is intentionally decoupled: the desktop egui UI lives
//! under `src/ui` (`cfg(not(wasm32))`), while these modules provide portable
//! app/view-model primitives and configuration that wasm bins and future egui
//! web shells can consume.

pub mod admin;
pub mod commands;
pub mod config;
pub mod kiosk;
#[cfg(not(target_arch = "wasm32"))]
pub mod kiosk_rest;
pub mod validation;
pub mod view_model;
pub mod web;

/// Identifies which web application variant is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAppKind {
    /// General map-first web client.
    Web,
    /// Admin console (`weba`).
    Admin,
    /// Kiosk/check-in client (`webk`).
    Kiosk,
}

impl WebAppKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Web => "Pinpoint Web",
            Self::Admin => "Pinpoint Admin (weba)",
            Self::Kiosk => "Pinpoint Kiosk (webk)",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Admin => "weba",
            Self::Kiosk => "webk",
        }
    }

    /// Whether this variant requires a signed-in session for its primary flows.
    pub fn requires_auth(self) -> bool {
        match self {
            Self::Web => false,
            Self::Admin => true,
            Self::Kiosk => false, // token/share-link flows, not account login
        }
    }
}

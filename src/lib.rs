//! Pinpoint — Rust multiplatform client for the Pinpoint API.
//!
//! One codebase for desktop (Windows/Linux/macOS), Android (`cargo-apk` +
//! `eframe`/`winit`) and iOS (`cargo-xcodebuild`), plus a wasm stub target.
//! The library target holds the portable core so contract tests under
//! `tests/` can drive [`api::ApiClient`] and [`osm::OsmClient`] without
//! launching a window.
//!
//! Backend access is modular via [`backend::Backend`] / [`backend::DynBackend`]
//! so client apps (desktop/mobile/webk/weba) can adapt to changed backend
//! architectures by swapping adapters, not app logic.

// Native client + server modules. The wasm web stubs only need the portable
// core (`backend` trait, `webapp`, `models`, `osm`, `security`) so they do not
// pull axum/sqlx/tokio/mio into the wasm build.
#[cfg(not(target_arch = "wasm32"))]
pub mod api;
#[cfg(not(target_arch = "wasm32"))]
pub mod app;
pub mod backend;
pub mod bookmarks;
pub mod crash;
pub mod i18n;
pub mod logging;
#[cfg(not(target_arch = "wasm32"))]
pub mod mobile;
pub mod models;
pub mod osm;
#[cfg(not(target_arch = "wasm32"))]
pub mod patterns;
pub mod platform;
pub mod security;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod update;
pub mod webapp;
// The GUI modules depend on native-only crates (tokio runtime, `which`),
// so they are compiled out of the WebAssembly target.
#[cfg(not(target_arch = "wasm32"))]
pub mod map;
#[cfg(not(target_arch = "wasm32"))]
pub mod tilepacks;
#[cfg(not(target_arch = "wasm32"))]
pub mod ui;

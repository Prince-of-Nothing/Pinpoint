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
#[cfg(not(target_arch = "wasm32"))]
pub mod app_lock;
pub mod backend;
pub mod bookmarks;
pub mod crash;
pub mod i18n;
#[cfg(not(target_arch = "wasm32"))]
pub mod local_route;
pub mod logging;
#[cfg(not(target_arch = "wasm32"))]
pub mod mobile;
pub mod models;
#[cfg(not(target_arch = "wasm32"))]
pub mod offline_install;
pub mod osm;
#[cfg(not(target_arch = "wasm32"))]
pub mod patterns;
pub mod platform;
pub mod prefs;
#[cfg(not(target_arch = "wasm32"))]
pub mod region_catalog;
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
pub mod map_server;
// The optional native map window needs `wry`/`tao`, which exist only for the
// desktop targets (see the `map-webview` feature in Cargo.toml).
#[cfg(all(
    feature = "map-webview",
    not(target_arch = "wasm32"),
    not(target_os = "android"),
    not(target_os = "ios")
))]
pub mod map_window;
#[cfg(not(target_arch = "wasm32"))]
pub mod tilepacks;
#[cfg(not(target_arch = "wasm32"))]
pub mod ui;

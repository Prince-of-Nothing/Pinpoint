//! Pinpoint Admin web client (`weba`, Rust/WASM via `trunk`).
//!
//! Admin console SPA: uses the modular backend abstraction so backend
//! architecture changes are isolated to adapters/config. Server-side
//! authorization is authoritative; UI checks are fail-closed hints.

use pinpoint_desktop::webapp::{config::WebAppConfig, WebAppKind};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = WebAppConfig::new(WebAppKind::Admin);
    eprintln!("pinpoint-weba (native stub)");
    eprintln!("  app      : {}", WebAppKind::Admin.label());
    eprintln!(
        "  platform : {}",
        pinpoint_desktop::platform::Platform::current().label()
    );
    eprintln!("  backend  : {}", cfg.backend.base_url);
    eprintln!("  build web with: trunk build --config web/weba/Trunk.toml --release");
}

// ── WASM implementation ──────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        set_text("status", &format!("panic: {info}"))
    }));
    let cfg = WebAppConfig::new(WebAppKind::Admin);
    set_text("backend", &cfg.backend.base_url);
    set_text(
        "status",
        "Ready. Sign in with an admin account (server-enforced).",
    );
    let app = pinpoint_desktop::webapp::admin::AdminApp::new(cfg);
    set_text("app-info", &app.snapshot_json());
    if let Err(e) = wire_dom() {
        set_text("status", &format!("UI wiring error: {e:?}"));
    }
}

#[cfg(target_arch = "wasm32")]
fn wire_dom() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    if let Some(btn) = document.get_element_by_id("signin") {
        let closure = Closure::<dyn FnMut(_)>::new(move |_ev: web_sys::Event| {
            let email = get_input_value("email").unwrap_or_default();
            let password = get_input_value("password").unwrap_or_default();
            if email.trim().is_empty() || password.is_empty() {
                set_text("status", "Error: email and password are required.");
                return;
            }
            // Backend login requires async HTTP. In browsers, cookie/CSRF handling
            // differs from native (browser-managed cookies, CORS/credentials). The
            // modular backend adapter is the right place to provide a browser-safe
            // transport; wire it here once configured for this deployment.
            set_text(
                "status",
                "Login requested. Configure a browser-compatible backend adapter/endpoint for this deployment (modular backend seam).",
            );
            set_html(
                "admin-panel",
                &format!(
                    "<p class='muted'>Prepared admin login for <code>{}</code>. Adapter wiring pending for browser credential flow.</p>",
                    pinpoint_desktop::webapp::validation::escape_html(email.trim())
                ),
            );
        });
        btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn get_input_value(id: &str) -> Option<String> {
    web_sys::window()?
        .document()?
        .get_element_by_id(id)?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

#[cfg(target_arch = "wasm32")]
fn set_text(id: &str, text: &str) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
    {
        el.set_text_content(Some(text));
    }
}

#[cfg(target_arch = "wasm32")]
fn set_html(id: &str, html: &str) {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
    {
        el.set_inner_html(html);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn weba_app_info() -> String {
    let cfg = WebAppConfig::new(WebAppKind::Admin);
    let app = pinpoint_desktop::webapp::admin::AdminApp::new(cfg);
    app.snapshot_json()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn weba_backend_url() -> String {
    WebAppConfig::new(WebAppKind::Admin).backend.base_url
}

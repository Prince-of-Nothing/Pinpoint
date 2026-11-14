//! Pinpoint Kiosk web client (`webk`, Rust/WASM via `trunk`).
//!
//! Modular kiosk/check-in client. Token/share-link/check-in contracts are
//! isolated in `webapp::kiosk` + adapter routes so backend architecture changes
//! do not require UI rewrites.

use pinpoint_desktop::webapp::{config::WebAppConfig, kiosk::KioskApp, WebAppKind};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let cfg = WebAppConfig::new(WebAppKind::Kiosk);
    let app = KioskApp::new(cfg);
    eprintln!("pinpoint-webk (native stub)");
    eprintln!("  app      : {}", WebAppKind::Kiosk.label());
    eprintln!(
        "  platform : {}",
        pinpoint_desktop::platform::Platform::current().label()
    );
    eprintln!("  backend  : {}", app.config.backend.base_url);
    eprintln!("  resolve  : {}", app.routes.resolve_share);
    eprintln!("  check-in : {}", app.routes.submit_check_in);
    eprintln!("  build web with: trunk build --config web/webk/Trunk.toml --release");
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
    std::panic::set_hook(Box::new(console_error_panic_hook_like));
    let cfg = WebAppConfig::new(WebAppKind::Kiosk);
    let mut app = KioskApp::new(cfg);
    // Pre-fill token from query string if present.
    if let Some(token) = query_param("token") {
        app.state.token_input = token;
    }
    if let Err(e) = wire_dom(app) {
        set_text("status", &format!("UI wiring error: {e:?}"));
    }
}

#[cfg(target_arch = "wasm32")]
fn wire_dom(mut app: KioskApp) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;

    // Bootstrap info.
    set_input_value(&document, "token", &app.state.token_input);
    set_text("backend", &app.config.backend.base_url);
    set_text(
        "status",
        "Ready. Enter a token to resolve (modular kiosk adapter).",
    );
    set_text("app-info", &app.snapshot_json());

    // Open/resolve button.
    {
        let doc = document.clone();
        let app = std::rc::Rc::new(std::cell::RefCell::new(app));
        let app_for_click = app.clone();
        if let Some(btn) = document.get_element_by_id("open") {
            let closure = Closure::<dyn FnMut(_)>::new(move |_ev: web_sys::Event| {
                let mut a = app_for_click.borrow_mut();
                a.state.token_input = get_input_value(&doc, "token").unwrap_or_default();
                match a.prepare_resolve() {
                    Ok(_token) => {
                        // The wasm kiosk stub does not ship the native REST
                        // adapter; token resolution stays in the native client.
                        a.state.set_error(
                            "Token resolution is not wired in the wasm stub.".to_string(),
                        );
                        set_text(
                            "status",
                            "WASM stub: use the native kiosk client to resolve a token.",
                        );
                        set_text("app-info", &a.snapshot_json());
                    }
                    Err(e) => {
                        a.state.set_error(e.clone());
                        set_text("status", &format!("Error: {e}"));
                        set_text("app-info", &a.snapshot_json());
                    }
                }
            });
            btn.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
            closure.forget();
        }
        // Consent toggle if present.
        if let Some(cb) = document.get_element_by_id("consent") {
            let app_for_consent = app.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |_ev: web_sys::Event| {
                if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                    let checked = doc
                        .get_element_by_id("consent")
                        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
                        .map(|i| i.checked())
                        .unwrap_or(false);
                    app_for_consent.borrow_mut().state.consent_accepted = checked;
                    set_text("app-info", &app_for_consent.borrow().snapshot_json());
                }
            });
            cb.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())?;
            closure.forget();
        }
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn render_resolved(app: &KioskApp) {
    if let Some(v) = &app.state.resolved {
        let mut msg = format!("Resolved token. consent_required={}", v.consent_required);
        if let Some(label) = &v.label {
            msg.push_str(&format!(" label={label}"));
        }
        if let Some(exp) = &v.expires_at {
            msg.push_str(&format!(" expires={exp}"));
        }
        if v.location.is_some() {
            msg.push_str(" location=available");
        }
        set_text("status", &msg);
        if let Some(loc) = &v.location {
            set_text(
                "result",
                &format!(
                    "Shared view: {} ({:.5}, {:.5}) precision={} updated={}",
                    loc.user_name, loc.latitude, loc.longitude, loc.precision_level, loc.updated_at
                ),
            );
        } else {
            set_text("result", "Token resolved (no location payload in response; adapter mapping may need update for this backend).");
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn query_param(key: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(key)
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
fn set_input_value(doc: &web_sys::Document, id: &str, value: &str) {
    if let Some(input) = doc
        .get_element_by_id(id)
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        input.set_value(value);
    }
}

#[cfg(target_arch = "wasm32")]
fn get_input_value(doc: &web_sys::Document, id: &str) -> Option<String> {
    doc.get_element_by_id(id)
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
}

#[cfg(target_arch = "wasm32")]
fn console_error_panic_hook_like(info: &std::panic::PanicHookInfo) {
    set_text("status", &format!("panic: {info}"));
}

// Keep JSON exports available for host integration/testing.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn webk_app_info() -> String {
    let cfg = WebAppConfig::new(WebAppKind::Kiosk);
    let app = KioskApp::new(cfg);
    app.snapshot_json()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn webk_validate_token(token: &str) -> String {
    match KioskApp::validate_token(token) {
        Ok(()) => serde_json::json!({"ok": true}).to_string(),
        Err(e) => serde_json::json!({"ok": false, "error": e}).to_string(),
    }
}

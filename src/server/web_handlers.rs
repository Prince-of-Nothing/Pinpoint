//! Static web UI (single-page app) served by the combined API server.
//!
//! The UI is plain HTML/CSS/JS (no third-party scripts, no build step) and
//! talks to the same `/api/v1` contract as the desktop/mobile clients with a
//! session cookie + `X-CSRF-Token`. CSP keeps it same-origin and tracker-free.

use axum::extract::Path;
use axum::http::{header, HeaderValue};
use axum::response::{Html, IntoResponse, Response};

const INDEX: &str = include_str!("../../web/app/index.html");
const PRIVACY: &str = include_str!("../../web/privacy.html");
const APP_JS: &str = include_str!("../../web/app/app.js");
const MAP_JS: &str = include_str!("../../web/app/map.js");
const NAV_JS: &str = include_str!("../../web/app/nav.js");
const SHARING_JS: &str = include_str!("../../web/app/sharing.js");
const APPEARANCE_JS: &str = include_str!("../../web/app/appearance.js");
const ADMIN_JS: &str = include_str!("../../web/app/admin.js");
const ADMIN_OVERVIEW_JS: &str = include_str!("../../web/app/admin/overview.js");
const ADMIN_USERS_JS: &str = include_str!("../../web/app/admin/users.js");
const ADMIN_DIAGNOSTICS_JS: &str = include_str!("../../web/app/admin/diagnostics.js");
const ADMIN_AUDIT_JS: &str = include_str!("../../web/app/admin/audit.js");
const APP_CSS: &str = include_str!("../../web/app/app.css");
pub(crate) const STATUS_CSS: &str = include_str!("../../web/status.css");
const FAVICON: &str = include_str!("../../web/app/favicon.svg");
const LEAFLET_JS: &str = include_str!("../../web/app/vendor/leaflet.js");
const LEAFLET_CSS: &str = include_str!("../../web/app/vendor/leaflet.css");
const LEAFLET_LICENSE: &str = include_str!("../../web/app/vendor/LICENSE");
const LEAFLET_LAYERS: &[u8] = include_bytes!("../../web/app/vendor/images/layers.png");
const LEAFLET_LAYERS_2X: &[u8] = include_bytes!("../../web/app/vendor/images/layers-2x.png");
const LEAFLET_MARKER: &[u8] = include_bytes!("../../web/app/vendor/images/marker-icon.png");
const LEAFLET_MARKER_2X: &[u8] = include_bytes!("../../web/app/vendor/images/marker-icon-2x.png");
const LEAFLET_SHADOW: &[u8] = include_bytes!("../../web/app/vendor/images/marker-shadow.png");

// Same-origin only: no Google Fonts, no CDN scripts, no third-party pixels.
// `font-src` is explicit even though the UI uses system fonts, so a future
// `@font-face` cannot silently pull a remote file.
const CSP: &str = "default-src 'self'; img-src 'self' data:; script-src 'self'; \
style-src 'self'; font-src 'self'; connect-src 'self'; base-uri 'none'; \
frame-ancestors 'none'; form-action 'self'; object-src 'none'; manifest-src 'self'";

const PERMISSIONS_POLICY: &str = "geolocation=(self), camera=(), microphone=(), payment=(), usb=()";

pub(crate) fn secure(mut resp: Response) -> Response {
    let headers = resp.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("require-corp"),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("same-origin"),
    );
    // Static modules may be revalidated; HTML overrides this with no-store.
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    resp
}

pub async fn index() -> Response {
    let mut resp = secure(Html(INDEX).into_response());
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

/// Public privacy policy. Same-origin, no scripts, no third-party assets.
pub async fn privacy() -> Response {
    let mut resp = secure(Html(PRIVACY).into_response());
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

pub async fn app_js() -> Response {
    let mut resp = APP_JS.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    secure(resp)
}

/// ES module imported by `/app.js` (same-origin; CSP allows it).
pub async fn map_js() -> Response {
    let mut resp = MAP_JS.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    secure(resp)
}

pub async fn nav_js() -> Response {
    let mut resp = NAV_JS.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    secure(resp)
}

pub async fn sharing_js() -> Response {
    module_js(SHARING_JS)
}

pub async fn appearance_js() -> Response {
    module_js(APPEARANCE_JS)
}

fn module_js(content: &'static str) -> Response {
    let mut resp = content.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    secure(resp)
}

pub async fn admin_js() -> Response {
    module_js(ADMIN_JS)
}

pub async fn admin_overview_js() -> Response {
    module_js(ADMIN_OVERVIEW_JS)
}

pub async fn admin_users_js() -> Response {
    module_js(ADMIN_USERS_JS)
}

pub async fn admin_diagnostics_js() -> Response {
    module_js(ADMIN_DIAGNOSTICS_JS)
}

pub async fn admin_audit_js() -> Response {
    module_js(ADMIN_AUDIT_JS)
}

pub async fn app_css() -> Response {
    let mut resp = APP_CSS.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/css; charset=utf-8"),
    );
    secure(resp)
}

/// Locally hosted icon (no favicon CDN, no network request beyond this app).
pub async fn favicon() -> Response {
    let mut resp = FAVICON.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("image/svg+xml"),
    );
    let mut resp = secure(resp);
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    resp
}

fn cached(mut resp: Response, mime: &'static str) -> Response {
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    let mut resp = secure(resp);
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    resp
}

pub async fn leaflet_js() -> Response {
    cached(LEAFLET_JS.into_response(), "text/javascript; charset=utf-8")
}

pub async fn leaflet_css() -> Response {
    cached(LEAFLET_CSS.into_response(), "text/css; charset=utf-8")
}

pub async fn leaflet_license() -> Response {
    cached(LEAFLET_LICENSE.into_response(), "text/plain; charset=utf-8")
}

/// Only the five vendored Leaflet sprites are served; anything else is 404.
pub async fn leaflet_image(Path(name): Path<String>) -> Response {
    let bytes: &[u8] = match name.as_str() {
        "layers.png" => LEAFLET_LAYERS,
        "layers-2x.png" => LEAFLET_LAYERS_2X,
        "marker-icon.png" => LEAFLET_MARKER,
        "marker-icon-2x.png" => LEAFLET_MARKER_2X,
        "marker-shadow.png" => LEAFLET_SHADOW,
        _ => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({"error": "not found"})),
            )
                .into_response()
        }
    };
    cached(bytes.to_vec().into_response(), "image/png")
}

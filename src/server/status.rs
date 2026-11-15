//! Read-only service status and integration discovery (issue #33).
//!
//! `GET /status` is a minimal HTML page for operators; `GET /api/v1/services`
//! is the same information as JSON. Both are read-only and intentionally free
//! of secrets: endpoints are stripped of userinfo and query strings, and no
//! account, location or friend data is read. The page works in every binary
//! (combined or split), reporting which service instance answered.

use axum::{
    extract::State,
    http::header,
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use super::state::AppState;

/// Routes mounted by every service binary and by the combined router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/status", get(html))
        .route("/status.css", get(css))
        .route("/api/v1/services", get(json_status))
}

async fn json_status(State(state): State<AppState>) -> Json<Value> {
    Json(payload(&state))
}

async fn html(State(state): State<AppState>) -> Response {
    let body = render(&payload(&state));
    let mut response = Html(body).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    super::web_handlers::secure(response)
}

async fn css() -> Response {
    let mut response = super::web_handlers::STATUS_CSS.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/css; charset=utf-8"),
    );
    super::web_handlers::secure(response)
}

/// Stable, public view of the deployment. Kept pure apart from env-configured
/// integration endpoints so it can be tested without a live server.
pub fn payload(state: &AppState) -> Value {
    let tile_template = std::env::var("PINPOINT_TILE_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/map/tiles/{z}/{x}/{y}".to_string());

    let mut integrations = json!({
        "tiles": sanitize_endpoint(&tile_template),
        "tile_proxy": "/map/tiles/{z}/{x}/{y}",
        "nominatim": sanitize_endpoint(&crate::osm::nominatim_base()),
        "osrm": sanitize_endpoint(&crate::osm::osrm_base()),
        "overpass": sanitize_endpoint(&crate::osm::overpass_url()),
    });
    if let Some(url) = crate::osm::osrm_base_for(crate::osm::TravelMode::Foot) {
        integrations["osrm_foot"] = json!(sanitize_endpoint(&url));
    }
    if let Some(url) = crate::osm::osrm_base_for(crate::osm::TravelMode::Bike) {
        integrations["osrm_bike"] = json!(sanitize_endpoint(&url));
    }

    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "service": state.owner.label(),
        "database": if state.pg.is_some() { "postgres" } else { "in-memory" },
        "components": {
            "auth": "/api/v1/auth",
            "users": "/api/v1/users",
            "friendships": "/api/v1/friendships",
            "locations": "/api/v1/locations",
            "map": "/map",
            "diagnostics": "/api/v1/diagnostics",
            "admin": "/api/v1/admin"
        },
        "integrations": integrations,
        "endpoints": {
            "health": "/health",
            "sync_status": "/api/v1/sync-status",
            "services": "/api/v1/services",
            "status_page": "/status"
        },
        "attribution": "© OpenStreetMap contributors (ODbL)"
    })
}

/// Remove credentials and query/fragment data from an endpoint. Keeps the
/// scheme, host, port and path so operators can see the current domain without
/// leaking tokens or userinfo.
pub fn sanitize_endpoint(raw: &str) -> String {
    let mut out = raw.trim().to_string();
    if let Some(scheme_end) = out.find("://") {
        let authority_start = scheme_end + 3;
        let authority_end = out[authority_start..]
            .find(['/', '?', '#'])
            .map(|i| authority_start + i)
            .unwrap_or(out.len());
        if let Some(at) = out[authority_start..authority_end].rfind('@') {
            out.replace_range(authority_start..authority_start + at + 1, "");
        }
    }
    if let Some(cut) = out.find(['?', '#']) {
        out.truncate(cut);
    }
    out
}

fn html_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn render(value: &Value) -> String {
    let mut rows = String::new();
    if let Some(components) = value.get("components").and_then(Value::as_object) {
        for (name, path) in components {
            rows.push_str(&format!(
                "<tr><th>{}</th><td><code>{}</code></td></tr>",
                html_escape(name),
                html_escape(path.as_str().unwrap_or(""))
            ));
        }
    }
    let mut integrations = String::new();
    if let Some(items) = value.get("integrations").and_then(Value::as_object) {
        for (name, endpoint) in items {
            integrations.push_str(&format!(
                "<tr><th>{}</th><td><code>{}</code></td></tr>",
                html_escape(name),
                html_escape(endpoint.as_str().unwrap_or(""))
            ));
        }
    }
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let service = value
        .get("service")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let database = value
        .get("database")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let attribution = value
        .get("attribution")
        .and_then(Value::as_str)
        .unwrap_or("");

    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Pinpoint services</title><link rel=\"stylesheet\" href=\"/status.css\">\
         </head><body><main>\
         <h1>Pinpoint services</h1>\
         <p class=\"muted\">Version {version} · service <strong>{service}</strong> · \
         database <strong>{database}</strong></p>\
         <h2>Components</h2><table>{rows}</table>\
         <h2>Integrations</h2><table>{integrations}</table>\
         <p class=\"muted\">Read-only. No account, location or friend data is read. \
         Endpoints omit credentials and query strings. {attribution}</p>\
         <p class=\"muted\"><a href=\"/health\">/health</a> · \
         <a href=\"/api/v1/services\">/api/v1/services</a> · \
         <a href=\"/api/v1/sync-status\">/api/v1/sync-status</a></p>\
         </main></body></html>",
        version = html_escape(version),
        service = html_escape(service),
        database = html_escape(database),
        rows = rows,
        integrations = integrations,
        attribution = html_escape(attribution),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::state::ServiceKind;

    #[test]
    fn strips_credentials_and_queries() {
        assert_eq!(
            sanitize_endpoint("https://user:pass@tiles.example.com/{z}/{x}/{y}.png?token=secret"),
            "https://tiles.example.com/{z}/{x}/{y}.png"
        );
        assert_eq!(
            sanitize_endpoint("https://example.com/path#fragment"),
            "https://example.com/path"
        );
        assert_eq!(
            sanitize_endpoint("https://example.com/path"),
            "https://example.com/path"
        );
        assert_eq!(sanitize_endpoint("  /relative?q=1  "), "/relative");
    }

    #[test]
    fn payload_reports_service_database_and_components() {
        let state = AppState::for_owner(ServiceKind::Locations);
        let value = payload(&state);
        assert_eq!(value["service"], "locations");
        assert_eq!(value["database"], "in-memory");
        assert_eq!(value["components"]["locations"], "/api/v1/locations");
        assert_eq!(value["endpoints"]["health"], "/health");
        assert!(value["integrations"]["nominatim"].is_string());
        assert_eq!(value["attribution"], "© OpenStreetMap contributors (ODbL)");
        let rendered = render(&value);
        assert!(rendered.contains("Pinpoint services"));
        assert!(rendered.contains("OpenStreetMap"));
        assert!(rendered.contains("/api/v1/services"));
    }

    #[test]
    fn html_escapes_untrusted_values() {
        assert_eq!(
            html_escape("<script>alert('x')</script>"),
            "&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;"
        );
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }
}

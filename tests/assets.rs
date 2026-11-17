//! Asset-locality guard: the web UI must never pull fonts, scripts, styles or
//! images from a third-party origin, and the CSP must enforce same-origin.
//!
//! This is deliberately a source-level test: it fails the build if someone
//! pastes a Google Fonts/CDN URL into the shipped HTML/CSS/JS, before a
//! browser ever gets a chance to request it.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

const HTML: &str = include_str!("../web/app/index.html");
const CSS: &str = include_str!("../web/app/app.css");
const JS: &str = include_str!("../web/app/app.js");
const MAP_ADAPTER: &str = include_str!("../web/app/map.js");
const MAP_FOUNDATION: &str = include_str!("../web/map/map.js");
const ROUTE_UI: &str = include_str!("../web/app/route-ui.js");
const ROUTE_CSS: &str = include_str!("../web/app/route.css");

const ATTRIBUTION_URLS: &[&str] = &[
    "https://www.openstreetmap.org/copyright",
    "https://www.openmaptiles.org/",
    "https://opendatacommons.org/licenses/odbl/1-0/",
];

const BANNED_HOSTS: &[&str] = &[
    "fonts.googleapis",
    "fonts.gstatic",
    "gstatic.com",
    "unpkg.com",
    "jsdelivr.net",
    "bootstrapcdn",
    "fontawesome",
    "cdnjs.",
    "google-analytics",
    "googletagmanager",
];

#[test]
fn shipped_web_assets_have_no_third_party_hosts() {
    for (name, text) in [
        ("index.html", HTML),
        ("app.css", CSS),
        ("app.js", JS),
        ("map.js (adapter)", MAP_ADAPTER),
        ("map/map.js (foundation)", MAP_FOUNDATION),
        ("route-ui.js", ROUTE_UI),
        ("route.css", ROUTE_CSS),
    ] {
        let lower = text.to_ascii_lowercase();
        for host in BANNED_HOSTS {
            assert!(
                !lower.contains(host),
                "{name} references third-party host `{host}` — assets must be local"
            );
        }
    }
}

#[test]
fn css_and_js_make_no_absolute_requests() {
    assert!(!CSS.contains("http://") && !CSS.contains("https://"));
    assert!(!CSS.contains("@import"), "no imported stylesheets");
    assert!(!CSS.contains("url(http"), "no remote url() assets");
    assert!(!JS.contains("http://") && !JS.contains("https://"));
    assert!(!JS.contains("fetch(\"http"), "all fetches stay same-origin");
    assert!(!JS.contains("fetch('http"));
    // The adapter and the shared foundation only use root-relative paths.
    for (name, text) in [("map.js", MAP_ADAPTER), ("map/map.js", MAP_FOUNDATION)] {
        assert!(
            !text.contains("http://") && !text.contains("https://"),
            "{name} must not reference a remote origin"
        );
    }
}

#[test]
fn route_ui_assets_are_local_and_network_free() {
    // The map page loads these two files from the loopback map server; the
    // component takes an injected router and must never fetch by itself.
    for (needle, label) in [
        ("fetch(", "fetch"),
        ("XMLHttpRequest", "XMLHttpRequest"),
        ("WebSocket", "WebSocket"),
        ("EventSource", "EventSource"),
        ("http://", "absolute http URL"),
        ("https://", "absolute https URL"),
    ] {
        assert!(
            !ROUTE_UI.contains(needle),
            "route-ui.js must not use {label}"
        );
    }
    assert!(ROUTE_UI.contains("export function createRouteUI"));
    assert!(ROUTE_CSS.contains(".pinpoint-route"));
    assert!(!ROUTE_CSS.contains("@import"));
    assert!(!ROUTE_CSS.contains("url("));
    assert!(!ROUTE_CSS.contains("http"));
}

#[test]
fn html_only_links_out_for_attribution() {
    // Subresources (script/link/img src|href) must be relative.
    for line in HTML.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<script")
            || trimmed.starts_with("<link")
            || trimmed.starts_with("<img")
        {
            assert!(
                !trimmed.contains("http://") && !trimmed.contains("https://"),
                "subresource must be local: {trimmed}"
            );
        }
    }

    // Absolute URLs are allowed only for the mandatory OSM/OpenMapTiles/ODbL
    // attribution links and the opt-in OpenFreeMap style URL. The style URL
    // lives in a meta tag, never in a subresource, so the offline page stays
    // same-origin unless the user turns the online basemap on.
    const ONLINE_STYLE_URL: &str = "https://tiles.openfreemap.org/styles/liberty";
    let mut rest = HTML;
    while let Some(index) = rest.find("https://") {
        let tail = &rest[index..];
        let end = tail.find(['"', '\'', ' ', '<', '>']).unwrap_or(tail.len());
        let url = &tail[..end];
        assert!(
            ATTRIBUTION_URLS.contains(&url) || url == ONLINE_STYLE_URL,
            "unexpected absolute URL in index.html: {url}"
        );
        rest = &tail[end..];
    }
    assert!(
        HTML.contains(&format!(
            "<meta name=\"pinpoint-online-style\" content=\"{ONLINE_STYLE_URL}\""
        )),
        "index.html must keep the online style URL page-owned"
    );
    for url in ATTRIBUTION_URLS {
        assert!(
            HTML.contains(url),
            "index.html must show the attribution link {url}"
        );
    }
}

#[tokio::test]
async fn csp_is_same_origin_and_blocks_remote_fonts() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let csp = resp
        .headers()
        .get("content-security-policy")
        .expect("CSP header")
        .to_str()
        .unwrap();
    assert!(csp.contains("default-src 'self'"), "CSP: {csp}");
    assert!(csp.contains("font-src 'self'"), "CSP must pin fonts: {csp}");
    assert!(csp.contains("script-src 'self'"), "CSP: {csp}");
    assert!(
        csp.contains("worker-src 'self'"),
        "CSP must allow the local worker: {csp}"
    );
    assert!(
        csp.contains("connect-src 'self' https://tiles.openfreemap.org"),
        "CSP must allow only the opt-in OpenFreeMap host for tiles: {csp}"
    );
    assert!(
        csp.contains("img-src 'self' data: https://tiles.openfreemap.org"),
        "CSP must allow only the opt-in OpenFreeMap host for images: {csp}"
    );
    assert!(
        !csp.contains('*') && !csp.contains("http://"),
        "CSP must not allow non-https or wildcard origins: {csp}"
    );
    assert_eq!(
        csp.matches("https://tiles.openfreemap.org").count(),
        2,
        "the only remote origin is the opt-in OpenFreeMap host: {csp}"
    );

    let headers = resp.headers();
    assert_eq!(
        headers
            .get("cross-origin-embedder-policy")
            .and_then(|v| v.to_str().ok()),
        Some("require-corp")
    );
    assert_eq!(
        headers
            .get("cross-origin-opener-policy")
            .and_then(|v| v.to_str().ok()),
        Some("same-origin")
    );
    assert_eq!(
        headers
            .get("cross-origin-resource-policy")
            .and_then(|v| v.to_str().ok()),
        Some("same-origin")
    );
    let permissions = headers
        .get("permissions-policy")
        .and_then(|v| v.to_str().ok())
        .expect("permissions policy");
    assert!(permissions.contains("geolocation=(self)"));
    assert!(permissions.contains("camera=()"));
    assert_eq!(
        headers.get("cache-control").and_then(|v| v.to_str().ok()),
        Some("no-store"),
        "HTML must not be cached"
    );
}

#[tokio::test]
async fn favicon_is_served_locally() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    for path in ["/favicon.svg", "/favicon.ico"] {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        assert_eq!(
            resp.headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("image/svg+xml"),
            "{path}"
        );
    }
}

#[tokio::test]
async fn admin_assets_are_served() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    for path in [
        "/admin.js",
        "/admin/overview.js",
        "/admin/users.js",
        "/admin/diagnostics.js",
        "/admin/audit.js",
        "/sharing.js",
        "/appearance.js",
    ] {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
    }
}

#[tokio::test]
async fn maplibre_and_pmtiles_are_vendored_and_served() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let css = std::fs::read_to_string(root.join("web/vendor/maplibre-gl.css"))
        .expect("web/vendor/maplibre-gl.css must be vendored");
    let css_lower = css.to_ascii_lowercase();
    for host in ["cdn.", "unpkg.com", "jsdelivr.net"] {
        assert!(
            !css_lower.contains(host),
            "vendored MapLibre CSS must not reference {host}"
        );
    }
    assert!(
        !css_lower.contains("url(http"),
        "vendored MapLibre CSS must not load remote url() assets"
    );

    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    let assets = [
        ("/map-assets/map.js", "text/javascript"),
        ("/map-assets/style.json", "application/json"),
        ("/map-assets/README.md", "text/plain"),
        ("/map-assets/fixtures/README.md", "text/plain"),
        (
            "/map-assets/fixtures/monaco.pmtiles",
            "application/octet-stream",
        ),
        ("/vendor/maplibre-gl.js", "text/javascript"),
        ("/vendor/maplibre-gl-worker.mjs", "text/javascript"),
        ("/vendor/maplibre-gl.css", "text/css"),
        ("/vendor/maplibre-gl.LICENSE.txt", "text/plain"),
        ("/vendor/pmtiles.js", "text/javascript"),
        ("/vendor/pmtiles.LICENSE.txt", "text/plain"),
    ];
    for (path, mime) in assets {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.starts_with(mime),
            "{path} content type: {content_type}"
        );
        let body = to_bytes(resp.into_body(), 4 * 1024 * 1024)
            .await
            .expect("asset body");
        assert!(!body.is_empty(), "{path} must not be empty");
    }

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/map-assets/style.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let style = String::from_utf8(body.to_vec()).unwrap();
    assert!(style.contains("OpenStreetMap contributors"));
    assert!(style.contains("OpenMapTiles"));
    assert!(style.contains("ODbL"));
}

#[tokio::test]
async fn map_fixture_supports_byte_ranges() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());

    let full = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/map-assets/fixtures/monaco.pmtiles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(full.status(), StatusCode::OK);
    assert_eq!(
        full.headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok()),
        Some("bytes")
    );
    let full_body = to_bytes(full.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(full_body.len(), 94389);
    assert_eq!(&full_body[..7], b"PMTiles");

    let partial = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/map-assets/fixtures/monaco.pmtiles")
                .header("range", "bytes=0-126")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        partial
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok()),
        Some("bytes 0-126/94389")
    );
    assert_eq!(
        partial
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok()),
        Some("127")
    );
    let partial_body = to_bytes(partial.into_body(), 4096).await.unwrap();
    assert_eq!(partial_body.len(), 127);
    assert_eq!(&partial_body[..7], b"PMTiles");

    let invalid = app
        .oneshot(
            Request::builder()
                .uri("/map-assets/fixtures/monaco.pmtiles")
                .header("range", "bytes=999999-")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        invalid
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok()),
        Some("bytes */94389")
    );
}

#[tokio::test]
async fn offline_app_assets_are_served() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    let assets = [
        ("/offline-search.js", "text/javascript"),
        ("/offline.js", "text/javascript"),
        ("/offline-catalog.js", "text/javascript"),
        ("/sw.js", "text/javascript"),
        ("/map-assets/search.js", "text/javascript"),
        ("/vendor/sqlite3.mjs", "text/javascript"),
        ("/vendor/sqlite3.wasm", "application/wasm"),
        ("/app/offline-demo.html", "text/html"),
        ("/app/offline-demo.js", "text/javascript"),
        ("/app/offline.css", "text/css"),
        ("/app/offline.js", "text/javascript"),
        ("/app/offline-catalog.js", "text/javascript"),
        (
            "/app/fixtures/offline-pack/manifest.json",
            "application/json",
        ),
        ("/map/map.js", "text/javascript"),
        ("/map/style.json", "application/json"),
    ];
    for (path, mime) in assets {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.starts_with(mime),
            "{path} content type: {content_type}"
        );
        let body = to_bytes(resp.into_body(), 8 * 1024 * 1024)
            .await
            .expect("asset body");
        assert!(!body.is_empty(), "{path} must not be empty");
    }

    let sw = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/sw.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        sw.headers()
            .get("service-worker-allowed")
            .and_then(|v| v.to_str().ok()),
        Some("/"),
        "the service worker must control the pack routes"
    );

    let range = app
        .oneshot(
            Request::builder()
                .uri("/map/fixtures/monaco.pmtiles")
                .header("range", "bytes=0-6")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(range.status(), StatusCode::PARTIAL_CONTENT);
    let body = to_bytes(range.into_body(), 16).await.unwrap();
    assert_eq!(&body[..], b"PMTiles");
}

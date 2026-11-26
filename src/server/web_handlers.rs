//! Static web UI (single-page app) served by the combined API server.
//!
//! The UI is plain HTML/CSS/JS (no third-party scripts, no build step) and
//! talks to the same `/api/v1` contract as the desktop/mobile clients with a
//! session cookie + `X-CSRF-Token`. CSP keeps it same-origin and tracker-free.
//!
//! The map uses the shared MapLibre GL JS + PMTiles foundation from `web/map/`
//! and the vendored libraries from `web/vendor/`. PMTiles reads tiles with
//! HTTP Range requests, so the fixture route implements byte serving.

use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};

const INDEX: &str = include_str!("../../web/app/index.html");
const PRIVACY: &str = include_str!("../../web/privacy.html");
const APP_JS: &str = include_str!("../../web/app/app.js");
const MESSAGE_CRYPTO_JS: &str = include_str!("../../web/app/message-crypto.js");
const MAP_JS: &str = include_str!("../../web/app/map.js");
const MEASURE_JS: &str = include_str!("../../web/app/measure.js");
const SERVER_ADDRESS_JS: &str = include_str!("../../web/app/server-address.js");
const SERVICES_JS: &str = include_str!("../../web/app/services.js");
const MAP_LAYERS_JS: &str = include_str!("../../web/app/map-layers.js");
const NAV_JS: &str = include_str!("../../web/app/nav.js");
const DRAWER_DISMISS_JS: &str = include_str!("../../web/app/drawer-dismiss.js");
const SEARCH_SHORTCUT_JS: &str = include_str!("../../web/app/search-shortcut.js");
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
const MANIFEST: &str = include_str!("../../web/app/manifest.webmanifest");

// Shared map foundation (web/map) and vendored browser libraries (web/vendor).
// Everything is compiled into the binary: no runtime file lookup, no CDN.
const MAP_FOUNDATION_JS: &str = include_str!("../../web/map/map.js");
const MAP_STYLE_JSON: &str = include_str!("../../web/map/style.json");
const MAP_README: &str = include_str!("../../web/map/README.md");
const MAP_FIXTURE_README: &str = include_str!("../../web/map/fixtures/README.md");
const MAP_FIXTURE: &[u8] = include_bytes!("../../web/map/fixtures/monaco.pmtiles");
const MAPLIBRE_JS: &str = include_str!("../../web/vendor/maplibre-gl.js");
const MAPLIBRE_WORKER: &str = include_str!("../../web/vendor/maplibre-gl-worker.mjs");
const MAPLIBRE_CSS: &str = include_str!("../../web/vendor/maplibre-gl.css");
const MAPLIBRE_LICENSE: &str = include_str!("../../web/vendor/maplibre-gl.LICENSE.txt");
const PMTILES_JS: &str = include_str!("../../web/vendor/pmtiles.js");
const PMTILES_LICENSE: &str = include_str!("../../web/vendor/pmtiles.LICENSE.txt");

// Offline pack manager, offline search and the service worker. The app imports
// these by absolute path; they must be compiled in like the rest of the UI.
const OFFLINE_SEARCH_JS: &str = include_str!("../../web/app/offline-search.js");
const OFFLINE_JS: &str = include_str!("../../web/app/offline.js");
const OFFLINE_CATALOG_JS: &str = include_str!("../../web/app/offline-catalog.js");
const OFFLINE_DEMO: &str = include_str!("../../web/app/offline-demo.html");
const OFFLINE_DEMO_JS: &str = include_str!("../../web/app/offline-demo.js");
const OFFLINE_CSS: &str = include_str!("../../web/app/offline.css");
const OFFLINE_MANIFEST: &str = include_str!("../../web/app/fixtures/offline-pack/manifest.json");
const OFFLINE_MANIFEST_BROKEN: &str =
    include_str!("../../web/app/fixtures/offline-pack/manifest-broken.json");
const OFFLINE_LICENSE: &str = include_str!("../../web/app/fixtures/offline-pack/LICENSE.txt");
const SEARCH_JS: &str = include_str!("../../web/map/search.js");
const SQLITE_MODULE: &str = include_str!("../../web/vendor/sqlite3.mjs");
const SQLITE_WASM: &[u8] = include_bytes!("../../web/vendor/sqlite3.wasm");
const SW_JS: &str = include_str!("../../web/sw.js");

// Same-origin only: no Google Fonts, no CDN scripts, no third-party pixels.
// `font-src` is explicit even though the UI uses system fonts, so a future
// `@font-face` cannot silently pull a remote file. MapLibre's module worker is
// same-origin, and PMTiles range requests stay under `connect-src 'self'`.
// MapLibre positions its canvas and controls through the CSSOM
// (`element.style.*`), which CSP does not treat as inline CSS; the app's own
// styles live in same-origin stylesheets. No markup uses `style="..."`, so
// `style-src 'self'` is enough. Scripts stay strict: only same-origin modules
// and the SQLite WASM evaluator are allowed.
//
// Remote hosts are opt-in map layers only, all off by default (ADR-0021,
// ADR-0026): the OpenFreeMap basemap (style, tiles, glyphs and sprites on
// `tiles.openfreemap.org`), EOX Sentinel-2 satellite imagery
// (`tiles.maps.eox.at`) and the Terrain Tiles DEM on AWS
// (`elevation-tiles-prod.s3.amazonaws.com`). Each is reachable only while the
// user has that layer on, and only for tiles and images.
// The vendored MapLibre worker is same-origin, so `worker-src` stays `'self'`
// (`docs/offline/android-webview-online.md` §3).
const CSP: &str = "default-src 'self'; \
img-src 'self' data: https://tiles.openfreemap.org https://tiles.maps.eox.at \
https://elevation-tiles-prod.s3.amazonaws.com; \
script-src 'self' 'wasm-unsafe-eval'; \
worker-src 'self'; style-src 'self'; font-src 'self'; \
connect-src 'self' https://tiles.openfreemap.org https://tiles.maps.eox.at \
https://elevation-tiles-prod.s3.amazonaws.com; base-uri 'none'; \
frame-ancestors 'none'; form-action 'self'; object-src 'none'; \
manifest-src 'self'";

/// Remote map hosts the CSP allows, each behind an opt-in layer.
#[cfg(test)]
const OPT_IN_MAP_HOSTS: [&str; 3] = [
    "https://tiles.openfreemap.org",
    "https://tiles.maps.eox.at",
    "https://elevation-tiles-prod.s3.amazonaws.com",
];

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
pub async fn message_crypto_js() -> Response {
    let mut resp = MESSAGE_CRYPTO_JS.into_response();
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

pub async fn drawer_dismiss_js() -> Response {
    let mut resp = DRAWER_DISMISS_JS.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    secure(resp)
}

pub async fn search_shortcut_js() -> Response {
    let mut resp = SEARCH_SHORTCUT_JS.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/javascript; charset=utf-8"),
    );
    secure(resp)
}

pub async fn measure_js() -> Response {
    module_js(MEASURE_JS)
}

pub async fn server_address_js() -> Response {
    module_js(SERVER_ADDRESS_JS)
}

pub async fn services_js() -> Response {
    module_js(SERVICES_JS)
}

pub async fn map_layers_js() -> Response {
    module_js(MAP_LAYERS_JS)
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

/// Same-origin app manifest for home-screen installs.
pub async fn manifest() -> Response {
    let mut resp = MANIFEST.into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/manifest+json"),
    );
    let mut resp = secure(resp);
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
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

// ---------------------------------------------------------------------------
// Shared map foundation + vendored MapLibre/PMTiles (all same-origin)
// ---------------------------------------------------------------------------

pub async fn map_foundation_js() -> Response {
    module_js(MAP_FOUNDATION_JS)
}

pub async fn map_style_json() -> Response {
    cached(MAP_STYLE_JSON.into_response(), "application/json")
}

pub async fn map_readme() -> Response {
    cached(MAP_README.into_response(), "text/plain; charset=utf-8")
}

pub async fn map_fixture_readme() -> Response {
    cached(
        MAP_FIXTURE_README.into_response(),
        "text/plain; charset=utf-8",
    )
}

pub async fn maplibre_js() -> Response {
    cached(
        MAPLIBRE_JS.into_response(),
        "text/javascript; charset=utf-8",
    )
}

pub async fn maplibre_worker() -> Response {
    cached(
        MAPLIBRE_WORKER.into_response(),
        "text/javascript; charset=utf-8",
    )
}

pub async fn maplibre_css() -> Response {
    cached(MAPLIBRE_CSS.into_response(), "text/css; charset=utf-8")
}

pub async fn maplibre_license() -> Response {
    cached(
        MAPLIBRE_LICENSE.into_response(),
        "text/plain; charset=utf-8",
    )
}

pub async fn pmtiles_js() -> Response {
    cached(PMTILES_JS.into_response(), "text/javascript; charset=utf-8")
}

pub async fn pmtiles_license() -> Response {
    cached(PMTILES_LICENSE.into_response(), "text/plain; charset=utf-8")
}

pub async fn offline_search_js() -> Response {
    module_js(OFFLINE_SEARCH_JS)
}

pub async fn offline_js() -> Response {
    module_js(OFFLINE_JS)
}

pub async fn offline_catalog_js() -> Response {
    module_js(OFFLINE_CATALOG_JS)
}

pub async fn search_js() -> Response {
    module_js(SEARCH_JS)
}

/// The pack-manager demo is a first-class same-origin page; the service worker
/// precaches it. It is useful for installing the Moldova pack before opening
/// the main app.
pub async fn offline_demo() -> Response {
    let mut resp = Html(OFFLINE_DEMO).into_response();
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    secure(resp)
}

pub async fn offline_demo_js() -> Response {
    module_js(OFFLINE_DEMO_JS)
}

pub async fn offline_css() -> Response {
    cached(OFFLINE_CSS.into_response(), "text/css; charset=utf-8")
}

pub async fn offline_manifest() -> Response {
    cached(OFFLINE_MANIFEST.into_response(), "application/json")
}

pub async fn offline_manifest_broken() -> Response {
    cached(OFFLINE_MANIFEST_BROKEN.into_response(), "application/json")
}

pub async fn offline_license() -> Response {
    cached(OFFLINE_LICENSE.into_response(), "text/plain; charset=utf-8")
}

pub async fn sqlite_module() -> Response {
    cached(
        SQLITE_MODULE.into_response(),
        "text/javascript; charset=utf-8",
    )
}

pub async fn sqlite_wasm() -> Response {
    cached(SQLITE_WASM.into_response(), "application/wasm")
}

/// Root service worker. `Service-Worker-Allowed: /` keeps the page's pack
/// routes (`/offline-packs/...`) in scope.
pub async fn service_worker() -> Response {
    let mut resp = module_js(SW_JS);
    resp.headers_mut().insert(
        header::HeaderName::from_static("service-worker-allowed"),
        HeaderValue::from_static("/"),
    );
    resp
}

enum ByteRange {
    None,
    Invalid,
    Satisfiable(usize, usize),
}

/// Parse a single `Range: bytes=...` header. Multipart ranges are not needed
/// by PMTiles and are treated as invalid rather than partially served.
fn parse_range(headers: &HeaderMap, len: usize) -> ByteRange {
    let Some(value) = headers.get(header::RANGE) else {
        return ByteRange::None;
    };
    let Ok(text) = value.to_str() else {
        return ByteRange::Invalid;
    };
    let Some((unit, spec)) = text.trim().split_once('=') else {
        return ByteRange::Invalid;
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") || spec.contains(',') || len == 0 {
        return ByteRange::Invalid;
    }
    let Some((start, end)) = spec.trim().split_once('-') else {
        return ByteRange::Invalid;
    };
    let start = start.trim();
    let end = end.trim();

    let (start, end) = if start.is_empty() {
        // Suffix range: the last N bytes.
        let Ok(suffix) = end.parse::<usize>() else {
            return ByteRange::Invalid;
        };
        if suffix == 0 {
            return ByteRange::Invalid;
        }
        (len.saturating_sub(suffix), len - 1)
    } else {
        let Ok(start) = start.parse::<usize>() else {
            return ByteRange::Invalid;
        };
        let end = if end.is_empty() {
            len - 1
        } else {
            match end.parse::<usize>() {
                Ok(end) => end.min(len - 1),
                Err(_) => return ByteRange::Invalid,
            }
        };
        (start, end)
    };

    if start >= len || end < start {
        return ByteRange::Invalid;
    }
    ByteRange::Satisfiable(start, end)
}

/// PMTiles fixture. Range requests return `206` so MapLibre/PMTiles can read
/// the archive header and individual tiles without downloading the whole file.
pub async fn map_fixture(headers: HeaderMap) -> Response {
    let len = MAP_FIXTURE.len();
    let range = parse_range(&headers, len);

    let mut resp = match range {
        ByteRange::None => {
            let mut resp = MAP_FIXTURE.to_vec().into_response();
            resp.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&len.to_string()).expect("fixture length"),
            );
            resp
        }
        ByteRange::Invalid => {
            let resp = (StatusCode::RANGE_NOT_SATISFIABLE, "").into_response();
            let mut resp = secure(resp);
            resp.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{len}")).expect("content range"),
            );
            resp.headers_mut()
                .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            return resp;
        }
        ByteRange::Satisfiable(start, end) => {
            let slice = MAP_FIXTURE[start..=end].to_vec();
            let mut resp = (StatusCode::PARTIAL_CONTENT, slice).into_response();
            resp.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&(end - start + 1).to_string()).expect("slice length"),
            );
            resp.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{len}"))
                    .expect("content range"),
            );
            resp
        }
    };

    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    resp.headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    let mut resp = secure(resp);
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(value: &str, len: usize) -> ByteRange {
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_str(value).unwrap());
        parse_range(&headers, len)
    }

    fn directive(name: &str) -> &'static str {
        CSP.split(';')
            .map(str::trim)
            .find(|part| part.split_whitespace().next() == Some(name))
            .unwrap_or_else(|| panic!("missing {name} directive in {CSP}"))
    }

    #[test]
    fn csp_allows_only_the_opt_in_map_hosts() {
        for name in ["connect-src", "img-src"] {
            let sources = directive(name);
            for host in OPT_IN_MAP_HOSTS {
                assert!(
                    sources.contains(host),
                    "{name} lacks the opt-in map host {host}: {sources}"
                );
            }
            assert!(
                sources.contains("'self'"),
                "{name} dropped same-origin access: {sources}"
            );
            // Nothing beyond 'self', data: images and the listed map hosts.
            for source in sources.split_whitespace().skip(1) {
                assert!(
                    source == "'self'"
                        || (name == "img-src" && source == "data:")
                        || OPT_IN_MAP_HOSTS.contains(&source),
                    "{name} allows an unlisted source {source}"
                );
            }
        }
        // Exact hosts, no wildcard subdomains, and no blob workers.
        assert!(!CSP.contains('*'), "wildcard source in {CSP}");
        assert!(!CSP.contains("blob:"), "blob source in {CSP}");
        assert_eq!(directive("worker-src"), "worker-src 'self'");
        assert_eq!(directive("font-src"), "font-src 'self'");
        // Styles are same-origin files or CSSOM assignments, never inline
        // markup; allowing 'unsafe-inline' would weaken the XSS barrier.
        assert_eq!(directive("style-src"), "style-src 'self'");
        assert!(!CSP.contains("style-src 'self' 'unsafe-inline'"));
        // Nothing that executes code or renders frames was widened.
        assert_eq!(directive("default-src"), "default-src 'self'");
        assert_eq!(
            directive("script-src"),
            "script-src 'self' 'wasm-unsafe-eval'"
        );
        assert_eq!(directive("frame-ancestors"), "frame-ancestors 'none'");
        assert_eq!(directive("object-src"), "object-src 'none'");
    }

    #[test]
    fn range_parser_handles_pmtiles_requests() {
        assert!(matches!(
            range("bytes=0-126", 1000),
            ByteRange::Satisfiable(0, 126)
        ));
        assert!(matches!(
            range("bytes=900-", 1000),
            ByteRange::Satisfiable(900, 999)
        ));
        assert!(matches!(
            range("bytes=-100", 1000),
            ByteRange::Satisfiable(900, 999)
        ));
        assert!(matches!(
            range("bytes=0-9999", 1000),
            ByteRange::Satisfiable(0, 999)
        ));
        assert!(matches!(
            parse_range(&HeaderMap::new(), 1000),
            ByteRange::None
        ));
        assert!(matches!(range("bytes=1000-", 1000), ByteRange::Invalid));
        assert!(matches!(range("bytes=0-1,5-6", 1000), ByteRange::Invalid));
        assert!(matches!(range("items=0-1", 1000), ByteRange::Invalid));
    }
}

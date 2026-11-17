//! Loopback HTTP host for the shared MapLibre map.
//!
//! Android has no `file://` origin that can load ES modules, workers and
//! PMTiles byte ranges, so the WebView page is served from an ephemeral
//! `127.0.0.1` port instead. The same server will serve downloaded PMTiles
//! packs from the app sandbox once the download manager lands.
//!
//! Security properties:
//! - binds only to `127.0.0.1` and only on an ephemeral port;
//! - embedded assets are addressed by an exact-path table (no filesystem
//!   traversal and no directory listing);
//! - disk-backed packs must be `packs/<region>/<name>` (or the legacy
//!   `packs/<name>`) and are canonicalised under the configured packs
//!   directory;
//! - no session, CSRF token or account data is ever served.
//!
//! PMTiles reads through HTTP Range requests, so every response advertises
//! `Accept-Ranges: bytes` and supports `206 Partial Content` and `416`.
//!
//! `/map/catalog`, `/map/offline/status`, `/map/offline/download` and
//! `/map/config` come from [`crate::offline_install`], the same module the
//! desktop server mounts. Downloaded artifacts land in
//! `<app data>/packs/<region>/` and are served at `/packs/<region>/<name>`
//! with the same Range semantics.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use axum::body::{Body, Bytes};
use axum::extract::{Extension, Path as UrlPath};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Router;
use tokio::sync::oneshot;

use crate::offline_install::{DownloadPlan, OfflineInstaller};

const MIME_HTML: &str = "text/html; charset=utf-8";
const MIME_CSS: &str = "text/css; charset=utf-8";
const MIME_JS: &str = "text/javascript; charset=utf-8";
const MIME_JSON: &str = "application/json; charset=utf-8";
const MIME_PMTILES: &str = "application/octet-stream";
const MIME_TEXT: &str = "text/plain; charset=utf-8";

const MAP_INDEX: &[u8] = include_bytes!("../../web/map/index.html");
const MAP_PREVIEW_CSS: &[u8] = include_bytes!("../../web/map/preview.css");
const MAP_JS: &[u8] = include_bytes!("../../web/map/map.js");
const MAP_STYLE: &[u8] = include_bytes!("../../web/map/style.json");
/// Route panel component shared with the web app (no imports, no network).
const ROUTE_UI_JS: &[u8] = include_bytes!("../../web/app/route-ui.js");
const ROUTE_UI_CSS: &[u8] = include_bytes!("../../web/app/route.css");
/// The committed ODbL Monaco fixture. The same file is loaded from disk for
/// downloaded packs; this copy keeps the prototype self-contained in the APK.
const MONACO_PMTILES: &[u8] = include_bytes!("../../web/map/fixtures/monaco.pmtiles");
const MAPLIBRE_JS: &[u8] = include_bytes!("../../web/vendor/maplibre-gl.js");
const MAPLIBRE_WORKER: &[u8] = include_bytes!("../../web/vendor/maplibre-gl-worker.mjs");
const MAPLIBRE_CSS: &[u8] = include_bytes!("../../web/vendor/maplibre-gl.css");
const PMTILES_JS: &[u8] = include_bytes!("../../web/vendor/pmtiles.js");

struct EmbeddedAsset {
    path: &'static str,
    mime: &'static str,
    bytes: &'static [u8],
}

/// Exact-path table for everything `web/map/index.html` loads. Adding a file
/// to the page means adding it here, which keeps the HTTP surface closed.
const ASSETS: &[EmbeddedAsset] = &[
    EmbeddedAsset {
        path: "map/index.html",
        mime: MIME_HTML,
        bytes: MAP_INDEX,
    },
    EmbeddedAsset {
        path: "map/preview.css",
        mime: MIME_CSS,
        bytes: MAP_PREVIEW_CSS,
    },
    EmbeddedAsset {
        path: "map/map.js",
        mime: MIME_JS,
        bytes: MAP_JS,
    },
    EmbeddedAsset {
        path: "map/style.json",
        mime: MIME_JSON,
        bytes: MAP_STYLE,
    },
    EmbeddedAsset {
        path: "map/fixtures/monaco.pmtiles",
        mime: MIME_PMTILES,
        bytes: MONACO_PMTILES,
    },
    EmbeddedAsset {
        path: "app/route-ui.js",
        mime: MIME_JS,
        bytes: ROUTE_UI_JS,
    },
    EmbeddedAsset {
        path: "app/route.css",
        mime: MIME_CSS,
        bytes: ROUTE_UI_CSS,
    },
    EmbeddedAsset {
        path: "vendor/maplibre-gl.js",
        mime: MIME_JS,
        bytes: MAPLIBRE_JS,
    },
    EmbeddedAsset {
        path: "vendor/maplibre-gl-worker.mjs",
        mime: MIME_JS,
        bytes: MAPLIBRE_WORKER,
    },
    EmbeddedAsset {
        path: "vendor/maplibre-gl.css",
        mime: MIME_CSS,
        bytes: MAPLIBRE_CSS,
    },
    EmbeddedAsset {
        path: "vendor/pmtiles.js",
        mime: MIME_JS,
        bytes: PMTILES_JS,
    },
];

/// A running loopback map server. Dropping it shuts the thread down.
pub struct MapServer {
    origin: String,
    port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MapServer {
    /// Bind `127.0.0.1:0` and serve the embedded map assets plus, optionally,
    /// installed `packs/<region>/<name>` files. Production uses the region
    /// catalog; [`Self::start_with_plan`] keeps the legacy single-pack plan for
    /// fixture tests.
    pub fn start(packs_dir: Option<PathBuf>) -> std::io::Result<Self> {
        Self::start_with(packs_dir, OfflineInstaller::new)
    }

    /// Fixture/test variant with explicit artifact metadata.
    pub fn start_with_plan(
        packs_dir: Option<PathBuf>,
        plan: DownloadPlan,
    ) -> std::io::Result<Self> {
        Self::start_with(packs_dir, move |packs| {
            OfflineInstaller::with_plan(packs, plan)
        })
    }

    fn start_with(
        packs_dir: Option<PathBuf>,
        make_installer: impl FnOnce(Option<PathBuf>) -> OfflineInstaller + Send + 'static,
    ) -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let local_router = Arc::new(crate::local_route::LocalRouter::for_packs_dir(
            packs_dir.as_deref(),
        ));
        let installer = Arc::new(make_installer(packs_dir));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let thread = std::thread::Builder::new()
            .name("pinpoint-map-server".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        tracing::error!(%error, "map server: could not build a runtime");
                        return;
                    }
                };
                runtime.block_on(async move {
                    let listener = match tokio::net::TcpListener::from_std(listener) {
                        Ok(listener) => listener,
                        Err(error) => {
                            tracing::error!(%error, "map server: listener hand-off failed");
                            return;
                        }
                    };
                    let app = router(installer, local_router);
                    let served = axum::serve(listener, app)
                        .with_graceful_shutdown(async move {
                            let _ = shutdown_rx.await;
                        })
                        .await;
                    if let Err(error) = served {
                        tracing::warn!(%error, "map server stopped");
                    }
                });
            })?;

        tracing::info!(port = address.port(), "map server listening on loopback");
        Ok(Self {
            origin: format!("http://{address}"),
            port: address.port(),
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        })
    }

    /// Origin without a trailing slash, for example `http://127.0.0.1:41234`.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The MapLibre preview page served by this instance.
    pub fn map_url(&self) -> String {
        format!("{}/map/index.html", self.origin)
    }
}

impl Drop for MapServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Process-wide server used by the Android WebView. The first map open starts
/// it; it stays alive while the app process lives so reopening is instant.
pub fn shared() -> Result<&'static MapServer, String> {
    static SHARED: OnceLock<MapServer> = OnceLock::new();
    if let Some(server) = SHARED.get() {
        return Ok(server);
    }
    let packs = crate::platform::app_data_dir().map(|dir| dir.join("packs"));
    let server = MapServer::start(packs)
        .map_err(|error| format!("could not start the loopback map server: {error}"))?;
    if let Err(duplicate) = SHARED.set(server) {
        // Another thread won the race; drop the duplicate binding.
        drop(duplicate);
    }
    SHARED
        .get()
        .ok_or_else(|| "the loopback map server did not start".to_string())
}

fn router(
    installer: Arc<OfflineInstaller>,
    local_router: Arc<crate::local_route::LocalRouter>,
) -> Router {
    Router::new()
        .route("/map/route", get(crate::local_route::map_route))
        .route("/map/catalog", get(crate::offline_install::catalog))
        .route("/map/config", get(crate::offline_install::config))
        .route("/map/offline/status", get(crate::offline_install::status))
        .route(
            "/map/offline/download",
            post(crate::offline_install::download),
        )
        .route(
            "/packs/:region/:name",
            get(crate::offline_install::serve_region_pack),
        )
        .route("/packs/:name", get(crate::offline_install::serve_pack))
        .route(
            "/",
            get(|| async { Redirect::temporary("/map/index.html") }),
        )
        .route("/*path", get(serve))
        .layer(Extension(installer))
        .layer(Extension(local_router))
}

async fn serve(method: Method, headers: HeaderMap, UrlPath(path): UrlPath<String>) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, "GET, HEAD")],
            "Method Not Allowed",
        )
            .into_response();
    }

    let range = match headers.get(header::RANGE) {
        Some(value) => match value.to_str() {
            Ok(value) => Some(value.to_string()),
            Err(_) => return text_response(StatusCode::BAD_REQUEST, "Invalid Range header"),
        },
        None => None,
    };

    if let Some(asset) = ASSETS.iter().find(|asset| asset.path == path) {
        return embedded_response(asset, range.as_deref(), &method);
    }

    text_response(StatusCode::NOT_FOUND, "Not Found")
}

fn embedded_response(asset: &EmbeddedAsset, range: Option<&str>, method: &Method) -> Response {
    let size = asset.bytes.len() as u64;
    match select_range(range, size) {
        RangeSelection::Full => body_response(
            StatusCode::OK,
            asset.mime,
            size,
            None,
            method,
            Body::from(Bytes::from_static(asset.bytes)),
        ),
        RangeSelection::Partial { start, end } => {
            let slice = &asset.bytes[start as usize..=end as usize];
            body_response(
                StatusCode::PARTIAL_CONTENT,
                asset.mime,
                end - start + 1,
                Some(format!("bytes {start}-{end}/{size}")),
                method,
                Body::from(Bytes::from_static(slice)),
            )
        }
        RangeSelection::Unsatisfiable => range_not_satisfiable(size),
    }
}

enum RangeSelection {
    Full,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

/// Parse one RFC 7233 byte range. Multipart ranges are deliberately rejected:
/// PMTiles only asks for a single contiguous range.
fn select_range(raw: Option<&str>, size: u64) -> RangeSelection {
    let Some(raw) = raw else {
        return if size == 0 {
            RangeSelection::Unsatisfiable
        } else {
            RangeSelection::Full
        };
    };
    let raw = raw.trim();
    let Some(spec) = raw.strip_prefix("bytes=") else {
        return RangeSelection::Unsatisfiable;
    };
    let spec = spec.trim();
    if spec.contains(',') {
        return RangeSelection::Unsatisfiable;
    }
    let Some((start_raw, end_raw)) = spec.split_once('-') else {
        return RangeSelection::Unsatisfiable;
    };
    if size == 0 {
        return RangeSelection::Unsatisfiable;
    }
    let start_raw = start_raw.trim();
    let end_raw = end_raw.trim();

    if start_raw.is_empty() {
        let Ok(suffix) = end_raw.parse::<u64>() else {
            return RangeSelection::Unsatisfiable;
        };
        if suffix == 0 {
            return RangeSelection::Unsatisfiable;
        }
        let start = size.saturating_sub(suffix);
        return RangeSelection::Partial {
            start,
            end: size - 1,
        };
    }

    let Ok(start) = start_raw.parse::<u64>() else {
        return RangeSelection::Unsatisfiable;
    };
    if start >= size {
        return RangeSelection::Unsatisfiable;
    }
    let end = if end_raw.is_empty() {
        size - 1
    } else {
        match end_raw.parse::<u64>() {
            Ok(end) => end.min(size - 1),
            Err(_) => return RangeSelection::Unsatisfiable,
        }
    };
    if end < start {
        return RangeSelection::Unsatisfiable;
    }
    RangeSelection::Partial { start, end }
}

fn body_response(
    status: StatusCode,
    mime: &str,
    length: u64,
    content_range: Option<String>,
    method: &Method,
    body: Body,
) -> Response {
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        body
    };
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CONTENT_LENGTH, length.to_string());
    if mime == MIME_HTML {
        builder = builder.header(
            header::CONTENT_SECURITY_POLICY,
            crate::map_server::MAP_PAGE_CSP,
        );
    }
    if let Some(content_range) = content_range {
        builder = builder.header(header::CONTENT_RANGE, content_range);
    }
    builder
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn range_not_satisfiable(size: u64) -> Response {
    (
        StatusCode::RANGE_NOT_SATISFIABLE,
        [
            (header::CONTENT_TYPE, MIME_TEXT.to_string()),
            (header::CONTENT_RANGE, format!("bytes */{size}")),
        ],
        "Range Not Satisfiable",
    )
        .into_response()
}

fn text_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, MIME_TEXT)],
        message.to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(path: &str) -> &'static EmbeddedAsset {
        ASSETS
            .iter()
            .find(|asset| asset.path == path)
            .expect("asset should be embedded")
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client")
    }

    async fn get(server: &MapServer, path: &str) -> reqwest::Response {
        client()
            .get(format!("{}{path}", server.origin()))
            .send()
            .await
            .expect("loopback request")
    }

    #[tokio::test]
    async fn binds_loopback_and_serves_the_map_page_and_vendor_files() {
        let server = MapServer::start(None).expect("server starts");
        assert!(server.origin().starts_with("http://127.0.0.1:"));
        assert_ne!(server.port(), 0);
        assert_eq!(
            server.map_url(),
            format!("{}/map/index.html", server.origin())
        );

        let index = get(&server, "/map/index.html").await;
        assert_eq!(index.status(), StatusCode::OK);
        assert_eq!(
            index.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(index.headers()[header::ACCEPT_RANGES], "bytes");
        let csp = index.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .expect("map page CSP");
        assert!(csp.contains("connect-src 'self' https://tiles.openfreemap.org"));
        assert!(csp.contains("worker-src 'self'"));
        assert!(!csp.contains('*') && !csp.contains("blob:"), "{csp}");
        let body = index.text().await.unwrap();
        assert!(body.contains("id=\"map\""));
        assert!(
            body.contains("/fixtures/monaco.pmtiles") || body.contains("./fixtures/monaco.pmtiles")
        );
        assert!(body.contains("route-panel"));
        assert!(body.contains("../app/route-ui.js"));
        assert!(body.contains("../app/route.css"));

        let route_js = get(&server, "/app/route-ui.js").await;
        assert_eq!(route_js.status(), StatusCode::OK);
        assert_eq!(
            route_js.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        let route_js_body = route_js.text().await.unwrap();
        assert!(route_js_body.contains("export function createRouteUI"));

        let route_css = get(&server, "/app/route.css").await;
        assert_eq!(route_css.status(), StatusCode::OK);
        assert_eq!(
            route_css.headers()[header::CONTENT_TYPE],
            "text/css; charset=utf-8"
        );
        assert!(route_css.text().await.unwrap().contains(".pinpoint-route"));

        let vendor = get(&server, "/vendor/maplibre-gl.js").await;
        assert_eq!(vendor.status(), StatusCode::OK);
        assert_eq!(
            vendor.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert!(!vendor.bytes().await.unwrap().is_empty());

        let worker = get(&server, "/vendor/maplibre-gl-worker.mjs").await;
        assert_eq!(worker.status(), StatusCode::OK);
        assert_eq!(
            worker.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );

        let style = get(&server, "/map/style.json").await;
        assert_eq!(style.status(), StatusCode::OK);
        assert_eq!(
            style.headers()[header::CONTENT_TYPE],
            "application/json; charset=utf-8"
        );

        let fixture = get(&server, "/map/fixtures/monaco.pmtiles").await;
        assert_eq!(fixture.status(), StatusCode::OK);
        assert!(
            fixture
                .headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .is_none(),
            "only the map page carries the CSP"
        );
        assert_eq!(
            fixture.headers()[header::CONTENT_TYPE],
            "application/octet-stream"
        );
        assert_eq!(
            fixture.headers()[header::CONTENT_LENGTH],
            MONACO_PMTILES.len().to_string()
        );
    }

    #[tokio::test]
    async fn serves_pmtiles_byte_ranges() {
        let server = MapServer::start(None).expect("server starts");
        let fixture = asset("map/fixtures/monaco.pmtiles");
        let size = fixture.bytes.len() as u64;

        let response = client()
            .get(format!("{}/map/fixtures/monaco.pmtiles", server.origin()))
            .header(header::RANGE, "bytes=0-99")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers()[header::CONTENT_RANGE],
            format!("bytes 0-99/{size}")
        );
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "100");
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            &fixture.bytes[..100]
        );

        let suffix = client()
            .get(format!("{}/map/fixtures/monaco.pmtiles", server.origin()))
            .header(header::RANGE, "bytes=-16")
            .send()
            .await
            .unwrap();
        assert_eq!(suffix.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            suffix.headers()[header::CONTENT_RANGE],
            format!("bytes {}-{}/{size}", size - 16, size - 1)
        );
        assert_eq!(suffix.bytes().await.unwrap().len(), 16);

        let open_ended = client()
            .get(format!("{}/map/fixtures/monaco.pmtiles", server.origin()))
            .header(header::RANGE, &format!("bytes={}-", size - 8))
            .send()
            .await
            .unwrap();
        assert_eq!(open_ended.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(open_ended.bytes().await.unwrap().len(), 8);

        let invalid = client()
            .get(format!("{}/map/fixtures/monaco.pmtiles", server.origin()))
            .header(header::RANGE, &format!("bytes={size}-"))
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            invalid.headers()[header::CONTENT_RANGE],
            format!("bytes */{size}")
        );
    }

    #[tokio::test]
    async fn rejects_traversal_and_unknown_paths() {
        let server = MapServer::start(None).expect("server starts");
        for path in [
            "/Cargo.toml",
            "/map/../Cargo.toml",
            "/map/%2e%2e/Cargo.toml",
            "/vendor/../Cargo.toml",
            "/packs/../Cargo.toml",
            "/packs/unknown.pmtiles",
            "/packs/not-a-pack.txt",
        ] {
            let response = get(&server, path).await;
            assert!(
                response.status() == StatusCode::NOT_FOUND
                    || response.status() == StatusCode::BAD_REQUEST,
                "{path} should be closed, got {}",
                response.status()
            );
        }
        let root = get(&server, "/").await;
        assert!(root.status().is_redirection());
        assert_eq!(root.headers()[header::LOCATION], "/map/index.html");
    }

    #[tokio::test]
    async fn serves_downloaded_packs_from_the_packs_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("demo.pmtiles"), b"0123456789").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"not a pack").unwrap();
        let server = MapServer::start(Some(dir.path().to_path_buf())).expect("server starts");

        let full = get(&server, "/packs/demo.pmtiles").await;
        assert_eq!(full.status(), StatusCode::OK);
        assert!(
            full.headers()
                .get(header::CONTENT_SECURITY_POLICY)
                .is_none(),
            "only the map page carries the CSP"
        );
        assert_eq!(
            full.headers()[header::CONTENT_TYPE],
            "application/octet-stream"
        );
        assert_eq!(full.bytes().await.unwrap().as_ref(), b"0123456789");

        let ranged = client()
            .get(format!("{}/packs/demo.pmtiles", server.origin()))
            .header(header::RANGE, "bytes=2-5")
            .send()
            .await
            .unwrap();
        assert_eq!(ranged.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(ranged.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
        assert_eq!(ranged.bytes().await.unwrap().as_ref(), b"2345");

        assert_eq!(
            get(&server, "/packs/notes.txt").await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get(&server, "/packs/../demo.pmtiles").await.status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn serves_the_region_catalog_and_region_packs() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("moldova")).expect("region dir");
        std::fs::write(dir.path().join("moldova/moldova.pmtiles"), b"0123456789").unwrap();
        let server =
            MapServer::start_with_plan(Some(dir.path().to_path_buf()), DownloadPlan::default())
                .expect("server starts");

        let catalog = get(&server, "/map/catalog").await;
        assert_eq!(catalog.status(), StatusCode::OK);
        assert!(
            catalog.headers()[header::CONTENT_TYPE]
                .to_str()
                .expect("catalog content type")
                .starts_with("application/json"),
            "unexpected catalog content type: {:?}",
            catalog.headers()[header::CONTENT_TYPE]
        );
        let catalog: serde_json::Value = catalog.json().await.expect("catalog JSON");
        assert!(catalog["regions"].is_array(), "catalog: {catalog}");

        let ranged = client()
            .get(format!("{}/packs/moldova/moldova.pmtiles", server.origin()))
            .header(header::RANGE, "bytes=2-5")
            .send()
            .await
            .unwrap();
        assert_eq!(ranged.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(ranged.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
        assert_eq!(ranged.bytes().await.unwrap().as_ref(), b"2345");

        assert_eq!(
            get(&server, "/packs/atlantis/moldova.pmtiles")
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn download_takes_a_region_id_and_rejects_unknown_regions() {
        let dir = tempfile::tempdir().expect("temp dir");
        // A dead host, so a job can never start a real transfer from this test.
        let plan = DownloadPlan {
            base_url: Some("http://127.0.0.1:1".to_string()),
            ..Default::default()
        };
        let server = MapServer::start_with_plan(Some(dir.path().to_path_buf()), plan)
            .expect("server starts");

        let unknown = client()
            .post(format!("{}/map/offline/download", server.origin()))
            .json(&serde_json::json!({"region": "atlantis"}))
            .send()
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn head_has_length_without_a_body() {
        let server = MapServer::start(None).expect("server starts");
        let response = client()
            .head(format!("{}/map/index.html", server.origin()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            MAP_INDEX.len().to_string()
        );
        assert!(response.bytes().await.unwrap().is_empty());
    }

    #[test]
    fn range_parser_is_bounded() {
        assert!(matches!(
            select_range(Some("bytes=0-0"), 10),
            RangeSelection::Partial { start: 0, end: 0 }
        ));
        assert!(matches!(
            select_range(Some("bytes=5-"), 10),
            RangeSelection::Partial { start: 5, end: 9 }
        ));
        assert!(matches!(
            select_range(Some("bytes=5-100"), 10),
            RangeSelection::Partial { start: 5, end: 9 }
        ));
        assert!(matches!(
            select_range(Some("bytes=-4"), 10),
            RangeSelection::Partial { start: 6, end: 9 }
        ));
        assert!(matches!(
            select_range(Some("bytes=-0"), 10),
            RangeSelection::Unsatisfiable
        ));
        assert!(matches!(
            select_range(Some("bytes=10-"), 10),
            RangeSelection::Unsatisfiable
        ));
        assert!(matches!(
            select_range(Some("bytes=4-2"), 10),
            RangeSelection::Unsatisfiable
        ));
        assert!(matches!(
            select_range(Some("bytes=0-1,3-4"), 10),
            RangeSelection::Unsatisfiable
        ));
        assert!(matches!(
            select_range(Some("items=0-1"), 10),
            RangeSelection::Unsatisfiable
        ));
    }
}

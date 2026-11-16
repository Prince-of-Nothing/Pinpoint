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
//! - disk-backed packs must be a single `packs/<name>.pmtiles` segment and
//!   are canonicalised under the configured packs directory;
//! - no session, CSRF token or account data is ever served.
//!
//! PMTiles reads through HTTP Range requests, so every response advertises
//! `Accept-Ranges: bytes` and supports `206 Partial Content` and `416`.

use std::io::{Read, Seek, SeekFrom};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use tokio::sync::oneshot;

const MIME_HTML: &str = "text/html; charset=utf-8";
const MIME_CSS: &str = "text/css; charset=utf-8";
const MIME_JS: &str = "text/javascript; charset=utf-8";
const MIME_JSON: &str = "application/json; charset=utf-8";
const MIME_PMTILES: &str = "application/octet-stream";
const MIME_TEXT: &str = "text/plain; charset=utf-8";

/// Largest single HTTP response the server buffers for a downloaded pack.
/// PMTiles clients use ranges; a client that asks for a whole large pack is
/// told to use a range instead of allocating the file twice.
const MAX_BUFFERED_PACK_BYTES: u64 = 64 * 1024 * 1024;

const MAP_INDEX: &[u8] = include_bytes!("../../web/map/index.html");
const MAP_PREVIEW_CSS: &[u8] = include_bytes!("../../web/map/preview.css");
const MAP_JS: &[u8] = include_bytes!("../../web/map/map.js");
const MAP_STYLE: &[u8] = include_bytes!("../../web/map/style.json");
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

struct ServerState {
    packs_dir: Option<PathBuf>,
}

/// A running loopback map server. Dropping it shuts the thread down.
pub struct MapServer {
    origin: String,
    port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MapServer {
    /// Bind `127.0.0.1:0` and serve the embedded map assets plus, optionally,
    /// `*.pmtiles` files from `packs_dir` at `/packs/<name>.pmtiles`.
    pub fn start(packs_dir: Option<PathBuf>) -> std::io::Result<Self> {
        let listener = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let state = Arc::new(ServerState { packs_dir });
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
                    let app = router(state);
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

fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Redirect::temporary("/map/index.html") }),
        )
        .route("/*path", get(serve))
        .with_state(state)
}

async fn serve(
    State(state): State<Arc<ServerState>>,
    method: Method,
    headers: HeaderMap,
    UrlPath(path): UrlPath<String>,
) -> Response {
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

    if let Some(name) = path.strip_prefix("packs/") {
        return pack_response(&state, name, range.as_deref(), &method).await;
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

async fn pack_response(
    state: &ServerState,
    name: &str,
    range: Option<&str>,
    method: &Method,
) -> Response {
    let Some(file_name) = valid_pack_name(name) else {
        return text_response(StatusCode::NOT_FOUND, "Not Found");
    };
    let Some(packs_dir) = state.packs_dir.as_ref() else {
        return text_response(StatusCode::NOT_FOUND, "No offline pack directory");
    };

    let root = packs_dir.clone();
    let path = packs_dir.join(file_name);
    let resolved = tokio::task::spawn_blocking(move || -> std::io::Result<(PathBuf, u64)> {
        let root = std::fs::canonicalize(&root)?;
        let path = std::fs::canonicalize(&path)?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "pack outside the packs directory",
            ));
        }
        let size = std::fs::metadata(&path)?.len();
        Ok((path, size))
    })
    .await;

    let (path, size) = match resolved {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            tracing::debug!(%error, pack = file_name, "map server: pack unavailable");
            return text_response(StatusCode::NOT_FOUND, "Not Found");
        }
        Err(error) => {
            tracing::warn!(%error, "map server: pack lookup task failed");
            return text_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
        }
    };

    let (start, end) = match select_range(range, size) {
        RangeSelection::Full => {
            if size > MAX_BUFFERED_PACK_BYTES {
                return pack_range_required(size);
            }
            (0, size.saturating_sub(1))
        }
        RangeSelection::Partial { start, end } => (start, end),
        RangeSelection::Unsatisfiable => return range_not_satisfiable(size),
    };
    let length = end.saturating_sub(start) + 1;
    if length > MAX_BUFFERED_PACK_BYTES {
        return pack_range_required(size);
    }

    match read_pack_range(path, start, length).await {
        Ok(bytes) => body_response(
            if range.is_some() {
                StatusCode::PARTIAL_CONTENT
            } else {
                StatusCode::OK
            },
            MIME_PMTILES,
            length,
            range.map(|_| format!("bytes {start}-{end}/{size}")),
            method,
            Body::from(bytes),
        ),
        Err(error) => {
            tracing::warn!(%error, pack = file_name, "map server: pack read failed");
            text_response(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
        }
    }
}

async fn read_pack_range(path: PathBuf, start: u64, length: u64) -> std::io::Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || -> std::io::Result<Vec<u8>> {
        let mut file = std::fs::File::open(path)?;
        file.seek(SeekFrom::Start(start))?;
        let mut buffer = vec![0u8; length as usize];
        file.read_exact(&mut buffer)?;
        Ok(buffer)
    })
    .await
    .map_err(|error| std::io::Error::other(format!("pack read task failed: {error}")))?
}

/// Only a single, non-hidden `*.pmtiles` file name is accepted. The caller
/// still canonicalises the joined path to reject symlinks and `..`.
fn valid_pack_name(name: &str) -> Option<&str> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || !name.ends_with(".pmtiles")
        || name
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | ':'))
    {
        return None;
    }
    Some(name)
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

fn pack_range_required(size: u64) -> Response {
    (
        StatusCode::RANGE_NOT_SATISFIABLE,
        [
            (header::CONTENT_TYPE, MIME_TEXT.to_string()),
            (header::CONTENT_RANGE, format!("bytes */{size}")),
        ],
        "Use a byte range to read this offline pack",
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
        let body = index.text().await.unwrap();
        assert!(body.contains("id=\"map\""));
        assert!(
            body.contains("/fixtures/monaco.pmtiles") || body.contains("./fixtures/monaco.pmtiles")
        );

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

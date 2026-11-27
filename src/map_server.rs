//! Loopback HTTP server for the shared MapLibre map page.
//!
//! The optional desktop WebView window (`--map-window`, feature
//! `map-webview`) loads `/map/index.html` from this server instead of
//! `file://`: ES modules, module workers and PMTiles need one HTTP origin,
//! and PMTiles reads tiles with `Range` requests.
//!
//! The server is deliberately small:
//!
//! * binds `127.0.0.1:0` (ephemeral loopback port), never a public interface;
//! * serves only the files under `web/` (the map page, fixture and vendor
//!   assets), with path-traversal and symlink-escape checks;
//! * supports single `Range` requests with `206` and `Content-Range`, and
//!   answers unsatisfiable ranges with `416`, matching `web/map/serve.mjs`;
//! * sends `Accept-Ranges: bytes` and correct MIME types for the page;
//! * mounts the shared offline installer (`/map/catalog`, `/map/offline/*`,
//!   `/map/config`) and streams installed `packs/<region>/<name>` archives
//!   with Range support (legacy `packs/<name>` stays reachable).
//!
//! This is not the production pack server. It exposes only public, bundled
//! ODbL map assets and downloaded public pack files to the local machine and
//! holds no session or friend data.

use axum::body::{Body, Bytes};
use axum::extract::{Extension, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::offline_install::{DownloadPlan, OfflineInstaller};

/// Directory under a repository/packaging root that contains `map/` and
/// `vendor/`.
const WEB_DIR: &str = "web";

/// Marker used to validate a candidate web root.
const WEB_MARKER: &str = "map/index.html";

/// Content type of the map page (and the only response that carries the CSP).
const MIME_HTML: &str = "text/html; charset=utf-8";

/// CSP for the loopback map page in the desktop `wry` window and the Android
/// WebView (`docs/offline/android-webview-online.md` §3): one remote origin,
/// the opt-in OpenFreeMap basemap; the two inline blocks in
/// `web/map/index.html` need `'unsafe-inline'`; the vendored MapLibre worker is
/// same-origin, so no `blob:`. Applied to HTML only, never to pack bytes.
pub(crate) const MAP_PAGE_CSP: &str = "default-src 'self'; \
script-src 'self' 'unsafe-inline'; worker-src 'self'; \
style-src 'self' 'unsafe-inline'; \
img-src 'self' data: https://tiles.openfreemap.org; font-src 'self'; \
connect-src 'self' https://tiles.openfreemap.org; base-uri 'none'; \
frame-ancestors 'none'; form-action 'none'; object-src 'none'";

/// Start a map server on an ephemeral loopback port.
///
/// The server owns its own single-threaded Tokio runtime so callers can start
/// it from the egui thread (or a synchronous CLI mode) without an ambient
/// runtime. The returned handle keeps the listener alive until it drops.
pub struct MapServer {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    token: String,
}

impl MapServer {
    /// Start the server with the app's own `<app data>/packs` directory and the
    /// production region catalog.
    pub fn start(web_root: impl AsRef<Path>) -> anyhow::Result<Self> {
        let packs = crate::platform::app_data_dir().map(|dir| dir.join("packs"));
        Self::start_with(web_root, packs, OfflineInstaller::new)
    }

    /// Start the server with an explicit packs directory (tests and tooling).
    /// These keep the v2.3.0 single-pack plan so they stay hermetic; production
    /// uses the region catalog through [`Self::start`].
    pub fn start_with_packs(
        web_root: impl AsRef<Path>,
        packs_dir: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        Self::start_with(web_root, packs_dir, |packs| {
            OfflineInstaller::with_plan(packs, DownloadPlan::default())
        })
    }

    /// Start with explicit artifact metadata. Only used by fixture tests; the
    /// production path is [`Self::start`].
    pub fn start_with_plan(
        web_root: impl AsRef<Path>,
        packs_dir: Option<PathBuf>,
        plan: DownloadPlan,
    ) -> anyhow::Result<Self> {
        Self::start_with(web_root, packs_dir, move |packs| {
            OfflineInstaller::with_plan(packs, plan)
        })
    }

    fn start_with(
        web_root: impl AsRef<Path>,
        packs_dir: Option<PathBuf>,
        make_installer: impl FnOnce(Option<PathBuf>) -> OfflineInstaller + Send + 'static,
    ) -> anyhow::Result<Self> {
        use anyhow::Context as _;

        let root = std::fs::canonicalize(web_root.as_ref())
            .with_context(|| format!("map web root {:?} is not readable", web_root.as_ref()))?;
        if !root.join(WEB_MARKER).is_file() {
            anyhow::bail!("map web root {:?} has no {WEB_MARKER}", root);
        }

        let token = crate::map_guard::new_token();
        let thread_token = token.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<SocketAddr, String>>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let assets = MapAssets {
            root: Arc::new(root),
        };
        let router = Arc::new(crate::local_route::LocalRouter::for_packs_dir(
            packs_dir.as_deref(),
        ));
        let installer = Arc::new(make_installer(packs_dir));

        std::thread::Builder::new()
            .name("pinpoint-map-server".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready_tx.send(Err(format!(
                            "could not create the map server runtime: {error}"
                        )));
                        return;
                    }
                };

                runtime.block_on(async move {
                    let listener = match TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await
                    {
                        Ok(listener) => listener,
                        Err(error) => {
                            let _ = ready_tx
                                .send(Err(format!("could not bind the map server: {error}")));
                            return;
                        }
                    };
                    let addr = match listener.local_addr() {
                        Ok(addr) => addr,
                        Err(error) => {
                            let _ = ready_tx.send(Err(format!(
                                "could not read the map server address: {error}"
                            )));
                            return;
                        }
                    };

                    let guard = crate::map_guard::MapGuard::new(addr.port(), thread_token);
                    let app = Router::new()
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
                        .fallback(serve_asset)
                        .layer(Extension(installer))
                        .layer(Extension(router))
                        .layer(axum::middleware::from_fn(move |request, next| {
                            crate::map_guard::enforce(guard.clone(), request, next)
                        }))
                        .with_state(assets);

                    if ready_tx.send(Ok(addr)).is_err() {
                        return;
                    }

                    tracing::info!(%addr, "map loopback server listening");
                    if let Err(error) = axum::serve(listener, app)
                        .with_graceful_shutdown(async move {
                            let _ = shutdown_rx.await;
                        })
                        .await
                    {
                        tracing::warn!(%error, "map loopback server stopped");
                    }
                });
            })
            .map_err(|error| anyhow::anyhow!("could not start the map server thread: {error}"))?;

        let addr = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow::anyhow!("timed out waiting for the map server to bind"))?
            .map_err(anyhow::Error::msg)?;

        Ok(Self {
            addr,
            shutdown: Some(shutdown_tx),
            token,
        })
    }

    /// Actual loopback address (`127.0.0.1:<ephemeral>`).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Root URL for the served `web/` tree.
    pub fn base_url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    /// URL the WebView must load. The fragment hands the token to the page only.
    pub fn index_url(&self) -> String {
        format!(
            "http://{}/{WEB_MARKER}{}",
            self.addr,
            crate::map_guard::fragment(&self.token)
        )
    }

    /// Per-launch token the map page sends with state-changing requests.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Stop accepting requests. The server thread also exits when the handle
    /// drops or the process ends.
    pub fn shutdown(mut self) {
        self.signal_shutdown();
    }

    fn signal_shutdown(&mut self) {
        if let Some(sender) = self.shutdown.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for MapServer {
    fn drop(&mut self) {
        self.signal_shutdown();
    }
}

#[derive(Clone)]
struct MapAssets {
    root: Arc<PathBuf>,
}

async fn serve_asset(State(assets): State<MapAssets>, request: Request) -> Response {
    let method = request.method().clone();
    if method != Method::GET && method != Method::HEAD {
        return error_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "Method Not Allowed",
            Some("GET, HEAD"),
        );
    }

    let Some(relative) = safe_relative_path(request.uri().path()) else {
        return error_response(StatusCode::FORBIDDEN, "Forbidden", None);
    };

    let path = match resolve_file(&assets.root, &relative).await {
        Ok(path) => path,
        Err(status) => {
            return error_response(status, status.canonical_reason().unwrap_or("Error"), None)
        }
    };

    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(_) => return error_response(StatusCode::NOT_FOUND, "Not Found", None),
    };
    let len = metadata.len();
    let mime = content_type(&path);

    let range_header = match request.headers().get(header::RANGE) {
        Some(value) => match value.to_str() {
            Ok(value) => Some(value),
            Err(_) => return range_not_satisfiable(len),
        },
        None => None,
    };

    match parse_range(range_header, len) {
        ByteRange::Unsatisfiable => range_not_satisfiable(len),
        ByteRange::Full => {
            if method == Method::HEAD {
                return asset_response(StatusCode::OK, mime, len, Body::empty(), None);
            }
            match tokio::fs::read(&path).await {
                Ok(bytes) => asset_response(
                    StatusCode::OK,
                    mime,
                    len,
                    Body::from(Bytes::from(bytes)),
                    None,
                ),
                Err(_) => error_response(StatusCode::NOT_FOUND, "Not Found", None),
            }
        }
        ByteRange::Partial { start, end } => {
            let length = end - start + 1;
            if method == Method::HEAD {
                return asset_response(
                    StatusCode::PARTIAL_CONTENT,
                    mime,
                    length,
                    Body::empty(),
                    Some(format!("bytes {start}-{end}/{len}")),
                );
            }
            match tokio::fs::read(&path).await {
                Ok(bytes) => {
                    let bytes = Bytes::from(bytes);
                    let slice = bytes.slice(start as usize..=end as usize);
                    asset_response(
                        StatusCode::PARTIAL_CONTENT,
                        mime,
                        length,
                        Body::from(slice),
                        Some(format!("bytes {start}-{end}/{len}")),
                    )
                }
                Err(_) => error_response(StatusCode::NOT_FOUND, "Not Found", None),
            }
        }
    }
}

async fn resolve_file(root: &Path, relative: &Path) -> Result<PathBuf, StatusCode> {
    let candidate = root.join(relative);
    let canonical = tokio::fs::canonicalize(&candidate)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if !canonical.starts_with(root) {
        return Err(StatusCode::FORBIDDEN);
    }

    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if metadata.is_dir() {
        let index = canonical.join("index.html");
        let index = tokio::fs::canonicalize(&index)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;
        if !index.starts_with(root) {
            return Err(StatusCode::FORBIDDEN);
        }
        return Ok(index);
    }
    if metadata.is_file() {
        Ok(canonical)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Decode a request path into a relative path below the web root.
///
/// Rejects encoded separators/traversal and Windows path syntax rather than
/// normalising them, so `..`, `C:` and `\` can never escape `web/`.
fn safe_relative_path(raw: &str) -> Option<PathBuf> {
    let decoded = percent_decode(raw)?;
    if decoded.contains('\0') {
        return None;
    }
    let mut relative = PathBuf::new();
    for part in decoded.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains('\\') || part.contains(':') {
            return None;
        }
        relative.push(part);
    }
    Some(relative)
}

fn percent_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteRange {
    Full,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

fn parse_range(raw: Option<&str>, len: u64) -> ByteRange {
    let Some(raw) = raw else {
        return ByteRange::Full;
    };
    let spec = raw.trim();
    let Some(spec) = spec
        .get(..6)
        .filter(|prefix| prefix.eq_ignore_ascii_case("bytes="))
        .map(|_| spec[6..].trim())
    else {
        return ByteRange::Unsatisfiable;
    };
    if len == 0 || spec.contains(',') {
        return ByteRange::Unsatisfiable;
    }
    let Some((start_text, end_text)) = spec.split_once('-') else {
        return ByteRange::Unsatisfiable;
    };
    if start_text.is_empty() && end_text.is_empty() {
        return ByteRange::Unsatisfiable;
    }

    if start_text.is_empty() {
        let Ok(suffix) = end_text.parse::<u64>() else {
            return ByteRange::Unsatisfiable;
        };
        if suffix == 0 {
            return ByteRange::Unsatisfiable;
        }
        let start = len.saturating_sub(suffix);
        return ByteRange::Partial {
            start,
            end: len - 1,
        };
    }

    let Ok(start) = start_text.parse::<u64>() else {
        return ByteRange::Unsatisfiable;
    };
    if start >= len {
        return ByteRange::Unsatisfiable;
    }
    let end = if end_text.is_empty() {
        len - 1
    } else {
        let Ok(end) = end_text.parse::<u64>() else {
            return ByteRange::Unsatisfiable;
        };
        if end < start {
            return ByteRange::Unsatisfiable;
        }
        end.min(len - 1)
    };
    ByteRange::Partial { start, end }
}

fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => MIME_HTML,
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("md" | "txt") => "text/plain; charset=utf-8",
        Some("pmtiles") => "application/octet-stream",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn asset_response(
    status: StatusCode,
    mime: &'static str,
    content_length: u64,
    body: Body,
    content_range: Option<String>,
) -> Response {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if mime == MIME_HTML {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(MAP_PAGE_CSP),
        );
    }
    if let Ok(value) = HeaderValue::from_str(&content_length.to_string()) {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    if let Some(content_range) = content_range {
        if let Ok(value) = HeaderValue::from_str(&content_range) {
            headers.insert(header::CONTENT_RANGE, value);
        }
    }
    response
}

fn range_not_satisfiable(len: u64) -> Response {
    let mut response = error_response(
        StatusCode::RANGE_NOT_SATISFIABLE,
        "Range Not Satisfiable",
        None,
    );
    if let Ok(value) = HeaderValue::from_str(&format!("bytes */{len}")) {
        response.headers_mut().insert(header::CONTENT_RANGE, value);
    }
    response
}

fn error_response(
    status: StatusCode,
    message: &'static str,
    allow: Option<&'static str>,
) -> Response {
    let mut response = Response::new(Body::from(message));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if let Some(allow) = allow {
        headers.insert(header::ALLOW, HeaderValue::from_static(allow));
    }
    response
}

/// Locate the repository/packaging `web/` directory that contains the map
/// page and vendored libraries.
///
/// Resolution order: `PINPOINT_WEB_ROOT`, the current directory, ancestors of
/// the executable (packaged layout), then the compile-time manifest directory
/// (development builds). Returns a helpful error instead of serving 404s when
/// the assets were not packaged.
pub fn find_web_root() -> anyhow::Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(raw) = std::env::var_os("PINPOINT_WEB_ROOT") {
        candidates.push(PathBuf::from(raw));
    }
    if let Ok(current) = std::env::current_dir() {
        candidates.push(current);
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.extend(parent.ancestors().map(Path::to_path_buf));
        }
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    for base in candidates {
        let root = base.join(WEB_DIR);
        if root.join(WEB_MARKER).is_file() && root.join("vendor/maplibre-gl.js").is_file() {
            return std::fs::canonicalize(&root)
                .map_err(|error| anyhow::anyhow!("map web root {:?}: {error}", root));
        }
    }

    anyhow::bail!(
        "could not find web/map/index.html next to the executable; \
         run from the repository, set PINPOINT_WEB_ROOT, or package the web/ tree"
    )
}

#[cfg(test)]
mod tests {
    use super::{find_web_root, MapServer};
    use crate::offline_install::PACK_BYTES;
    use std::path::PathBuf;

    fn repo_web_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web")
    }

    fn fixture_len() -> u64 {
        std::fs::metadata(repo_web_root().join("map/fixtures/monaco.pmtiles"))
            .expect("fixture metadata")
            .len()
    }

    #[test]
    fn finds_the_repository_web_root() {
        let root = find_web_root().expect("web root");
        assert!(root.join("map/index.html").is_file());
        assert!(root.join("vendor/pmtiles.js").is_file());
    }

    #[tokio::test]
    async fn serves_the_map_page_with_attribution_and_mime_types() {
        let server = MapServer::start(repo_web_root()).expect("server");
        let client = reqwest::Client::new();

        let page = client
            .get(server.index_url())
            .send()
            .await
            .expect("page request");
        assert_eq!(page.status(), reqwest::StatusCode::OK);
        assert_eq!(page.headers()["content-type"], "text/html; charset=utf-8");
        assert_eq!(page.headers()["accept-ranges"], "bytes");
        let csp = page.headers()["content-security-policy"]
            .to_str()
            .expect("map page CSP");
        assert!(csp.contains("https://tiles.openfreemap.org"), "{csp}");
        assert!(csp.contains("connect-src 'self' https://tiles.openfreemap.org"));
        assert!(!csp.contains('*') && !csp.contains("blob:"), "{csp}");
        let body = page.text().await.expect("page body");
        assert!(body.contains("OpenStreetMap contributors"));
        assert!(body.contains("ODbL 1.0"));
        assert!(body.contains("route-panel"));
        assert!(body.contains("../app/route-ui.js"));
        assert!(body.contains("../app/route.css"));

        for (path, mime) in [
            ("app/route-ui.js", "text/javascript; charset=utf-8"),
            ("app/route.css", "text/css; charset=utf-8"),
        ] {
            let asset = client
                .get(format!("{}{path}", server.base_url()))
                .send()
                .await
                .expect("route UI asset request");
            assert_eq!(asset.status(), reqwest::StatusCode::OK, "{path}");
            assert_eq!(asset.headers()["content-type"], mime, "{path}");
            assert!(
                !asset.bytes().await.expect("route UI asset body").is_empty(),
                "{path} must not be empty"
            );
        }

        let style = client
            .get(format!("{}map/style.json", server.base_url()))
            .send()
            .await
            .expect("style request");
        assert_eq!(
            style.headers()["content-type"],
            "application/json; charset=utf-8"
        );

        let vendor = client
            .get(format!("{}vendor/maplibre-gl.js", server.base_url()))
            .send()
            .await
            .expect("vendor request");
        assert_eq!(
            vendor.headers()["content-type"],
            "text/javascript; charset=utf-8"
        );

        drop(client);
        server.shutdown();
    }

    #[tokio::test]
    async fn pmtiles_range_requests_return_206_and_content_range() {
        let server = MapServer::start(repo_web_root()).expect("server");
        let client = reqwest::Client::new();
        let total = fixture_len();
        let fixture = format!("{}map/fixtures/monaco.pmtiles", server.base_url());

        let response = client
            .get(&fixture)
            .header(reqwest::header::RANGE, "bytes=0-15")
            .send()
            .await
            .expect("range request");
        assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers()["content-range"],
            format!("bytes 0-15/{total}")
        );
        assert_eq!(response.headers()["content-length"], "16");
        assert_eq!(response.headers()["accept-ranges"], "bytes");
        let body = response.bytes().await.expect("range body");
        assert_eq!(body.len(), 16);
        assert_eq!(&body[..7], b"PMTiles");

        let suffix = client
            .get(&fixture)
            .header(reqwest::header::RANGE, "bytes=-16")
            .send()
            .await
            .expect("suffix range");
        assert_eq!(suffix.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            suffix.headers()["content-range"],
            format!("bytes {}-{}/{total}", total - 16, total - 1)
        );
        assert_eq!(suffix.bytes().await.expect("suffix body").len(), 16);

        drop(client);
        server.shutdown();
    }

    #[tokio::test]
    async fn config_and_installed_packs_are_served_from_the_packs_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let server = MapServer::start_with_packs(repo_web_root(), Some(dir.path().to_path_buf()))
            .expect("server");
        let client = reqwest::Client::new();

        let config: serde_json::Value = client
            .get(format!("{}map/config", server.base_url()))
            .send()
            .await
            .expect("config request")
            .json()
            .await
            .expect("config JSON");
        assert_eq!(config["pmtilesUrl"], "./fixtures/monaco.pmtiles");
        assert_eq!(config["packInstalled"], false);
        assert_eq!(config["graphInstalled"], false);
        assert!(config["regions"].is_array(), "config: {config}");

        // A sparse file with the production length is enough for the length
        // check; the real SHA-256 is verified by the download manager.
        std::fs::File::create(dir.path().join("moldova.pmtiles"))
            .expect("sparse pack")
            .set_len(PACK_BYTES)
            .expect("pack length");

        let config: serde_json::Value = client
            .get(format!("{}map/config", server.base_url()))
            .send()
            .await
            .expect("config request")
            .json()
            .await
            .expect("config JSON");
        assert_eq!(config["pmtilesUrl"], "/packs/moldova.pmtiles");
        assert_eq!(config["packInstalled"], true);
        assert_eq!(config["graphInstalled"], false);

        let status: serde_json::Value = client
            .get(format!("{}map/offline/status", server.base_url()))
            .send()
            .await
            .expect("status request")
            .json()
            .await
            .expect("status JSON");
        assert_eq!(status["pack"]["installed"], true);
        assert_eq!(status["pack"]["bytes"], PACK_BYTES);
        assert_eq!(status["pack"]["expected_bytes"], PACK_BYTES);
        assert_eq!(status["pack"]["phase"], "installed");
        assert_eq!(status["graph"]["installed"], false);

        let ranged = client
            .get(format!("{}packs/moldova.pmtiles", server.base_url()))
            .header(reqwest::header::RANGE, "bytes=0-15")
            .send()
            .await
            .expect("pack range");
        assert_eq!(ranged.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            ranged.headers()["content-range"],
            format!("bytes 0-15/{PACK_BYTES}")
        );
        assert!(
            ranged.headers().get("content-security-policy").is_none(),
            "pack bytes must not carry the page CSP"
        );
        assert_eq!(ranged.bytes().await.expect("range body").len(), 16);

        let invalid = client
            .get(format!("{}packs/moldova.pmtiles", server.base_url()))
            .header(reqwest::header::RANGE, format!("bytes={PACK_BYTES}-"))
            .send()
            .await
            .expect("invalid range");
        assert_eq!(invalid.status(), reqwest::StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            invalid.headers()["content-range"],
            format!("bytes */{PACK_BYTES}")
        );

        drop(client);
        server.shutdown();
    }

    #[tokio::test]
    async fn serves_the_region_catalog_and_region_packs() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("moldova")).expect("region dir");
        std::fs::File::create(dir.path().join("moldova/moldova.pmtiles"))
            .expect("sparse pack")
            .set_len(PACK_BYTES)
            .expect("pack length");
        let server = MapServer::start_with_packs(repo_web_root(), Some(dir.path().to_path_buf()))
            .expect("server");
        let client = reqwest::Client::new();

        let catalog = client
            .get(format!("{}map/catalog", server.base_url()))
            .send()
            .await
            .expect("catalog request");
        assert_eq!(catalog.status(), reqwest::StatusCode::OK);
        assert!(
            catalog.headers()["content-type"]
                .to_str()
                .expect("catalog content type")
                .starts_with("application/json"),
            "unexpected catalog content type: {:?}",
            catalog.headers()["content-type"]
        );
        let catalog: serde_json::Value = catalog.json().await.expect("catalog JSON");
        assert!(catalog["regions"].is_array(), "catalog: {catalog}");

        let ranged = client
            .get(format!(
                "{}packs/moldova/moldova.pmtiles",
                server.base_url()
            ))
            .header(reqwest::header::RANGE, "bytes=0-15")
            .send()
            .await
            .expect("region range");
        assert_eq!(ranged.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            ranged.headers()["content-range"],
            format!("bytes 0-15/{PACK_BYTES}")
        );
        assert_eq!(ranged.headers()["accept-ranges"], "bytes");
        assert!(
            ranged.headers().get("content-security-policy").is_none(),
            "pack bytes must not carry the page CSP"
        );
        assert_eq!(ranged.bytes().await.expect("range body").len(), 16);

        let unknown = client
            .get(format!(
                "{}packs/atlantis/moldova.pmtiles",
                server.base_url()
            ))
            .send()
            .await
            .expect("unknown region request");
        assert_eq!(unknown.status(), reqwest::StatusCode::NOT_FOUND);

        drop(client);
        server.shutdown();
    }

    #[tokio::test]
    async fn unsatisfiable_ranges_return_416() {
        let server = MapServer::start(repo_web_root()).expect("server");
        let client = reqwest::Client::new();
        let total = fixture_len();
        let fixture = format!("{}map/fixtures/monaco.pmtiles", server.base_url());

        for range in [
            format!("bytes={total}-"),
            "bytes=0-1,3-4".to_string(),
            "bytes=-0".to_string(),
            "items=0-1".to_string(),
        ] {
            let response = client
                .get(&fixture)
                .header(reqwest::header::RANGE, &range)
                .send()
                .await
                .expect("invalid range request");
            assert_eq!(
                response.status(),
                reqwest::StatusCode::RANGE_NOT_SATISFIABLE,
                "range {range}"
            );
            assert_eq!(
                response.headers()["content-range"],
                format!("bytes */{total}"),
                "range {range}"
            );
        }

        drop(client);
        server.shutdown();
    }

    #[test]
    fn rejects_encoded_traversal_and_windows_paths() {
        for raw in [
            "/map/../Cargo.toml",
            "/map/%2e%2e/Cargo.toml",
            "/map/%2E%2E/Cargo.toml",
            "/C:/Windows/win.ini",
            "/map/%5c..%5cCargo.toml",
            "/map/%00index.html",
            "/map/%zz",
        ] {
            assert!(
                super::safe_relative_path(raw).is_none(),
                "path must be rejected: {raw}"
            );
        }
        assert_eq!(
            super::safe_relative_path("/map/index.html"),
            Some(PathBuf::from("map/index.html"))
        );
    }

    #[tokio::test]
    async fn rejects_raw_traversal_and_non_get_methods() {
        use std::io::{Read, Write};

        let server = MapServer::start(repo_web_root()).expect("server");

        let mut stream = std::net::TcpStream::connect(server.addr()).expect("connect");
        stream
            .write_all(
                b"GET /map/%2e%2e/Cargo.toml HTTP/1.1\r\n\
                  Host: 127.0.0.1\r\n\
                  Connection: close\r\n\r\n",
            )
            .expect("write traversal");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read traversal");
        assert!(
            response.starts_with("HTTP/1.1 403"),
            "unexpected traversal response: {response}"
        );

        let client = reqwest::Client::new();
        let post = client
            .post(format!("{}map/index.html", server.base_url()))
            .header(crate::map_guard::TOKEN_HEADER, server.token())
            .send()
            .await
            .expect("post request");
        assert_eq!(post.status(), reqwest::StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(post.headers()["allow"], "GET, HEAD");

        drop(client);
        server.shutdown();
    }
}

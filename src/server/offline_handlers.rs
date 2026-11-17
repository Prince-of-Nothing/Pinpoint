//! Same-origin download proxy for the published offline packs.
//!
//! The web app and the Android/desktop WebView download the Moldova pack from
//! the Pinpoint server instead of the release host: GitHub release assets send
//! no `Access-Control-Allow-Origin`, so a page (or a page served from a
//! loopback WebView origin) cannot `fetch` them. This module publishes the
//! manifest from `web/app/packs/` and streams the three artifacts with Range
//! passthrough, which PMTiles readers and resumable downloads need.
//!
//! Security: the pack id and the file names are whitelisted constants. The
//! client never supplies a URL, so the handler cannot be used as an SSRF hop;
//! unknown ids/files return `404` before any outbound request. `Range` is the
//! only request header forwarded upstream.
//!
//! Design: `docs/superpowers/specs/2026-11-16-offline-maps-design.md` §7
//! (hosting) and §11 (download manager). Contract: `web/app/packs/README.md`.

use axum::{
    body::Body,
    extract::Path,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::OnceLock;
use std::time::Duration;

use super::AppState;

/// Release that holds the published Moldova artifacts. Immutable tag: a new
/// build publishes a new `data-moldova-YYYYMMDD` release.
const DEFAULT_PACK_BASE_URL: &str =
    "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/data-moldova-20261116";

/// The only pack this deployment publishes today.
const PACK_ID: &str = "moldova";

/// The committed manifest, already rewritten to same-origin URLs.
const MANIFEST_JSON: &str = include_str!("../../web/app/packs/moldova.json");

/// Whitelisted artifacts: `(file name, Content-Type)`. Anything not listed
/// here is a 404. Artifact URLs are not versioned by path, so caching stays
/// short: the download manager re-reads the manifest and verifies SHA-256.
const PACK_FILES: &[(&str, &str)] = &[
    ("moldova.pmtiles", "application/vnd.pmtiles"),
    ("moldova-search.sqlite", "application/vnd.sqlite3"),
    ("moldova-valhalla.tar", "application/x-tar"),
];

const CACHE_CONTROL: &str = "public, max-age=300";
const ACAO: &str = "*";
const EXPOSE_HEADERS: &str = "Content-Range, Content-Length, Accept-Ranges";

/// Pack routes. Merged by [`super::offline_router`] so a parallel change to the
/// web-app router only conflicts on that single merge line.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/offline/packs/:pack/manifest.json", get(manifest))
        .route("/offline/packs/:pack/:file", get(asset).options(preflight))
}

/// `GET /offline/packs/moldova/manifest.json`
///
/// Serves the committed manifest verbatim. Its `files[].url` entries are
/// already same-origin (`/offline/packs/moldova/<name>`), so the client never
/// needs to know the release host.
pub async fn manifest(Path(pack): Path<String>) -> Response {
    if pack != PACK_ID {
        return not_found();
    }
    let mut resp = MANIFEST_JSON.as_bytes().into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_CONTROL),
    );
    apply_cors(headers);
    resp
}

/// `GET|HEAD /offline/packs/moldova/<file>`
///
/// Streams the whitelisted release asset and forwards `Range` upstream. The
/// response keeps the upstream `Content-Range`/`Content-Length` and always
/// advertises `Accept-Ranges: bytes`, so a resumable download or a PMTiles
/// range read behaves exactly as it would against the release host.
pub async fn asset(
    Path((pack, file)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let Some((name, content_type)) = resolve_file(&pack, &file) else {
        return not_found();
    };
    let is_head = method == Method::HEAD;
    let url = upstream_url(&pack_base_url(), name);

    let mut request = if is_head {
        client().head(&url)
    } else {
        client().get(&url)
    };
    // Byte-accurate ranges: never let a proxy re-encode the payload.
    request = request.header(header::ACCEPT_ENCODING, "identity");
    if let Some(range) = headers.get(header::RANGE) {
        request = request.header(header::RANGE, range.clone());
    }

    let upstream = match request.send().await {
        Ok(resp) => resp,
        Err(_) => {
            return error(
                StatusCode::BAD_GATEWAY,
                "PACK_UPSTREAM_FAILED",
                "The pack host is unavailable right now. Try again.",
            )
        }
    };

    let status = upstream.status();
    if status != StatusCode::RANGE_NOT_SATISFIABLE && !status.is_success() {
        return error(
            StatusCode::BAD_GATEWAY,
            "PACK_UPSTREAM_FAILED",
            "The pack host did not return the requested artifact.",
        );
    }

    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CACHE_CONTROL),
    );
    for name in [header::CONTENT_LENGTH, header::CONTENT_RANGE] {
        if let Some(value) = upstream.headers().get(&name) {
            response_headers.insert(name, value.clone());
        }
    }
    apply_cors(&mut response_headers);

    let body = if is_head || status == StatusCode::RANGE_NOT_SATISFIABLE {
        Body::empty()
    } else {
        // Streamed: a 220 MB pack must never be buffered in memory.
        Body::from_stream(upstream.bytes_stream())
    };
    (status, response_headers, body).into_response()
}

/// CORS preflight: `Range` is not a safelisted request header, so a
/// cross-origin WebView page that sends it triggers `OPTIONS` first.
pub async fn preflight() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, HEAD, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Range, If-Range"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    apply_cors(headers);
    response
}

/// `(file name, Content-Type)` when `pack`/`file` are both whitelisted.
fn resolve_file(pack: &str, file: &str) -> Option<(&'static str, &'static str)> {
    if pack != PACK_ID {
        return None;
    }
    PACK_FILES.iter().copied().find(|(name, _)| *name == file)
}

/// `PINPOINT_PACK_BASE_URL` (tests/local fixtures) or the GitHub release.
fn pack_base_url() -> String {
    resolve_base_url(std::env::var("PINPOINT_PACK_BASE_URL").ok().as_deref())
}

fn resolve_base_url(raw: Option<&str>) -> String {
    match raw.map(str::trim).filter(|v| !v.is_empty()) {
        Some(base) if base.starts_with("http://") || base.starts_with("https://") => {
            base.trim_end_matches('/').to_string()
        }
        _ => DEFAULT_PACK_BASE_URL.to_string(),
    }
}

/// Only ever joined with a whitelisted file name.
fn upstream_url(base: &str, file: &str) -> String {
    format!("{base}/{file}")
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .user_agent(concat!(
                "Pinpoint/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/Prince-of-Nothing/Pinpoint)"
            ))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn apply_cors(headers: &mut HeaderMap) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static(ACAO),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(EXPOSE_HEADERS),
    );
    // A WebView page on another origin must be able to embed the stream even
    // when the app document sets COEP: require-corp.
    headers.insert(
        header::HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("cross-origin"),
    );
}

fn not_found() -> Response {
    error(
        StatusCode::NOT_FOUND,
        "PACK_NOT_FOUND",
        "Unknown offline pack or file.",
    )
}

fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    fn app() -> Router {
        routes().with_state(AppState::new())
    }

    fn get_req(uri: &str) -> axum::http::Request<Body> {
        axum::http::Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap()
    }

    fn get_range(uri: &str, range: &str) -> axum::http::Request<Body> {
        axum::http::Request::builder()
            .uri(uri)
            .header(header::RANGE, range)
            .body(Body::empty())
            .unwrap()
    }

    async fn body_json(resp: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn whitelist_accepts_only_published_artifacts() {
        for (name, ctype) in PACK_FILES.iter().copied() {
            assert!(resolve_file("moldova", name).is_some(), "{name} rejected");
            assert!(ctype.contains('/'), "{name} has no media type");
        }
        assert!(resolve_file("moldova", "manifest.json").is_none());
        assert!(resolve_file("other", "moldova.pmtiles").is_none());
        assert!(resolve_file("moldova", "../../etc/passwd").is_none());
        assert!(resolve_file("moldova", "..%2F..%2Fetc%2Fpasswd").is_none());
        assert!(resolve_file("moldova", "").is_none());
        assert!(resolve_file("moldova", "moldova.pmtiles ").is_none());
        assert!(resolve_file("Moldova", "moldova.pmtiles").is_none());
        assert!(resolve_file("moldova/x", "moldova.pmtiles").is_none());
    }

    #[test]
    fn base_url_defaults_to_the_release_and_honors_the_override() {
        assert_eq!(resolve_base_url(None), DEFAULT_PACK_BASE_URL);
        assert_eq!(resolve_base_url(Some("   ")), DEFAULT_PACK_BASE_URL);
        assert_eq!(
            resolve_base_url(Some("http://127.0.0.1:8099/packs/")),
            "http://127.0.0.1:8099/packs"
        );
        assert_eq!(
            resolve_base_url(Some("https://packs.example/moldova")),
            "https://packs.example/moldova"
        );
        // Non-HTTP schemes fall back instead of reaching reqwest.
        assert_eq!(
            resolve_base_url(Some("ftp://example/x")),
            DEFAULT_PACK_BASE_URL
        );
        assert_eq!(
            upstream_url(DEFAULT_PACK_BASE_URL, "moldova.pmtiles"),
            format!("{DEFAULT_PACK_BASE_URL}/moldova.pmtiles")
        );
    }

    #[test]
    fn committed_manifest_matches_the_release_and_is_same_origin() {
        let value: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        assert_eq!(value["id"], "moldova");
        assert_eq!(value["version"], "moldova-20261116");
        assert_eq!(value["bbox"], json!([26.6, 45.4, 30.2, 48.5]));
        assert_eq!(value["minzoom"], 0);
        assert_eq!(value["maxzoom"], 14);
        assert_eq!(value["license"], "ODbL-1.0");
        assert!(value["attribution"]
            .as_str()
            .unwrap()
            .contains("OpenStreetMap"));

        let files = value["files"].as_array().unwrap();
        assert_eq!(files.len(), PACK_FILES.len());
        let expected = [
            (
                "moldova.pmtiles",
                93_172_399_u64,
                "7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e",
            ),
            (
                "moldova-search.sqlite",
                6_770_688,
                "c576be58672ab39118b4ae12b07600c2632b081945dfeb2be28f4820b8d78c82",
            ),
            (
                "moldova-valhalla.tar",
                120_319_488,
                "a9fce80fae0e6fea566ee215d916bf98a0af909253fe81b6dedc1c6383fe3fa8",
            ),
        ];
        for (entry, (name, bytes, sha256)) in files.iter().zip(expected) {
            assert_eq!(entry["name"], name);
            assert_eq!(entry["bytes"], bytes);
            assert_eq!(entry["sha256"], sha256);
            assert_eq!(entry["url"], format!("/offline/packs/moldova/{name}"));
            assert!(
                !entry["url"].as_str().unwrap().contains("github.com"),
                "{name} still points at the release host"
            );
        }
    }

    #[tokio::test]
    async fn manifest_route_serves_same_origin_json() {
        let resp = app()
            .oneshot(get_req("/offline/packs/moldova/manifest.json"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json; charset=utf-8"
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
        assert_eq!(
            resp.headers().get(header::ACCEPT_RANGES),
            None,
            "the manifest is not a range asset"
        );
        let headers = resp.headers().clone();
        assert_eq!(
            headers.get(header::ACCESS_CONTROL_EXPOSE_HEADERS).unwrap(),
            EXPOSE_HEADERS
        );
        let value = body_json(resp).await;
        for file in value["files"].as_array().unwrap() {
            assert!(file["url"]
                .as_str()
                .unwrap()
                .starts_with("/offline/packs/moldova/"));
        }
    }

    #[tokio::test]
    async fn unknown_pack_and_traversal_are_404() {
        for uri in [
            "/offline/packs/other/manifest.json",
            "/offline/packs/other/moldova.pmtiles",
            // Router-level: extra path segments cannot match `:pack/:file`.
            "/offline/packs/moldova/../../etc/passwd",
            // Handler-level: percent-decoded traversal is not whitelisted.
            "/offline/packs/moldova/..%2F..%2Fetc%2Fpasswd",
            "/offline/packs/moldova/manifest.json/extra",
            "/offline/packs/moldova/unknown.bin",
        ] {
            let resp = app().oneshot(get_req(uri)).await.unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::NOT_FOUND,
                "{uri} was not rejected"
            );
        }
    }

    /// Local stand-in for the release host. A `bytes=0-7` range returns the
    /// real PMTiles magic plus the version byte (8 bytes), so the fixture
    /// matches what the GitHub asset actually serves. Axum serves `HEAD` from
    /// the same handler, proving a HEAD response carries length and no body.
    async fn mock_release() -> String {
        use axum::response::IntoResponse;
        let mock = Router::new().route(
            "/:file",
            get(|headers: HeaderMap| async move {
                let range = headers
                    .get(header::RANGE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if range == "bytes=0-7" {
                    (
                        StatusCode::PARTIAL_CONTENT,
                        [
                            (header::CONTENT_RANGE, "bytes 0-7/93172399"),
                            (header::CONTENT_LENGTH, "8"),
                            (header::ACCEPT_RANGES, "bytes"),
                        ],
                        b"PMTiles\x03".to_vec(),
                    )
                        .into_response()
                } else {
                    (
                        StatusCode::OK,
                        [
                            (header::CONTENT_LENGTH, "9"),
                            (header::ACCEPT_RANGES, "bytes"),
                        ],
                        "fake-pack",
                    )
                        .into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, mock).await.unwrap();
        });
        format!("http://{addr}")
    }

    struct EnvGuard;

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var("PINPOINT_PACK_BASE_URL");
        }
    }

    /// `PINPOINT_PACK_BASE_URL` is process-global; the two upstream tests take
    /// this lock so they cannot observe each other's fixture base URL.
    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn asset_route_forwards_range_and_streams() {
        let _serial = ENV_LOCK.lock().await;
        let base = mock_release().await;
        std::env::set_var("PINPOINT_PACK_BASE_URL", format!("{base}/"));
        let _guard = EnvGuard;

        let resp = app()
            .oneshot(get_range(
                "/offline/packs/moldova/moldova.pmtiles",
                "bytes=0-7",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            resp.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 0-7/93172399"
        );
        assert_eq!(resp.headers().get(header::CONTENT_LENGTH).unwrap(), "8");
        assert_eq!(resp.headers().get(header::ACCEPT_RANGES).unwrap(), "bytes");
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/vnd.pmtiles"
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_EXPOSE_HEADERS)
                .unwrap(),
            EXPOSE_HEADERS
        );
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(bytes.len(), 8, "bytes=0-7 is eight bytes");
        assert_eq!(&bytes[..7], &b"PMTiles"[..]);
    }

    #[tokio::test]
    async fn head_route_returns_length_without_a_body() {
        let _serial = ENV_LOCK.lock().await;
        let base = mock_release().await;
        std::env::set_var("PINPOINT_PACK_BASE_URL", base);
        let _guard = EnvGuard;

        let req = axum::http::Request::builder()
            .method(Method::HEAD)
            .uri("/offline/packs/moldova/moldova-valhalla.tar")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get(header::CONTENT_LENGTH).unwrap(), "9");
        assert_eq!(resp.headers().get(header::CONTENT_RANGE), None);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-tar"
        );
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(bytes.is_empty(), "HEAD must not stream the artifact");
    }

    #[tokio::test]
    async fn preflight_allows_range_for_cross_origin_webviews() {
        let req = axum::http::Request::builder()
            .method(Method::OPTIONS)
            .uri("/offline/packs/moldova/moldova-search.sqlite")
            .body(Body::empty())
            .unwrap();
        let resp = app().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_METHODS)
                .unwrap(),
            "GET, HEAD, OPTIONS"
        );
        assert_eq!(
            resp.headers()
                .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
                .unwrap(),
            "Range, If-Range"
        );
    }
}

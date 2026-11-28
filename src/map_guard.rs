//! Request guard for the loopback map server (threat-model finding F-7).
//!
//! Any app on the device can reach `127.0.0.1`, and a browser tab can send
//! cross-origin requests to it. So every request must name this server in its
//! `Host` header (this blocks DNS rebinding), and a browser `Origin` must be
//! this server's own origin. Requests that change state (`POST`, `DELETE`)
//! also need a per-launch token.
//!
//! The token reaches only the app's own WebView: it travels in the URL
//! fragment, which is never sent over the network and which other apps cannot
//! read. The map page moves it out of the address bar and sends it back in
//! the `X-Pinpoint-Map-Token` header.

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use rand::RngCore as _;

/// Header the map page sends with every state-changing request.
pub const TOKEN_HEADER: &str = "x-pinpoint-map-token";

/// Fragment key that carries the token into the app's own WebView.
const FRAGMENT_KEY: &str = "pinpoint-map-token";

/// A fresh random token, 256 bits, URL-safe. Call once per server start.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// The URL fragment that hands `token` to the WebView, for example `#pinpoint-map-token=…`.
pub fn fragment(token: &str) -> String {
    format!("#{FRAGMENT_KEY}={token}")
}

/// Checks that each request names this server and, for state changes, carries the token.
#[derive(Clone)]
pub struct MapGuard {
    port: u16,
    token: String,
}

impl MapGuard {
    /// Guard for a server bound to `127.0.0.1:port`.
    pub fn new(port: u16, token: String) -> Self {
        Self { port, token }
    }

    fn host_allowed(&self, headers: &HeaderMap) -> bool {
        headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .map(|host| {
                let host = host.to_ascii_lowercase();
                host == format!("127.0.0.1:{}", self.port)
                    || host == format!("localhost:{}", self.port)
            })
            .unwrap_or(false)
    }

    fn origin_allowed(&self, origin: &HeaderValue) -> bool {
        origin.to_str().ok().is_some_and(|origin| {
            origin == format!("http://127.0.0.1:{}", self.port)
                || origin == format!("http://localhost:{}", self.port)
        })
    }

    fn token_matches(&self, headers: &HeaderMap) -> bool {
        headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|sent| constant_time_eq(sent.as_bytes(), self.token.as_bytes()))
    }
}

/// Compares two byte strings without stopping at the first difference.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn is_state_changing(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// Axum middleware: `Host` on every request, then `Origin` and the token on state changes.
pub async fn enforce(guard: MapGuard, request: Request<Body>, next: Next) -> Response {
    if !guard.host_allowed(request.headers()) {
        return forbidden();
    }
    if is_state_changing(request.method()) {
        if let Some(origin) = request.headers().get(header::ORIGIN) {
            if !guard.origin_allowed(origin) {
                return forbidden();
            }
        }
        if !guard.token_matches(request.headers()) {
            return forbidden();
        }
    }
    next.run(request).await
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "forbidden").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request as HttpRequest;
    use axum::middleware::from_fn;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt as _;

    const PORT: u16 = 41234;
    const TOKEN: &str = "test-token";

    fn app() -> Router {
        let guard = MapGuard::new(PORT, TOKEN.to_string());
        Router::new()
            .route("/x", get(|| async { "read" }).post(|| async { "wrote" }))
            .layer(from_fn(move |request, next| {
                enforce(guard.clone(), request, next)
            }))
    }

    async fn status(request: HttpRequest<Body>) -> StatusCode {
        app().oneshot(request).await.expect("router").status()
    }

    fn request(method: &str, host: &str) -> HttpRequest<Body> {
        HttpRequest::builder()
            .method(method)
            .uri("/x")
            .header(header::HOST, host)
            .body(Body::empty())
            .expect("request")
    }

    #[tokio::test]
    async fn reads_on_the_loopback_host_need_no_token() {
        let host = format!("127.0.0.1:{PORT}");
        assert_eq!(status(request("GET", &host)).await, StatusCode::OK);
        let localhost = format!("localhost:{PORT}");
        assert_eq!(status(request("GET", &localhost)).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_host_that_is_not_this_server_is_refused() {
        assert_eq!(
            status(request("GET", "evil.example:41234")).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status(request("GET", "127.0.0.1")).await,
            StatusCode::FORBIDDEN,
            "a Host without this server's port is not this server"
        );
    }

    #[tokio::test]
    async fn a_state_change_needs_the_token() {
        let host = format!("127.0.0.1:{PORT}");
        assert_eq!(
            status(request("POST", &host)).await,
            StatusCode::FORBIDDEN,
            "POST without the token is refused"
        );

        let mut with_token = request("POST", &host);
        with_token
            .headers_mut()
            .insert(TOKEN_HEADER, HeaderValue::from_static(TOKEN));
        assert_eq!(status(with_token).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_wrong_token_is_refused() {
        let host = format!("127.0.0.1:{PORT}");
        let mut wrong = request("POST", &host);
        wrong
            .headers_mut()
            .insert(TOKEN_HEADER, HeaderValue::from_static("guessed"));
        assert_eq!(status(wrong).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_foreign_origin_is_refused_even_with_the_token() {
        let host = format!("127.0.0.1:{PORT}");
        let mut browser = request("POST", &host);
        browser
            .headers_mut()
            .insert(TOKEN_HEADER, HeaderValue::from_static(TOKEN));
        browser.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_static("https://attacker.example"),
        );
        assert_eq!(status(browser).await, StatusCode::FORBIDDEN);

        let mut own = request("POST", &host);
        own.headers_mut()
            .insert(TOKEN_HEADER, HeaderValue::from_static(TOKEN));
        own.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_str(&format!("http://127.0.0.1:{PORT}")).expect("origin"),
        );
        assert_eq!(status(own).await, StatusCode::OK);
    }

    #[test]
    fn the_fragment_carries_the_token_and_the_comparison_is_exact() {
        assert_eq!(fragment("abc"), "#pinpoint-map-token=abc");
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }

    #[test]
    fn new_tokens_are_long_and_differ() {
        let first = new_token();
        let second = new_token();
        assert!(first.len() >= 43, "256 bits in base64url");
        assert_ne!(first, second);
    }
}

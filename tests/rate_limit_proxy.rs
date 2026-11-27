//! Regression: a client cannot choose its own login bucket by sending
//! `X-Forwarded-For`. The limit follows the socket address unless the server is
//! marked as running behind a trusted proxy (`TRUST_PROXY`).

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use pinpoint_desktop::server::{router, AppState};
use std::net::SocketAddr;
use tower::ServiceExt;

async fn login_from(app: &axum::Router, peer: SocketAddr, forwarded: &str) -> StatusCode {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", forwarded)
        .body(Body::from(
            r#"{"email":"nobody@example.com","password":"wrong"}"#,
        ))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    app.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn rotating_forwarded_for_does_not_escape_the_login_limit() {
    let app = router(AppState::new());
    let peer: SocketAddr = "198.51.100.4:50000".parse().unwrap();
    // The login limit is 30 attempts per window per client.
    for i in 0..30 {
        let forwarded = format!("203.0.113.{}", i + 1);
        assert_ne!(
            login_from(&app, peer, &forwarded).await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    assert_eq!(
        login_from(&app, peer, "203.0.113.250").await,
        StatusCode::TOO_MANY_REQUESTS,
        "a new X-Forwarded-For must not reset the limit for the socket peer"
    );
}

#[tokio::test]
async fn trusted_proxy_header_selects_the_client_bucket() {
    let mut state = AppState::new();
    state.trust_proxy = true;
    let app = router(state);
    let peer: SocketAddr = "172.18.0.2:50000".parse().unwrap();
    // Behind the gateway, each forwarded client gets its own bucket.
    for i in 0..31 {
        let forwarded = format!("203.0.113.{}", i + 1);
        assert_ne!(
            login_from(&app, peer, &forwarded).await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
}

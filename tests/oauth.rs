//! OAuth 2.0 sign-in against a local mock identity provider.
//!
//! The provider endpoints are local axum servers, so the test exercises the
//! real authorization-code + PKCE exchange without external credentials.

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use pinpoint_desktop::security::oauth::{OAuthClient, OAuthProvider};
use pinpoint_desktop::server::{router, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn spawn_idp(email: &str, verified: bool) -> String {
    let email = email.to_string();
    let app = Router::new()
        .route(
            "/token",
            post(|| async { Json(json!({ "access_token": "test-token" })) }),
        )
        .route(
            "/userinfo",
            get(move || {
                let email = email.clone();
                async move {
                    Json(json!({
                        "sub": "subject-1",
                        "email": email,
                        "email_verified": verified,
                        "name": "OAuth User"
                    }))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock idp");
    let addr = listener.local_addr().expect("idp address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

fn state_with_provider(idp: &str) -> AppState {
    let state = AppState::new();
    let mut provider = OAuthProvider::google("test-client-id".into());
    provider.authorize_url = format!("{idp}/authorize");
    provider.token_url = format!("{idp}/token");
    state.set_oauth_client(
        OAuthClient {
            provider,
            userinfo_url: format!("{idp}/userinfo"),
            email_url: None,
            redirect_uri: "http://127.0.0.1:8080/api/v1/auth/oauth/google/callback".into(),
        },
        "test-secret".into(),
    );
    state
}

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.expect("router response")
}

fn header(resp: &Response, name: &str) -> String {
    resp.headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn cookie_with_prefix(resp: &Response, prefix: &str) -> Option<String> {
    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with(prefix))
        .map(|value| value.split(';').next().unwrap_or_default().to_string())
}

async fn start_flow(app: &Router, provider: &str) -> (String, String) {
    let resp = send(
        app,
        Request::builder()
            .uri(format!("/api/v1/auth/oauth/{provider}/start"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert!(
        resp.status().is_redirection(),
        "start must redirect, got {}",
        resp.status()
    );
    let location = header(&resp, "location");
    let state = url::Url::parse(&location)
        .expect("provider URL")
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.to_string())
        .expect("state parameter");
    assert!(location.contains("code_challenge_method=S256"));
    let cookie = cookie_with_prefix(&resp, "pinpoint_oauth_state=").expect("state cookie");
    (state, cookie)
}

async fn callback(app: &Router, provider: &str, state: &str, cookie: &str) -> Response {
    send(
        app,
        Request::builder()
            .uri(format!(
                "/api/v1/auth/oauth/{provider}/callback?code=test-code&state={state}"
            ))
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

async fn me(app: &Router, cookie: &str) -> (StatusCode, Value) {
    let resp = send(
        app,
        Request::builder()
            .uri("/api/v1/auth/me")
            .header(header::COOKIE, cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

#[tokio::test]
async fn oauth_creates_an_account_and_reuses_it_on_the_next_sign_in() {
    let idp = spawn_idp("oauth@example.invalid", true).await;
    let state = state_with_provider(&idp);
    let app = router(state);

    let (flow_state, cookie) = start_flow(&app, "google").await;
    let resp = callback(&app, "google", &flow_state, &cookie).await;
    assert!(resp.status().is_redirection());
    assert_eq!(header(&resp, "location"), "/");
    let session = cookie_with_prefix(&resp, "session_id=").expect("session cookie");
    let (status, body) = me(&app, &session).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], "oauth@example.invalid");
    assert_eq!(body["auth_method"], "oauth");
    let first_id = body["id"].as_i64().expect("user id");

    let (flow_state, cookie) = start_flow(&app, "google").await;
    let resp = callback(&app, "google", &flow_state, &cookie).await;
    let session = cookie_with_prefix(&resp, "session_id=").expect("session cookie");
    let (_, body) = me(&app, &session).await;
    assert_eq!(body["id"].as_i64(), Some(first_id));
}

#[tokio::test]
async fn oauth_refuses_an_email_that_already_has_a_password_account() {
    let idp = spawn_idp("taken@example.invalid", true).await;
    let state = state_with_provider(&idp);
    let app = router(state);
    let body = json!({
        "email": "taken@example.invalid",
        "password": "StrongPass!2026",
        "full_name": "Password User"
    });
    let resp = send(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let (flow_state, cookie) = start_flow(&app, "google").await;
    let resp = callback(&app, "google", &flow_state, &cookie).await;
    assert!(resp.status().is_redirection());
    assert!(
        header(&resp, "location").contains("oauth_error=email_exists"),
        "got {}",
        header(&resp, "location")
    );
    assert!(cookie_with_prefix(&resp, "session_id=").is_none());
}

#[tokio::test]
async fn oauth_refuses_an_unverified_provider_email() {
    let idp = spawn_idp("unverified@example.invalid", false).await;
    let state = state_with_provider(&idp);
    let app = router(state);
    let (flow_state, cookie) = start_flow(&app, "google").await;
    let resp = callback(&app, "google", &flow_state, &cookie).await;
    assert!(header(&resp, "location").contains("oauth_error=email_unverified"));
    assert!(cookie_with_prefix(&resp, "session_id=").is_none());
}

#[tokio::test]
async fn oauth_refuses_a_foreign_or_replayed_state() {
    let idp = spawn_idp("state@example.invalid", true).await;
    let state = state_with_provider(&idp);
    let app = router(state);

    let (flow_state, _) = start_flow(&app, "google").await;
    let resp = callback(&app, "google", &flow_state, "pinpoint_oauth_state=wrong").await;
    assert!(header(&resp, "location").contains("oauth_error=state_mismatch"));
    assert!(cookie_with_prefix(&resp, "session_id=").is_none());

    // A fresh flow: the first callback consumes the state, the replay fails.
    let (flow_state, cookie) = start_flow(&app, "google").await;
    let first = callback(&app, "google", &flow_state, &cookie).await;
    assert!(first.status().is_redirection());
    let replay = callback(&app, "google", &flow_state, &cookie).await;
    assert!(header(&replay, "location").contains("oauth_error=state_mismatch"));
    assert!(cookie_with_prefix(&replay, "session_id=").is_none());
}

async fn spawn_github_idp(email: &str, verified: bool) -> String {
    let primary = email.to_string();
    let secondary = "secondary@example.invalid".to_string();
    let app = Router::new()
        .route(
            "/token",
            post(|| async { Json(json!({ "access_token": "test-token" })) }),
        )
        .route(
            "/user",
            get(|| async { Json(json!({ "id": 4242, "login": "octocat", "name": "Mona Lisa" })) }),
        )
        .route(
            "/emails",
            get(move || {
                let primary = primary.clone();
                let secondary = secondary.clone();
                async move {
                    Json(json!([
                        { "email": secondary, "primary": false, "verified": true },
                        { "email": primary, "primary": true, "verified": verified }
                    ]))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock github");
    let addr = listener.local_addr().expect("github address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

fn state_with_github(idp: &str) -> AppState {
    let state = AppState::new();
    let mut provider = OAuthProvider::github("test-client-id".into());
    provider.authorize_url = format!("{idp}/authorize");
    provider.token_url = format!("{idp}/token");
    state.set_oauth_client(
        OAuthClient {
            provider,
            userinfo_url: format!("{idp}/user"),
            email_url: Some(format!("{idp}/emails")),
            redirect_uri: "http://127.0.0.1:8080/api/v1/auth/oauth/github/callback".into(),
        },
        "test-secret".into(),
    );
    state
}

#[tokio::test]
async fn github_oauth_uses_the_primary_verified_email() {
    let idp = spawn_github_idp("octo@example.invalid", true).await;
    let app = router(state_with_github(&idp));
    let (flow_state, cookie) = start_flow(&app, "github").await;
    let resp = callback(&app, "github", &flow_state, &cookie).await;
    assert_eq!(header(&resp, "location"), "/");
    let session = cookie_with_prefix(&resp, "session_id=").expect("session cookie");
    let (status, body) = me(&app, &session).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], "octo@example.invalid");
    assert_eq!(body["full_name"], "Mona Lisa");
    assert_eq!(body["auth_method"], "oauth");
    let first_id = body["id"].as_i64().expect("user id");

    let (flow_state, cookie) = start_flow(&app, "github").await;
    let resp = callback(&app, "github", &flow_state, &cookie).await;
    let session = cookie_with_prefix(&resp, "session_id=").expect("session cookie");
    let (_, body) = me(&app, &session).await;
    assert_eq!(body["id"].as_i64(), Some(first_id));
}

#[tokio::test]
async fn github_oauth_refuses_an_unverified_primary_email() {
    let idp = spawn_github_idp("octo@example.invalid", false).await;
    let app = router(state_with_github(&idp));
    let (flow_state, cookie) = start_flow(&app, "github").await;
    let resp = callback(&app, "github", &flow_state, &cookie).await;
    assert!(header(&resp, "location").contains("oauth_error=email_unverified"));
    assert!(cookie_with_prefix(&resp, "session_id=").is_none());
}

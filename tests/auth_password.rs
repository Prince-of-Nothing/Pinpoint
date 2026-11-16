//! SEC-2: password changes must prove the current password.
//!
//! These tests drive the real in-process router (in-memory store) so the
//! session-cookie + CSRF middleware is exercised, not just the handler body.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

const REGISTER: &str =
    r#"{"email":"sec2@example.invalid","password":"OldPass123!","full_name":"Alice"}"#;

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn json_req(method: &str, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn authed_req(method: &str, uri: &str, cookie: &str, csrf: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("Content-Type", "application/json")
        .header("Cookie", cookie)
        .header("X-CSRF-Token", csrf)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn password_change_requires_current_password() {
    let state = pinpoint_desktop::server::AppState::new();
    let app = pinpoint_desktop::server::router(state);

    // Register and capture the session cookie + CSRF token.
    let resp = app
        .clone()
        .oneshot(json_req("POST", "/api/v1/auth/register", REGISTER))
        .await
        .unwrap();
    let status = resp.status();
    let raw_cookie = resp
        .headers()
        .get("set-cookie")
        .map(|value| value.to_str().unwrap().to_string());
    let body = body_json(resp).await;
    assert_eq!(status, StatusCode::CREATED, "register failed: {body}");
    let raw_cookie = raw_cookie.expect("session cookie");
    let cookie = raw_cookie.split(';').next().unwrap().to_string();
    let csrf = body["csrf_token"].as_str().expect("csrf token").to_string();

    // Wrong current password must be rejected and must not revoke the session.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            "/api/v1/auth/password/change",
            &cookie,
            &csrf,
            r#"{"current_password":"WrongPass123!","new_password":"NewPass456!","confirm_password":"NewPass456!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "wrong current password must be rejected"
    );
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("Cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "rejected change must keep the session valid"
    );

    // Missing current password must also be rejected.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            "/api/v1/auth/password/change",
            &cookie,
            &csrf,
            r#"{"new_password":"NewPass456!","confirm_password":"NewPass456!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "missing current password must be rejected"
    );

    // Correct current password succeeds and revokes every session.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            "/api/v1/auth/password/change",
            &cookie,
            &csrf,
            r#"{"current_password":"OldPass123!","new_password":"NewPass456!","confirm_password":"NewPass456!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("Cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "password change must revoke the old session"
    );

    // Old password no longer works; the new one does.
    let resp = app
        .clone()
        .oneshot(json_req(
            "POST",
            "/api/v1/auth/login",
            r#"{"email":"sec2@example.invalid","password":"OldPass123!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = app
        .oneshot(json_req(
            "POST",
            "/api/v1/auth/login",
            r#"{"email":"sec2@example.invalid","password":"NewPass456!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

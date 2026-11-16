//! Self-service account deletion: confirmation, password proof and cascade.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

const REGISTER: &str =
    r#"{"email":"delete@example.invalid","password":"DeletePass123!","full_name":"Delete Me"}"#;

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
async fn self_deletion_requires_confirmation_and_password() {
    let state = pinpoint_desktop::server::AppState::new();
    let app = pinpoint_desktop::server::router(state.clone());

    let resp = app
        .clone()
        .oneshot(json_req("POST", "/api/v1/auth/register", REGISTER))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let raw_cookie = resp
        .headers()
        .get("set-cookie")
        .expect("session cookie")
        .to_str()
        .unwrap()
        .to_string();
    let cookie = raw_cookie.split(';').next().unwrap().to_string();
    let body = body_json(resp).await;
    let csrf = body["csrf_token"].as_str().expect("csrf").to_string();
    let user_id = body["user"]["id"].as_i64().expect("user id");

    // Wrong confirmation phrase is rejected before any deletion.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "DELETE",
            "/api/v1/users/me",
            &cookie,
            &csrf,
            r#"{"confirm":"delete","current_password":"DeletePass123!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Wrong current password is rejected and the session stays valid.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "DELETE",
            "/api/v1/users/me",
            &cookie,
            &csrf,
            r#"{"confirm":"DELETE","current_password":"WrongPass123!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
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
    assert_eq!(resp.status(), StatusCode::OK);

    // Correct confirmation + password deletes the account and revokes sessions.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "DELETE",
            "/api/v1/users/me",
            &cookie,
            &csrf,
            r#"{"confirm":"DELETE","current_password":"DeletePass123!"}"#,
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
        "deleted account must not keep a session"
    );

    let resp = app
        .oneshot(json_req(
            "POST",
            "/api/v1/auth/login",
            r#"{"email":"delete@example.invalid","password":"DeletePass123!"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "deleted account must not be able to sign in"
    );

    let store = state.store.lock().unwrap();
    assert!(
        store.users.iter().all(|u| u.id != user_id),
        "user row must be removed"
    );
}

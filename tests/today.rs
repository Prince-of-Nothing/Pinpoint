//! Private daily trail and place notes: owner-only, fail closed.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tower::ServiceExt;

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

async fn register(app: &Router, email: &str, name: &str) -> (String, String, i64) {
    let body =
        format!(r#"{{"email":"{email}","password":"LocationPass123!","full_name":"{name}"}}"#);
    let resp = app
        .clone()
        .oneshot(json_req("POST", "/api/v1/auth/register", &body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let body = body_json(resp).await;
    (
        cookie,
        body["csrf_token"].as_str().unwrap().to_string(),
        body["user"]["id"].as_i64().unwrap(),
    )
}

async fn get_json(app: &Router, uri: &str, cookie: &str) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("Cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    body_json(resp).await
}

#[tokio::test]
async fn trail_is_owner_only_and_records_each_publish() {
    let state = pinpoint_desktop::server::AppState::new();
    let app = pinpoint_desktop::server::router(state);
    let (alice_cookie, alice_csrf, _) =
        register(&app, "trail-alice@example.invalid", "Alice").await;
    let (bob_cookie, _, _) = register(&app, "trail-bob@example.invalid", "Bob").await;

    for (lat, lon) in [(47.0105, 28.8638), (47.0245, 28.8322)] {
        let resp = app
            .clone()
            .oneshot(authed_req(
                "POST",
                "/api/v1/locations/me",
                &alice_cookie,
                &alice_csrf,
                &format!(r#"{{"latitude":{lat},"longitude":{lon},"accuracy":5.0}}"#),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let points = get_json(&app, "/api/v1/locations/history", &alice_cookie).await;
    assert_eq!(points.as_array().expect("array").len(), 2);
    assert_eq!(points[0]["latitude"], 47.0105);
    assert_eq!(points[1]["longitude"], 28.8322);

    let other = get_json(&app, "/api/v1/locations/history", &bob_cookie).await;
    assert_eq!(other, json!([]));

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/locations/history")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn place_notes_are_owner_only_and_validated() {
    let state = pinpoint_desktop::server::AppState::new();
    let app = pinpoint_desktop::server::router(state);
    let (alice_cookie, alice_csrf, _) =
        register(&app, "notes-alice@example.invalid", "Alice").await;
    let (bob_cookie, bob_csrf, _) = register(&app, "notes-bob@example.invalid", "Bob").await;

    let photo = "data:image/png;base64,iVBORw0KGgo=";
    let body = json!({
        "latitude": 47.0245,
        "longitude": 28.8322,
        "label": "Cafe",
        "message": "The flat white here.",
        "photo": photo,
    });
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            "/api/v1/locations/notes",
            &alice_cookie,
            &alice_csrf,
            &body.to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    let id = created["id"].as_i64().expect("id");
    assert_eq!(created["message"], "The flat white here.");

    let own = get_json(&app, "/api/v1/locations/notes", &alice_cookie).await;
    assert_eq!(own.as_array().expect("array").len(), 1);
    let other = get_json(&app, "/api/v1/locations/notes", &bob_cookie).await;
    assert_eq!(other, json!([]));

    let resp = app
        .clone()
        .oneshot(authed_req(
            "DELETE",
            &format!("/api/v1/locations/notes/{id}"),
            &bob_cookie,
            &bob_csrf,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        get_json(&app, "/api/v1/locations/notes", &alice_cookie)
            .await
            .as_array()
            .expect("array")
            .len(),
        1
    );

    let resp = app
        .clone()
        .oneshot(authed_req(
            "DELETE",
            &format!("/api/v1/locations/notes/{id}"),
            &alice_cookie,
            &alice_csrf,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        get_json(&app, "/api/v1/locations/notes", &alice_cookie).await,
        json!([])
    );

    for invalid in [
        json!({"latitude": 47.0, "longitude": 28.8, "message": "no photo", "photo": "data:image/svg+xml;base64,PHN2Zz4="}),
        json!({"latitude": 47.0, "longitude": 28.8, "message": ""}),
        json!({"latitude": 200.0, "longitude": 28.8, "message": "bad point"}),
    ] {
        let resp = app
            .clone()
            .oneshot(authed_req(
                "POST",
                "/api/v1/locations/notes",
                &alice_cookie,
                &alice_csrf,
                &invalid.to_string(),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{invalid}");
    }

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/locations/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

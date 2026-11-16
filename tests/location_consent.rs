//! Server-side location consent: a friendship alone grants no visibility.

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

async fn register(app: &Router, email: &str, name: &str) -> (String, String, i64, String) {
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
        body["user"]["username"].as_str().unwrap().to_string(),
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
async fn friend_locations_require_an_active_share() {
    let state = pinpoint_desktop::server::AppState::new();
    let app = pinpoint_desktop::server::router(state.clone());
    let (alice_cookie, alice_csrf, _, _) =
        register(&app, "loc-alice@example.invalid", "Alice").await;
    let (bob_cookie, bob_csrf, _, bob_username) =
        register(&app, "loc-bob@example.invalid", "Bob").await;

    // Bob publishes while there is no friendship or share.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            "/api/v1/locations/me",
            &bob_cookie,
            &bob_csrf,
            r#"{"latitude":47.0105,"longitude":28.8638,"accuracy":10.0}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let visible = get_json(&app, "/api/v1/locations/friends", &alice_cookie).await;
    assert_eq!(visible, json!([]), "no consent means no locations");

    // Accepted friendship alone still grants nothing.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            "/api/v1/friendships/",
            &alice_cookie,
            &alice_csrf,
            &format!(r#"{{"receiver_username":"{bob_username}"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let friendship = body_json(resp).await;
    let friendship_id = friendship["id"].as_i64().expect("friendship id");
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            &format!("/api/v1/friendships/{friendship_id}/respond"),
            &bob_cookie,
            &bob_csrf,
            r#"{"action":"accept"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let visible = get_json(&app, "/api/v1/locations/friends", &alice_cookie).await;
    assert_eq!(
        visible,
        json!([]),
        "accepted friendship alone must not expose a location"
    );

    // Bob grants Alice an approximate share.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "POST",
            "/api/v1/locations/shares",
            &bob_cookie,
            &bob_csrf,
            &format!(
                r#"{{"friendship_id":{friendship_id},"direction":"to_friend","precision_level":"approximate"}}"#
            ),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let share = body_json(resp).await;
    let share_id = share["id"].as_i64().expect("share id");

    let visible = get_json(&app, "/api/v1/locations/friends", &alice_cookie).await;
    let items = visible.as_array().expect("array");
    assert_eq!(items.len(), 1, "one active share -> one location");
    assert_eq!(items[0]["precision_level"], "approximate");
    assert_eq!(items[0]["latitude"].as_f64().unwrap(), 47.01);
    assert_eq!(items[0]["longitude"].as_f64().unwrap(), 28.86);

    // Revoking the share hides it immediately.
    let resp = app
        .clone()
        .oneshot(authed_req(
            "PATCH",
            &format!("/api/v1/locations/shares/{share_id}"),
            &bob_cookie,
            &bob_csrf,
            r#"{"is_active":false}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let visible = get_json(&app, "/api/v1/locations/friends", &alice_cookie).await;
    assert_eq!(visible, json!([]), "revoked share must hide the location");

    // An expired share is also invisible.
    {
        let mut store = state.store.lock().unwrap();
        if let Some(s) = store.shares.iter_mut().find(|s| s.id == share_id) {
            s.is_active = true;
            s.expires_at = Some("2000-01-01T00:00:00Z".to_string());
        }
    }
    let visible = get_json(&app, "/api/v1/locations/friends", &alice_cookie).await;
    assert_eq!(visible, json!([]), "expired share must hide the location");
}

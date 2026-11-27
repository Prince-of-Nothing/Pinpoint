//! API contract tests for `ApiClient`.
//!
//! These pin the client to the Go server's documented behaviour
//! (`backend/internal/handlers`): the login envelope, session-cookie and
//! CSRF double-submit on unsafe methods, the error envelope, numeric IDs,
//! and fail-closed session handling. See root CONTRIBUTING.md: security
//! changes need a regression test *and* a test proving denied/invalid
//! cases stay denied.

mod common;

use common::{MockResponse, MockServer};
use pinpoint_desktop::api::ApiClient;

fn login_ok() -> MockResponse {
    MockResponse::json(
        200,
        &serde_json::json!({
            "access_token": "jwt-token",
            "token_type": "bearer",
            "expires_in": 900,
            "csrf_token": "csrf-abc",
            "user": {
                "id": 7,
                "email": "alice@example.com",
                "full_name": "Alice",
                "role": "resident",
                "is_active": true,
                "must_change_password": false,
                "created_at": "2026-01-01T00:00:00Z",
                "last_login_at": null
            }
        })
        .to_string(),
    )
    .with_cookie("session_id=abc123; Path=/; HttpOnly; SameSite=Lax")
    .with_cookie("at_csrf=csrf-abc; Path=/")
    .with_header("X-CSRF-Token", "csrf-abc")
}

fn error_envelope(status: u16, code: &str, message: &str) -> MockResponse {
    MockResponse::json(
        status,
        &serde_json::json!({
            "error": { "code": code, "message": message }
        })
        .to_string(),
    )
}

async fn login(server: &MockServer) -> ApiClient {
    let client = ApiClient::new(server.base_url.clone()).expect("client");
    let user = client
        .login("alice@example.com", "correct horse")
        .await
        .expect("login should succeed");
    assert_eq!(user.id, 7);
    assert_eq!(user.email, "alice@example.com");
    assert!(client.session_active());
    client
}

#[tokio::test]
async fn login_parses_user_and_stores_session_material() {
    let server = MockServer::start(vec![login_ok(), MockResponse::json(200, "[]")]);
    let client = login(&server).await;

    client.get_friendships().await.expect("friendships");

    let requests = server.requests();
    assert_eq!(requests.len(), 2);

    // The follow-up GET must carry the session cookie...
    let get = &requests[1];
    assert_eq!(get.method, "GET");
    let cookie = get
        .header("cookie")
        .expect("cookie header on authenticated GET");
    assert!(cookie.contains("session_id=abc123"), "cookie was: {cookie}");

    // ...but GETs are safe methods: no CSRF header is required or sent.
    assert!(
        get.header("x-csrf-token").is_none(),
        "safe methods must not send the CSRF header"
    );
}

#[tokio::test]
async fn unsafe_requests_send_csrf_double_submit() {
    let server = MockServer::start(vec![login_ok(), MockResponse::json(201, "{}")]);
    let client = login(&server).await;

    client
        .send_friend_request("calm-otter-42")
        .await
        .expect("friend request");

    let requests = server.requests();
    let post = &requests[1];
    assert_eq!(post.method, "POST");
    assert_eq!(post.path, "/api/v1/friendships/");

    // Cookie authenticates, X-CSRF-Token satisfies the double-submit check
    // in middleware.go for POST/PUT/PATCH/DELETE.
    let cookie = post.header("cookie").expect("cookie header");
    assert!(cookie.contains("session_id=abc123"));
    assert_eq!(post.header("x-csrf-token"), Some("csrf-abc"));

    let body: serde_json::Value = serde_json::from_str(&post.body).expect("json body");
    assert_eq!(body["receiver_username"], "calm-otter-42");
    // Friends are added by username only: no email field goes on the wire.
    assert!(body.get("receiver_email").is_none(), "body was: {body}");
}

#[tokio::test]
async fn csrf_falls_back_to_response_header_when_body_omits_it() {
    // Same contract, body without csrf_token: header mirror still works.
    let body = serde_json::json!({
        "access_token": "jwt-token",
        "user": {
            "id": 3,
            "email": "carol@example.com",
            "full_name": "Carol",
            "role": "resident",
            "is_active": true,
            "must_change_password": false
        }
    })
    .to_string();
    let server = MockServer::start(vec![
        MockResponse::json(200, &body)
            .with_cookie("session_id=zzz; Path=/")
            .with_header("X-CSRF-Token", "from-header"),
        MockResponse::json(201, "{}"),
    ]);

    let client = ApiClient::new(server.base_url.clone()).expect("client");
    client
        .login("carol@example.com", "pw")
        .await
        .expect("login");
    client
        .send_friend_request("dave-quiet-fox-7")
        .await
        .expect("request");

    let requests = server.requests();
    assert_eq!(requests[1].header("x-csrf-token"), Some("from-header"));
}

#[tokio::test]
async fn rejected_login_leaves_client_unauthenticated() {
    // Fail closed: a rejected login must not leave partial session state,
    // and later requests must go out unauthenticated.
    let server = MockServer::start(vec![
        error_envelope(401, "INVALID_CREDENTIALS", "Invalid email or password."),
        MockResponse::json(200, "[]"),
    ]);
    let client = ApiClient::new(server.base_url.clone()).expect("client");

    let err = client
        .login("alice@example.com", "wrong")
        .await
        .expect_err("login must fail");
    let msg = err.to_string();
    assert!(msg.contains("INVALID_CREDENTIALS"), "error was: {msg}");
    assert!(
        msg.contains("Invalid email or password"),
        "error was: {msg}"
    );
    assert!(!client.session_active());

    client
        .get_friendships()
        .await
        .expect("unauthenticated GET still issues");
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1].header("cookie").is_none(),
        "no session cookie may be sent after a failed login"
    );
    assert!(requests[1].header("x-csrf-token").is_none());
}

#[tokio::test]
async fn login_without_session_cookie_fails_closed() {
    // Regression: a 200 without Set-Cookie must not be treated as a
    // successful sign-in (the client would otherwise appear logged in
    // while every subsequent request is anonymous).
    let body = serde_json::json!({
        "access_token": "jwt-token",
        "csrf_token": "csrf-abc",
        "user": {
            "id": 7,
            "email": "alice@example.com",
            "full_name": "Alice",
            "role": "resident",
            "is_active": true,
            "must_change_password": false
        }
    })
    .to_string();
    let server = MockServer::start(vec![MockResponse::json(200, &body)]);
    let client = ApiClient::new(server.base_url.clone()).expect("client");

    let err = client
        .login("alice@example.com", "pw")
        .await
        .expect_err("must fail closed");
    assert!(
        err.to_string().contains("no session cookie"),
        "error was: {err}"
    );
    assert!(!client.session_active());
}

#[tokio::test]
async fn logout_failure_keeps_session_fail_closed() {
    // Regression for "logout ignores server failure": if the server does
    // not confirm the revocation, the client must not claim success or
    // silently drop the session.
    let server = MockServer::start(vec![
        login_ok(),
        error_envelope(500, "INTERNAL_ERROR", "An unexpected error occurred."),
        MockResponse::json(200, "[]"),
    ]);
    let client = login(&server).await;

    let err = client
        .logout()
        .await
        .expect_err("logout must surface failure");
    assert!(
        err.to_string().contains("INTERNAL_ERROR"),
        "error was: {err}"
    );
    assert!(
        client.session_active(),
        "session must survive a failed logout"
    );

    client.get_friendships().await.expect("next request");
    let requests = server.requests();
    let get = &requests[2];
    assert!(
        get.header("cookie")
            .is_some_and(|c| c.contains("session_id=abc123")),
        "session cookie must still be sent after a failed logout"
    );
}

#[tokio::test]
async fn confirmed_logout_clears_session() {
    let server = MockServer::start(vec![
        login_ok(),
        MockResponse::json(200, r#"{"message":"Successfully logged out"}"#),
        MockResponse::json(200, "[]"),
    ]);
    let client = login(&server).await;

    client.logout().await.expect("logout");
    assert!(!client.session_active());

    client.get_friendships().await.expect("next request");
    let requests = server.requests();
    assert!(
        requests[2].header("cookie").is_none(),
        "no cookie may be sent after a confirmed logout"
    );
}

#[tokio::test]
async fn create_share_sends_numeric_friendship_id() {
    // decodeJSON on the server is strict: friendship_id must be a JSON
    // number (i64), not the string form the old client sent.
    let server = MockServer::start(vec![login_ok(), MockResponse::json(201, "{}")]);
    let client = login(&server).await;

    client
        .create_location_share(42, "to_friend", "exact")
        .await
        .expect("create share");

    let requests = server.requests();
    let post = &requests[1];
    assert_eq!(post.path, "/api/v1/locations/shares");
    assert_eq!(post.header("x-csrf-token"), Some("csrf-abc"));
    let body: serde_json::Value = serde_json::from_str(&post.body).expect("json body");
    assert!(
        body["friendship_id"].is_i64(),
        "friendship_id must be numeric: {body}"
    );
    assert_eq!(body["friendship_id"], 42);
    assert_eq!(body["direction"], "to_friend");
    assert_eq!(body["precision_level"], "exact");
}

#[tokio::test]
async fn error_envelope_is_surfaced_readably() {
    let server = MockServer::start(vec![
        login_ok(),
        error_envelope(403, "CSRF_FAILED", "Missing or invalid CSRF token."),
    ]);
    let client = login(&server).await;

    let err = client
        .send_friend_request("calm-otter-42")
        .await
        .expect_err("must fail");
    let msg = err.to_string();
    assert!(msg.contains("CSRF_FAILED"), "error was: {msg}");
    assert!(
        msg.contains("Missing or invalid CSRF token"),
        "error was: {msg}"
    );
}

#[tokio::test]
async fn publish_location_sends_float_accuracy() {
    // Server schema: accuracy is *float64 — integers must not break it and
    // values must arrive as JSON numbers.
    let server = MockServer::start(vec![
        login_ok(),
        MockResponse::json(200, r#"{"message":"ok","shares_updated":1}"#),
    ]);
    let client = login(&server).await;

    client
        .publish_location(47.0105, 28.8612, 10.0)
        .await
        .expect("publish");

    let requests = server.requests();
    let body: serde_json::Value = serde_json::from_str(&requests[1].body).expect("json body");
    assert!(body["latitude"].is_f64());
    assert!(body["longitude"].is_f64());
    assert!(body["accuracy"].is_number());
    assert_eq!(requests[1].header("x-csrf-token"), Some("csrf-abc"));
}

#[tokio::test]
async fn today_endpoints_are_owner_paths_with_csrf_on_writes() {
    let history = r#"[{"id":1,"user_id":7,"latitude":47.0105,"longitude":28.8638,"accuracy":5.0,"recorded_at":"2026-11-18T00:00:00+00:00"}]"#;
    let note = r#"{"id":2,"user_id":7,"latitude":47.0245,"longitude":28.8322,"label":"Cafe","message":"Flat white","photo":null,"created_at":"2026-11-18T00:01:00+00:00"}"#;
    let server = MockServer::start(vec![
        login_ok(),
        MockResponse::json(200, history),
        MockResponse::json(200, &format!("[{note}]")),
        MockResponse::json(201, note),
        MockResponse::json(200, r#"{"message":"Note deleted."}"#),
    ]);
    let client = login(&server).await;

    let points = client.get_location_history().await.expect("history");
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].latitude, 47.0105);
    let notes = client.get_place_notes().await.expect("notes");
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].label, "Cafe");
    let created = client
        .create_place_note(47.0245, 28.8322, "Cafe", "Flat white", None)
        .await
        .expect("create note");
    assert_eq!(created.id, 2);
    client
        .delete_place_note(created.id)
        .await
        .expect("delete note");

    let requests = server.requests();
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].path, "/api/v1/locations/history");
    assert!(requests[1].header("x-csrf-token").is_none());
    assert_eq!(requests[2].path, "/api/v1/locations/notes");
    assert_eq!(requests[3].method, "POST");
    assert_eq!(requests[3].path, "/api/v1/locations/notes");
    assert_eq!(requests[3].header("x-csrf-token"), Some("csrf-abc"));
    let body: serde_json::Value = serde_json::from_str(&requests[3].body).expect("json body");
    assert_eq!(body["label"], "Cafe");
    assert_eq!(body["message"], "Flat white");
    assert!(body.get("photo").is_none());
    assert_eq!(requests[4].method, "DELETE");
    assert_eq!(requests[4].path, "/api/v1/locations/notes/2");
    assert_eq!(requests[4].header("x-csrf-token"), Some("csrf-abc"));
}

#[tokio::test]
async fn mfa_disable_posts_the_code_with_csrf() {
    let server = MockServer::start(vec![login_ok(), MockResponse::json(200, "{}")]);
    let client = login(&server).await;
    client.mfa_disable("123456").await.expect("disable MFA");

    let requests = server.requests();
    let post = &requests[1];
    assert_eq!(post.method, "POST");
    assert_eq!(post.path, "/api/v1/auth/mfa/disable");
    assert_eq!(post.header("x-csrf-token"), Some("csrf-abc"));
    let body: serde_json::Value = serde_json::from_str(&post.body).expect("json body");
    assert_eq!(body["code"], "123456");
}

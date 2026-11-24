//! In-process router probe: proves which routes match (no network needed).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use pinpoint_desktop::models::{Friendship, User};
use tower::ServiceExt;

/// Test-only fixture: the production store no longer seeds demo accounts.
fn seed_test_users(state: &pinpoint_desktop::server::AppState) {
    let mut store = state.store.lock().unwrap();
    if !store.users.is_empty() {
        return;
    }
    let mk = |id: i64, email: &str, name: &str, role: &str| User {
        id,
        username: String::new(),
        email: email.to_string(),
        full_name: name.to_string(),
        role: role.to_string(),
        is_active: true,
        must_change_password: false,
        password_hash: None,
        totp_secret: None,
        mfa_enabled: false,
        mfa_last_step: 0,
        failed_login_attempts: 0,
        locked_until: None,
        auth_method: "email".to_string(),
        public_key: None,
        recovery_code_hash: None,
        identity_subject: None,
    };
    store.users = vec![
        mk(1, "admin@example.com", "Administrator", "administrator"),
        mk(2, "alice@example.com", "Alice Example", "resident"),
        mk(3, "bob@example.com", "Bob Example", "resident"),
    ];
    store.friendships = vec![Friendship {
        id: 1,
        requester_id: 2,
        receiver_id: 3,
        requester_name: "Alice".into(),
        receiver_name: "Bob".into(),
        status: "accepted".into(),
        created_at: Some("2026-01-01T00:00:00Z".into()),
        responded_at: Some("2026-01-01T00:00:00Z".into()),
    }];
    store.next_user_id = 4;
    store.next_friendship_id = 2;
}

#[tokio::test]
async fn map_tile_route_matches() {
    let app = pinpoint_desktop::server::map_router()
        .with_state(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/map/tiles/10/1/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Tile fetch may fail offline (502) but must NOT be 404 (route must match).
    assert_ne!(resp.status(), StatusCode::NOT_FOUND, "tile route missing");
}

#[tokio::test]
async fn messages_service_mounts_its_routes() {
    use pinpoint_desktop::server::{messages_router, AppState, ServiceKind};
    assert_eq!(ServiceKind::Messages.label(), "messages");
    assert!(ServiceKind::Messages.owns_messages());
    // The split messages service must own envelopes and nothing else.
    assert!(!ServiceKind::Messages.owns_users());
    assert!(!ServiceKind::Messages.owns_friendships());
    let app = messages_router().with_state(AppState::for_owner(ServiceKind::Messages));
    for uri in [
        "/api/v1/messages/unread",
        "/api/v1/messages/conversations",
        "/api/v1/messages/key/1",
        "/api/v1/messages/1",
    ] {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "{uri} must be mounted and require a session"
        );
    }
}

#[tokio::test]
async fn minimal_param_route() {
    use axum::routing::get;
    let app = axum::Router::new().route("/:id", get(|| async { "hi" }));
    let resp = app
        .oneshot(Request::builder().uri("/42").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "minimal param route must match"
    );
}

#[tokio::test]
async fn users_by_id_route_matches() {
    let app = pinpoint_desktop::server::users_router()
        .with_state(pinpoint_desktop::server::AppState::new());
    // A non-numeric id is rejected by the Path<i64> extractor (400), which
    // still proves the route matched; the store no longer seeds demo users.
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/not-a-number")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "users by-id route missing"
    );
}

#[tokio::test]
async fn health_route_matches_all_services() {
    // Redundancy: every service replica must serve /health for Docker
    // healthchecks and gateway failover (docker-compose.yml). The routers
    // themselves no longer embed /health (the combined router would panic on
    // duplicate routes); each service binary adds it, as this test does.
    use pinpoint_desktop::server::health;
    for app in [
        pinpoint_desktop::server::auth_router()
            .route("/health", get(health))
            .with_state(pinpoint_desktop::server::AppState::new()),
        pinpoint_desktop::server::users_router()
            .route("/health", get(health))
            .with_state(pinpoint_desktop::server::AppState::new()),
        pinpoint_desktop::server::locations_router()
            .route("/health", get(health))
            .with_state(pinpoint_desktop::server::AppState::new()),
    ] {
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "/health missing");
    }
}

#[tokio::test]
async fn sync_status_route_matches() {
    // Redundancy: /api/v1/sync-status must be reachable for replica
    // freshness monitoring by clients and load balancers.
    let app = axum::Router::new()
        .route(
            "/api/v1/sync-status",
            get(pinpoint_desktop::server::sync_status),
        )
        .with_state(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/sync-status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "sync-status route missing"
    );
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["mode"], "in-memory");
    assert_eq!(json["database_connected"], false);
}

#[tokio::test]
async fn session_list_and_revoke_flow() {
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    // A session always belongs to a real, active account; the middleware
    // fails closed otherwise.
    {
        let mut store = state.store.lock().unwrap();
        store.users.push(User {
            id: 42,
            username: String::new(),
            email: "session-user@example.com".into(),
            full_name: "Session User".into(),
            role: "resident".into(),
            is_active: true,
            must_change_password: false,
            password_hash: None,
            totp_secret: None,
            mfa_enabled: false,
            mfa_last_step: 0,
            failed_login_attempts: 0,
            locked_until: None,
            auth_method: "email".into(),
            public_key: None,
            recovery_code_hash: None,
            identity_subject: None,
        });
    }
    let (sid, csrf) = state.create_session(42, "email", false).await;
    let app = router(state.clone());
    let cookie = format!("session_id={sid}");

    // Unauthenticated: fail closed.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Authenticated: one session, marked current.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/sessions")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["sessions"][0]["current"], true);
    let prefix = json["sessions"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(prefix.len(), 8, "only a short prefix may be exposed");
    assert!(!prefix.contains(&sid), "full session id must stay secret");

    // Revoking without the CSRF token is rejected (double-submit).
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/auth/sessions/{prefix}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // With the token the session is revoked and the cookie cleared.
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/auth/sessions/{prefix}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(state.get_session(&sid).await.is_none());
}

#[tokio::test]
async fn anonymous_account_creation_and_recovery() {
    use pinpoint_desktop::server::{router, AppState};

    let app = router(AppState::new());
    // The device name becomes the public display name: it follows the same
    // character and display-control rules as an email account's name.
    for invalid in [
        serde_json::json!({"device_name": "é".repeat(101)}).to_string(),
        serde_json::json!({"device_name": "Alice\u{202E}Smith"}).to_string(),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/anonymous")
                    .header("content-type", "application/json")
                    .body(Body::from(invalid))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    let body = r#"{"device_name":"Test device"}"#;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/anonymous")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let code = json["recovery_code"].as_str().unwrap().to_string();
    assert!(code.len() >= 32, "recovery code must be high-entropy");
    assert_eq!(json["user"]["auth_method"], "anonymous");
    let handle = json["user"]["username"].as_str().expect("public handle");
    let user_id = json["user"]["id"].as_i64().expect("id");
    assert!(
        handle.ends_with(&format!("-{user_id}")),
        "handle must carry the unique id suffix"
    );

    // Recover with the code: a fresh session + a rotated code.
    let body = serde_json::json!({ "recovery_code": code }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/anonymous/recover")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rotated = json["recovery_code"].as_str().unwrap().to_string();
    assert_ne!(rotated, code, "recovery code must rotate on use");

    // The old code is now dead.
    let body = serde_json::json!({ "recovery_code": code }).to_string();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/anonymous/recover")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_endpoints_require_admin_role() {
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    seed_test_users(&state);
    // Seed: 1 = administrator, 2 = alice (resident).
    let (alice_sid, _) = state.create_session(2, "email", false).await;
    let (admin_sid, _) = state.create_session(1, "email", false).await;
    let app = router(state);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/overview")
                .header("cookie", format!("session_id={alice_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/overview")
                .header("cookie", format!("session_id={admin_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_delete_user_revokes_sessions() {
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    seed_test_users(&state);
    let (admin_sid, admin_csrf) = state.create_session(1, "email", false).await;
    let (bob_sid, _) = state.create_session(3, "email", false).await;
    let app = router(state.clone());

    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/admin/users/3")
                .header("cookie", format!("session_id={admin_sid}"))
                .header("x-csrf-token", &admin_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(state.get_session(&bob_sid).await.is_none());
    assert!(state.store.lock().unwrap().find_user_by_id(3).is_none());
}

#[tokio::test]
async fn usernames_are_exact_lookup_and_profiles_are_private() {
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    seed_test_users(&state);
    // Seed: 1 = administrator ("brave-badger-1"), 2 = Alice
    // ("bright-badger-2"), 3 = Bob ("calm-badger-3"); Alice and Bob are
    // accepted friends.
    let (alice_sid, alice_csrf) = state.create_session(2, "email", false).await;
    let (bob_sid, _) = state.create_session(3, "email", false).await;
    let (admin_sid, _) = state.create_session(1, "email", false).await;
    let app = router(state);

    // Exact handle -> public card only, plus the viewer's relation.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/lookup?username=bright-badger-2")
                .header("cookie", format!("session_id={bob_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["username"], "bright-badger-2");
    assert_eq!(json["friendship_status"], "accepted");
    assert!(json.get("email").is_none(), "email must stay private");
    assert!(
        json.get("full_name").is_none(),
        "legal name must stay private"
    );

    // Prefixes do not resolve: discovery is exact-match only.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/lookup?username=bright")
                .header("cookie", format!("session_id={bob_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Accepted friends see the friend profile.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/2")
                .header("cookie", format!("session_id={bob_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["email"], "alice@example.com");
    assert_eq!(json["full_name"], "Alice Example");
    assert!(json.get("role").is_none(), "role is not friend-visible");

    // Walking numeric ids cannot reach an unrelated account.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/1")
                .header("cookie", format!("session_id={bob_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Signed-out lookup fails closed.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/lookup?username=bright-badger-2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Admins retain the private record.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/users/2")
                .header("cookie", format!("session_id={admin_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Friend requests resolve handles and store handles, not legal names.
    let body = serde_json::json!({ "receiver_username": "brave-badger-1" }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/friendships/")
                .header("cookie", format!("session_id={alice_sid}"))
                .header("x-csrf-token", &alice_csrf)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["requester_name"], "bright-badger-2");
    assert_eq!(json["receiver_name"], "brave-badger-1");

    // Duplicate requests are rejected instead of stacking rows.
    let body = serde_json::json!({ "receiver_username": "@BRAVE-BADGER-1" }).to_string();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/friendships/")
                .header("cookie", format!("session_id={alice_sid}"))
                .header("x-csrf-token", &alice_csrf)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn diagnostics_reports_and_audit_are_role_gated() {
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    seed_test_users(&state);
    let (alice_sid, alice_csrf) = state.create_session(2, "email", false).await;
    let (admin_sid, admin_csrf) = state.create_session(1, "email", false).await;
    let app = router(state);

    // Signed-in report, with CSRF.
    let body = serde_json::json!({
        "category": "map",
        "message": "Tiles failed near the station",
        "platform": "web"
    })
    .to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/diagnostics/report")
                .header("cookie", format!("session_id={alice_sid}"))
                .header("x-csrf-token", &alice_csrf)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Anonymous report: accepted, but stored without an account link.
    let body = serde_json::json!({
        "category": "other",
        "message": "Signed-out crash on the login screen",
        "platform": "web"
    })
    .to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/diagnostics/report")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Display controls are rejected: reports are rendered in the admin console.
    let body = serde_json::json!({
        "category": "other",
        "message": "line\u{202E}break",
        "platform": "web"
    })
    .to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/diagnostics/report")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Reports are not visible to non-admins.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/diagnostics")
                .header("cookie", format!("session_id={alice_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Admin listing exposes handles only.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/diagnostics")
                .header("cookie", format!("session_id={admin_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["total"], 2);
    assert_eq!(json["open"], 2);
    let reports = json["reports"].as_array().unwrap();
    assert_eq!(reports[0]["reporter"], "anonymous");
    assert_eq!(reports[1]["reporter"], "bright-badger-2");
    assert!(reports[1].get("email").is_none());
    assert!(reports[1].get("latitude").is_none());
    let report_id = reports[1]["id"].as_i64().unwrap();

    // Triage writes a status and an audit entry.
    let body = serde_json::json!({ "status": "triaged" }).to_string();
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/admin/diagnostics/{report_id}"))
                .header("cookie", format!("session_id={admin_sid}"))
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Deleting an account also lands in the append-only audit trail.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/admin/users/3")
                .header("cookie", format!("session_id={admin_sid}"))
                .header("x-csrf-token", &admin_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/audit")
                .header("cookie", format!("session_id={admin_sid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let actions: Vec<&str> = json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["action"].as_str())
        .collect();
    assert!(actions.contains(&"diagnostic.triage"));
    assert!(actions.contains(&"user.delete"));
    assert!(
        !json.to_string().contains("bob@example.com"),
        "audit must not store emails"
    );
}

#[tokio::test]
async fn map_capabilities_lists_driving() {
    let app = pinpoint_desktop::server::map_router()
        .with_state(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/map/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["modes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m == "driving"));
    assert!(json["autocomplete"].is_boolean());
    assert!(json["avoid"].is_boolean());
}

#[tokio::test]
async fn map_search_rejects_short_query() {
    let app = pinpoint_desktop::server::map_router()
        .with_state(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/map/search?q=ab")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn map_route_rejects_invalid_coordinates() {
    let app = pinpoint_desktop::server::map_router()
        .with_state(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/map/route?from_lat=999&from_lon=28.8&to_lat=47.0&to_lon=28.9&mode=driving")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn status_page_and_services_json_are_public_and_sanitized() {
    use pinpoint_desktop::server::{router, AppState, ServiceKind};

    let app = router(AppState::for_owner(ServiceKind::All));
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/services")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["service"], "all");
    assert_eq!(json["components"]["locations"], "/api/v1/locations");
    assert!(json["integrations"]["nominatim"].is_string());
    assert!(
        !json["integrations"].to_string().contains('@'),
        "integration endpoints must not expose userinfo"
    );

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Pinpoint services"));
    assert!(html.contains("OpenStreetMap"));
    assert!(html.contains("/api/v1/services"));
}

#[tokio::test]
async fn split_service_mounts_status_with_its_own_kind() {
    use pinpoint_desktop::server::{auth_router, status_routes, AppState, ServiceKind};

    let app = auth_router()
        .merge(status_routes())
        .with_state(AppState::for_owner(ServiceKind::Auth));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/services")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["service"], "auth");
}

#[tokio::test]
async fn api_json_carries_baseline_security_headers() {
    use pinpoint_desktop::server::{router, AppState};

    // An unauthenticated API call: the 401 JSON must still be hardened.
    let app = router(AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let headers = resp.headers();
    assert_eq!(
        headers["strict-transport-security"],
        "max-age=31536000; includeSubDomains"
    );
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert_eq!(
        headers["permissions-policy"],
        "geolocation=(self), camera=(), microphone=(), payment=(), usb=()"
    );
    assert_eq!(headers["cross-origin-opener-policy"], "same-origin");
    assert_eq!(headers["cross-origin-resource-policy"], "same-origin");
    assert_eq!(headers["x-permitted-cross-domain-policies"], "none");
    // Private API data must not be cached by a shared browser or proxy.
    assert_eq!(headers["cache-control"], "no-store");
}

#[tokio::test]
async fn health_carries_baseline_security_headers() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let headers = resp.headers();
    assert_eq!(
        headers["strict-transport-security"],
        "max-age=31536000; includeSubDomains"
    );
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert_eq!(
        headers["permissions-policy"],
        "geolocation=(self), camera=(), microphone=(), payment=(), usb=()"
    );
    assert_eq!(headers["cross-origin-opener-policy"], "same-origin");
    assert_eq!(headers["cross-origin-resource-policy"], "same-origin");
    assert_eq!(headers["x-permitted-cross-domain-policies"], "none");
}

#[tokio::test]
async fn privacy_route_matches() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/privacy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Privacy policy"));
    assert!(html.to_ascii_lowercase().contains("delete your account"));
}

#[tokio::test]
async fn manifest_is_same_origin_and_installable() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/manifest.webmanifest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "application/manifest+json");
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["start_url"], "/");
    assert_eq!(json["scope"], "/");
    assert_eq!(json["display"], "standalone");
    assert_eq!(json["icons"][0]["src"], "/favicon.svg");
}

#[tokio::test]
async fn admin_created_user_requires_a_password_and_forces_a_change() {
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    {
        let mut store = state.store.lock().unwrap();
        store.users.push(User {
            id: 1,
            username: String::new(),
            email: "admin-create@example.com".into(),
            full_name: "Administrator".into(),
            role: "administrator".into(),
            is_active: true,
            must_change_password: false,
            password_hash: None,
            totp_secret: None,
            mfa_enabled: false,
            mfa_last_step: 0,
            failed_login_attempts: 0,
            locked_until: None,
            auth_method: "email".into(),
            public_key: None,
            recovery_code_hash: None,
            identity_subject: None,
        });
        store.next_user_id = 2;
    }
    let (sid, csrf) = state.create_session(1, "email", false).await;
    let app = router(state.clone());

    let request = |body: &'static str| {
        Request::builder()
            .method("POST")
            .uri("/api/v1/users/")
            .header("cookie", format!("session_id={sid}"))
            .header("x-csrf-token", &csrf)
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap()
    };

    // No shared default password: the admin must supply one.
    let resp = app
        .clone()
        .oneshot(request(
            r#"{"email":"new-user@example.com","full_name":"New User"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Roles are a closed set; a typo must not create an account with an
    // unrecognised privilege string.
    let resp = app
        .clone()
        .oneshot(request(
            r#"{"email":"bad-role@example.com","full_name":"Bad Role","password":"TemporaryPass!2026","role":"superuser"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = app
        .clone()
        .oneshot(request(
            r#"{"email":"new-user@example.com","full_name":"New User","password":"TemporaryPass!2026"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["id"], 2);
    let must_change = {
        let store = state.store.lock().unwrap();
        store
            .find_user_by_id(2)
            .map(|u| u.must_change_password)
            .unwrap_or(false)
    };
    assert!(must_change, "temporary password must be rotated");
    let audited = {
        let store = state.store.lock().unwrap();
        store
            .admin_audit
            .iter()
            .any(|entry| entry.action == "user.create" && entry.target == "user:2")
    };
    assert!(audited, "admin user creation must be audited");

    // The temporary password only unlocks the password-change flow.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"new-user@example.com","password":"TemporaryPass!2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let csrf = json["csrf_token"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/friendships/")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "an unrotated temporary password must not reach the API"
    );

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/password/force-change")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"new_password":"RotatedPass!2026","confirm_password":"RotatedPass!2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // A fresh login with the rotated password can use the API.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"new-user@example.com","password":"RotatedPass!2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let cookie = resp
        .headers()
        .get("set-cookie")
        .expect("rotated session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/friendships/")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn deactivating_an_account_revokes_its_sessions() {
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    {
        let mut store = state.store.lock().unwrap();
        let mk = |id: i64, email: &str, role: &str| User {
            id,
            username: String::new(),
            email: email.to_string(),
            full_name: format!("User {id}"),
            role: role.to_string(),
            is_active: true,
            must_change_password: false,
            password_hash: None,
            totp_secret: None,
            mfa_enabled: false,
            mfa_last_step: 0,
            failed_login_attempts: 0,
            locked_until: None,
            auth_method: "email".to_string(),
            public_key: None,
            recovery_code_hash: None,
            identity_subject: None,
        };
        store.users = vec![
            mk(1, "deactivate-admin@example.com", "administrator"),
            mk(2, "deactivate-target@example.com", "resident"),
        ];
        store.next_user_id = 3;
    }
    let (admin_sid, admin_csrf) = state.create_session(1, "email", false).await;
    let (target_sid, _) = state.create_session(2, "email", false).await;
    let app = router(state.clone());

    let request =
        |sid: &str, method: &str, uri: &str, csrf: Option<&str>, body: Option<&'static str>| {
            let mut builder = Request::builder()
                .method(method)
                .uri(uri)
                .header("cookie", format!("session_id={sid}"));
            if let Some(csrf) = csrf {
                builder = builder.header("x-csrf-token", csrf);
            }
            if body.is_some() {
                builder = builder.header("content-type", "application/json");
            }
            builder.body(Body::from(body.unwrap_or_default())).unwrap()
        };

    let resp = app
        .clone()
        .oneshot(request(&target_sid, "GET", "/api/v1/auth/me", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "target starts signed in");

    let resp = app
        .clone()
        .oneshot(request(
            &admin_sid,
            "PATCH",
            "/api/v1/admin/users/2",
            Some(&admin_csrf),
            Some(r#"{"is_active":false}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    assert!(
        state.get_session(&target_sid).await.is_none(),
        "deactivation must revoke the account's sessions"
    );
    let resp = app
        .clone()
        .oneshot(request(&target_sid, "GET", "/api/v1/auth/me", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Defense in depth: even a session row recreated by a stale replica must
    // not authenticate an inactive account.
    let (zombie_sid, _) = state.create_session(2, "email", false).await;
    let resp = app
        .clone()
        .oneshot(request(&zombie_sid, "GET", "/api/v1/auth/me", None, None))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "the middleware must fail closed for an inactive account"
    );

    let resp = app
        .oneshot(request(&admin_sid, "GET", "/api/v1/auth/me", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "the admin session survives");
}

#[tokio::test]
async fn registration_rejects_an_overlong_name() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    let body = serde_json::json!({
        "email": "long-name@example.invalid",
        "password": "StrongPass!2026",
        "full_name": "x".repeat(101)
    })
    .to_string();
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn map_tiles_reject_out_of_range_coordinates() {
    let app = pinpoint_desktop::server::router(pinpoint_desktop::server::AppState::new());
    for uri in ["/map/tiles/2/4/0", "/map/tiles/2/0/4", "/map/tiles/0/1/0"] {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{uri}");
    }
}

#[tokio::test]
async fn admin_can_issue_a_temporary_password() {
    use pinpoint_desktop::security::password::{hash_password, verify_password, DEFAULT_ITERS};
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    let old_hash = hash_password("OldPass!2026", DEFAULT_ITERS);
    {
        let mut store = state.store.lock().unwrap();
        let mk = |id: i64, email: &str, role: &str, hash: Option<String>| User {
            id,
            username: String::new(),
            email: email.to_string(),
            full_name: format!("User {id}"),
            role: role.to_string(),
            is_active: true,
            must_change_password: false,
            password_hash: hash,
            totp_secret: None,
            mfa_enabled: false,
            mfa_last_step: 0,
            failed_login_attempts: 0,
            locked_until: None,
            auth_method: "email".to_string(),
            public_key: None,
            recovery_code_hash: None,
            identity_subject: None,
        };
        store.users = vec![
            mk(1, "reset-admin@example.com", "administrator", None),
            mk(2, "reset-target@example.com", "resident", Some(old_hash)),
        ];
        store.users[1].totp_secret = Some("JBSWY3DPEHPK3PXP".into());
        store.users[1].mfa_enabled = true;
        store.next_user_id = 3;
    }
    let (admin_sid, admin_csrf) = state.create_session(1, "email", false).await;
    let (target_sid, _) = state.create_session(2, "email", false).await;
    let app = router(state.clone());

    let request =
        |sid: &str, method: &str, uri: &str, csrf: Option<&str>, body: Option<&'static str>| {
            let mut builder = Request::builder()
                .method(method)
                .uri(uri)
                .header("cookie", format!("session_id={sid}"));
            if let Some(csrf) = csrf {
                builder = builder.header("x-csrf-token", csrf);
            }
            if body.is_some() {
                builder = builder.header("content-type", "application/json");
            }
            builder.body(Body::from(body.unwrap_or_default())).unwrap()
        };

    // Self-reset is refused; the Account panel is the self-service path.
    let resp = app
        .clone()
        .oneshot(request(
            &admin_sid,
            "POST",
            "/api/v1/admin/users/1/password",
            Some(&admin_csrf),
            Some(r#"{"password":"TempPass!2026"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = app
        .clone()
        .oneshot(request(
            &admin_sid,
            "POST",
            "/api/v1/admin/users/2/password",
            Some(&admin_csrf),
            Some(r#"{"password":"TempPass!2026","reset_mfa":true}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        state.get_session(&target_sid).await.is_none(),
        "reset must revoke sessions"
    );

    let (hash, must_change, mfa_enabled, totp_secret) = {
        let store = state.store.lock().unwrap();
        let user = store.find_user_by_id(2).unwrap();
        (
            user.password_hash.clone().unwrap(),
            user.must_change_password,
            user.mfa_enabled,
            user.totp_secret.clone(),
        )
    };
    assert!(verify_password("TempPass!2026", &hash));
    assert!(must_change);
    assert!(!mfa_enabled);
    assert!(totp_secret.is_none());

    // Old password fails; the temporary one works and is flagged for rotation.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"reset-target@example.com","password":"OldPass!2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"reset-target@example.com","password":"TempPass!2026"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["user"]["must_change_password"], true);

    let audited = {
        let store = state.store.lock().unwrap();
        store
            .admin_audit
            .iter()
            .any(|entry| entry.action == "user.password_reset" && entry.target == "user:2")
    };
    assert!(audited, "password reset must be audited");
}

#[tokio::test]
async fn email_registration_and_login_ignore_case_and_padding() {
    use pinpoint_desktop::server::{router, AppState};

    let app = router(AppState::new());
    let body = r#"{"email":"  CaseUser@Example.com  ","password":"StrongPass!2026","full_name":"Case User"}"#;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["user"]["email"], "caseuser@example.com");

    // A different case is the same account, not a duplicate.
    let duplicate =
        r#"{"email":"CASEUSER@EXAMPLE.COM","password":"StrongPass!2026","full_name":"Case User"}"#;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(duplicate))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Login accepts the same address in any case, with padding.
    let login = r#"{"email":"  CASEUSER@example.com  ","password":"StrongPass!2026"}"#;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(login))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_can_force_a_password_rotation_with_patch() {
    use pinpoint_desktop::security::password::{hash_password, DEFAULT_ITERS};
    use pinpoint_desktop::server::{router, AppState};

    let state = AppState::new();
    let hash = hash_password("OldPass!2026", DEFAULT_ITERS);
    {
        let mut store = state.store.lock().unwrap();
        let mk = |id: i64, email: &str, role: &str, hash: Option<String>| User {
            id,
            username: String::new(),
            email: email.to_string(),
            full_name: format!("User {id}"),
            role: role.to_string(),
            is_active: true,
            must_change_password: false,
            password_hash: hash,
            totp_secret: None,
            mfa_enabled: false,
            mfa_last_step: 0,
            failed_login_attempts: 0,
            locked_until: None,
            auth_method: "email".to_string(),
            public_key: None,
            recovery_code_hash: None,
            identity_subject: None,
        };
        store.users = vec![
            mk(1, "patch-admin@example.com", "administrator", None),
            mk(2, "patch-target@example.com", "resident", Some(hash)),
        ];
        store.next_user_id = 3;
    }
    let (admin_sid, admin_csrf) = state.create_session(1, "email", false).await;
    let (target_sid, target_csrf) = state.create_session(2, "email", false).await;
    let app = router(state.clone());

    let request = |sid: &str, method: &str, uri: &str, csrf: &str, body: Option<&'static str>| {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("cookie", format!("session_id={sid}"))
            .header("x-csrf-token", csrf);
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        builder.body(Body::from(body.unwrap_or_default())).unwrap()
    };

    let resp = app
        .clone()
        .oneshot(request(
            &admin_sid,
            "PATCH",
            "/api/v1/users/2",
            &admin_csrf,
            Some(r#"{"must_change_password":true}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    {
        let store = state.store.lock().unwrap();
        assert!(store.find_user_by_id(2).unwrap().must_change_password);
        assert!(
            store.admin_audit.iter().any(
                |entry| entry.action == "user.must_change_password" && entry.target == "user:2"
            ),
            "forcing a rotation must be audited"
        );
    }
    let resp = app
        .clone()
        .oneshot(request(
            &target_sid,
            "GET",
            "/api/v1/locations/shares",
            &target_csrf,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    // `/auth/me` stays readable so the client can render the rotation screen.
    let resp = app
        .clone()
        .oneshot(request(
            &target_sid,
            "GET",
            "/api/v1/auth/me",
            &target_csrf,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(request(
            &admin_sid,
            "PATCH",
            "/api/v1/users/2",
            &admin_csrf,
            Some(r#"{"must_change_password":false}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .clone()
        .oneshot(request(
            &target_sid,
            "GET",
            "/api/v1/locations/shares",
            &target_csrf,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // The admin console route accepts the same flag and audits the change.
    let resp = app
        .clone()
        .oneshot(request(
            &admin_sid,
            "PATCH",
            "/api/v1/admin/users/2",
            &admin_csrf,
            Some(r#"{"must_change_password":true}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    {
        let store = state.store.lock().unwrap();
        assert!(store.find_user_by_id(2).unwrap().must_change_password);
        assert!(
            store.admin_audit.iter().any(
                |entry| entry.action == "user.must_change_password" && entry.target == "user:2"
            ),
            "the admin route must audit the rotation flag"
        );
    }
    let resp = app
        .clone()
        .oneshot(request(
            &target_sid,
            "GET",
            "/api/v1/locations/shares",
            &target_csrf,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let resp = app
        .clone()
        .oneshot(request(
            &admin_sid,
            "PATCH",
            "/api/v1/admin/users/2",
            &admin_csrf,
            Some(r#"{"must_change_password":false}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .oneshot(request(
            &target_sid,
            "GET",
            "/api/v1/locations/shares",
            &target_csrf,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn registration_validates_the_email_address() {
    use pinpoint_desktop::server::{router, AppState};

    let app = router(AppState::new());
    for invalid in [
        r#"{"email":"not-an-email","password":"StrongPass!2026","full_name":"No Email"}"#,
        r#"{"email":"bidi\u202euser@example.com","password":"StrongPass!2026","full_name":"Bidi"}"#,
        r#"{"email":"user@domain","password":"StrongPass!2026","full_name":"No Dot"}"#,
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(invalid))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "body: {invalid}");
    }

    let valid =
        r#"{"email":"caf\u00e9@example.com","password":"StrongPass!2026","full_name":"Cafe"}"#;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(valid))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn throttled_login_reports_retry_after() {
    use pinpoint_desktop::server::{router, AppState};

    // The login throttle is per client address; without the header the
    // router sees no client and never limits.
    let app = router(AppState::new());
    let mut last = None;
    for _ in 0..31 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header("content-type", "application/json")
                    .header("x-forwarded-for", "203.0.113.7")
                    .body(Body::from(
                        r#"{"email":"nobody@example.com","password":"WrongPass!2026"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        last = Some(resp);
    }
    let resp = last.expect("the loop made requests");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let secs: u64 = resp.headers()["retry-after"]
        .to_str()
        .expect("Retry-After is a header")
        .parse()
        .expect("Retry-After is an integer");
    assert!((1..=300).contains(&secs), "retry hint was {secs}");
}

#[tokio::test]
async fn private_map_queries_are_not_cacheable() {
    use pinpoint_desktop::server::{router, AppState};

    // These answers carry the user's query or coordinates. A shared cache
    // must not replay them; tiles stay cacheable.
    let app = router(AppState::new());
    for uri in [
        "/map/search?q=ab",
        "/map/reverse?lat=200&lon=0",
        "/map/route?from_lat=0&from_lon=0&to_lat=0&to_lon=0&mode=swim",
    ] {
        let resp = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(resp.headers()["cache-control"], "no-store", "{uri}");
    }
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/map/tiles/99/0/0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(resp.headers().get("cache-control").is_none());
}

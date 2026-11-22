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

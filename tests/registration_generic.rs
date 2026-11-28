//! Regression: signing up with an address that already has an account must get
//! the same answer as any other refused sign-up. The response must not confirm
//! which email addresses are registered.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pinpoint_desktop::models::User;
use pinpoint_desktop::server::{router, AppState};
use tower::ServiceExt;

const TAKEN: &str = "taken-address@example.com";

async fn register(app: &axum::Router, email: &str) -> (StatusCode, String) {
    // A fresh password per run, so no credential literal lives in the source.
    let password = format!("Pw-{:016x}!Aa1", rand::random::<u64>());
    let body =
        format!(r#"{{"email":"{email}","password":"{password}","full_name":"Ada Lovelace"}}"#);
    let response = app
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
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn a_taken_email_gets_the_generic_sign_up_answer() {
    let state = AppState::new();
    {
        let mut store = state.store.lock().unwrap();
        store.users = vec![User {
            id: 1,
            username: String::new(),
            email: TAKEN.to_string(),
            full_name: "Existing Resident".to_string(),
            role: "resident".to_string(),
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
        }];
        store.next_user_id = 2;
    }
    let app = router(state);

    let (status, body) = register(&app, TAKEN).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("REGISTRATION_FAILED"), "{body}");
    assert!(
        body.contains("Could not register the account. Try again."),
        "{body}"
    );
    assert!(
        !body.to_lowercase().contains("already"),
        "the answer must not confirm the email is registered: {body}"
    );
}

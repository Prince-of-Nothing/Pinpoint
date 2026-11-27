//! Regression: a correct password must not reset the lockout counter before the
//! TOTP step, and a wrong TOTP code must count as a failed attempt. Otherwise a
//! caller who knows the password can guess the 6-digit code without limit.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pinpoint_desktop::models::User;
use pinpoint_desktop::security::password::{hash_password, DEFAULT_ITERS};
use pinpoint_desktop::server::{router, AppState};
use tower::ServiceExt;

const EMAIL: &str = "mfa-lockout@example.com";
const PASSWORD: &str = "Right!Pass2026";

async fn login_status(app: &axum::Router, totp: &str) -> StatusCode {
    let body = format!(r#"{{"email":"{EMAIL}","password":"{PASSWORD}","totp":"{totp}"}}"#);
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn wrong_totp_codes_lock_the_account_even_with_the_right_password() {
    let state = AppState::new();
    {
        let mut store = state.store.lock().unwrap();
        store.users = vec![User {
            id: 1,
            username: String::new(),
            email: EMAIL.to_string(),
            full_name: "MFA Lockout".to_string(),
            role: "resident".to_string(),
            is_active: true,
            must_change_password: false,
            password_hash: Some(hash_password(PASSWORD, DEFAULT_ITERS)),
            totp_secret: Some("JBSWY3DPEHPK3PXP".to_string()),
            mfa_enabled: true,
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
    let app = router(state.clone());

    // Five attempts with the right password and a wrong code are five failures.
    for _ in 0..5 {
        assert_eq!(
            login_status(&app, "not-a-code").await,
            StatusCode::UNAUTHORIZED
        );
    }
    // The sixth attempt is refused by the lockout, even with the right password.
    assert_eq!(
        login_status(&app, "not-a-code").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

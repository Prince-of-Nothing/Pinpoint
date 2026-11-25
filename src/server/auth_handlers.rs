//! Auth service handlers (/api/v1/auth/*) with real server-side sessions.
use axum::{
    extract::{Extension, OriginalUri, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::state::AppState;
use super::{error_envelope, CurrentUser, IntoResponse};

/// One valid Argon2id hash used only to equalize login timing for unknown or
/// inactive accounts. It is never compared for access, so it hashes a random
/// throwaway secret: only its cost has to match a real hash.
fn dummy_password_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        let throwaway = format!("{:032x}", rand::random::<u128>());
        crate::security::password::hash_password(&throwaway, 0)
    })
}

fn user_json(u: &crate::models::User) -> Value {
    json!({
        "id": u.id,
        "username": super::usernames::username_for(u.id),
        "email": u.email,
        "full_name": u.full_name,
        "role": u.role,
        "is_active": u.is_active,
        "must_change_password": u.must_change_password,
        "mfa_enabled": u.mfa_enabled,
        "auth_method": u.auth_method,
    })
}

/// Session cookie on HTTPS. The `__Host-` prefix makes the browser reject a
/// copy scoped to a parent domain or a subdomain path, so a sibling host
/// cannot overwrite the session.
const HOST_SESSION_COOKIE: &str = "__Host-pinpoint_session";
/// Loopback HTTP dev keeps the plain name: `__Host-` requires `Secure`.
const SESSION_COOKIE: &str = "session_id";

pub(crate) fn session_cookie(session_id: &str, secure: bool) -> String {
    if secure {
        format!("{HOST_SESSION_COOKIE}={session_id}; Path=/; HttpOnly; SameSite=Lax; Secure")
    } else {
        format!("{SESSION_COOKIE}={session_id}; Path=/; HttpOnly; SameSite=Lax")
    }
}

/// HTTPS detection: TLS proxies (Render) forward the original scheme in
/// `X-Forwarded-Proto`; an absolute-form `https` request URI also counts.
pub(crate) fn request_is_secure(headers: &HeaderMap, uri: &axum::http::Uri) -> bool {
    uri.scheme_str() == Some("https")
        || headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .map(|proto| proto.eq_ignore_ascii_case("https"))
            .unwrap_or(false)
}

/// Both names are cleared: a client may still hold the legacy HTTP cookie
/// after an HTTPS login, and the `__Host-` variant needs `Secure` even to be
/// expired.
pub(crate) fn clear_cookies() -> [&'static str; 2] {
    [
        "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        "__Host-pinpoint_session=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0",
    ]
}

pub(crate) fn append_clear_cookies(resp: &mut axum::response::Response) {
    for cookie in clear_cookies() {
        resp.headers_mut()
            .append("Set-Cookie", cookie.parse().unwrap());
    }
}

fn session_response(
    user: &crate::models::User,
    csrf: &str,
    session_id: &str,
    secure: bool,
) -> axum::response::Response {
    let mut resp = (
        StatusCode::OK,
        Json(json!({
            // Legacy field kept empty for older clients; sessions are opaque
            // cookies now (ADR-0012). `expires_in` is the sliding idle window.
            "access_token": "",
            "token_type": "session",
            "expires_in": super::state::session_idle_seconds(),
            "csrf_token": csrf,
            "user": user_json(user),
        })),
    )
        .into_response();
    resp.headers_mut()
        .insert("X-CSRF-Token", csrf.parse().unwrap());
    resp.headers_mut().insert(
        "Set-Cookie",
        session_cookie(session_id, secure).parse().unwrap(),
    );
    resp
}

fn unauthenticated() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": {"code": "UNAUTHENTICATED", "message": "Sign in required."}})),
    )
        .into_response()
}

/// `POST /auth/login` — password verification + real session cookie.
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let password = body.get("password").and_then(Value::as_str).unwrap_or("");

    let invalid = || {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": {"code": "INVALID_CREDENTIALS", "message": "Invalid email or password."}})),
        )
            .into_response()
    };
    // Scope the store lock so it is never held across the session await.
    let user = {
        let store = state.store.lock().unwrap();
        store.find_user_by_email(email).cloned()
    };
    let Some(user) = user else {
        // Do the same Argon2 work as a real user so response timing does not
        // reveal whether an email exists.
        let _ = crate::security::password::verify_password(password, dummy_password_hash());
        return invalid();
    };
    if !user.is_active {
        let _ = crate::security::password::verify_password(password, dummy_password_hash());
        return invalid();
    }
    // Lockout check.
    if let Some(until) = &user.locked_until {
        if until.as_str() > "2026-01-01T00:00:00Z" {
            let now = chrono::Utc::now().to_rfc3339();
            if until.as_str() > now.as_str() {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error": {"code": "ACCOUNT_LOCKED", "message": "Account temporarily locked. Try again later."}})),
                )
                    .into_response();
            }
        }
    }
    let ok = match &user.password_hash {
        Some(h) => crate::security::password::verify_password(password, h),
        None => false,
    };
    let user_id = user.id;
    if !ok {
        // Increment failed attempts; lock after 5.
        {
            let mut store = state.store.lock().unwrap();
            if let Some(u) = store.users.iter_mut().find(|u| u.id == user_id) {
                u.failed_login_attempts = u.failed_login_attempts.saturating_add(1);
                if u.failed_login_attempts >= 5 {
                    u.locked_until =
                        Some((chrono::Utc::now() + chrono::Duration::minutes(15)).to_rfc3339());
                }
            }
        }
        return invalid();
    }
    // Reset on success.
    {
        let mut store = state.store.lock().unwrap();
        if let Some(u) = store.users.iter_mut().find(|u| u.id == user_id) {
            u.failed_login_attempts = 0;
            u.locked_until = None;
        }
    }
    // Transparent upgrade: legacy PBKDF2 hashes are re-hashed to Argon2id.
    if crate::security::password::needs_rehash(user.password_hash.as_deref().unwrap_or_default()) {
        let new_hash = crate::security::password::hash_password(password, 0);
        let mut store = state.store.lock().unwrap();
        if let Some(u) = store.users.iter_mut().find(|u| u.id == user_id) {
            u.password_hash = Some(new_hash);
        }
    }
    // MFA gate: enrolled users must present a valid TOTP in the login body.
    if user.mfa_enabled {
        let now = chrono::Utc::now().timestamp() as u64;
        let code = body.get("totp").and_then(Value::as_str).unwrap_or("");
        let step = user
            .totp_secret
            .as_deref()
            .and_then(|s| crate::security::mfa::verify_totp(s, code, now));
        match step {
            Some(st) if st > user.mfa_last_step => {
                let mut store = state.store.lock().unwrap();
                if let Some(u) = store.users.iter_mut().find(|u| u.id == user.id) {
                    u.mfa_last_step = st;
                }
            }
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": {"code": "MFA_REQUIRED", "message": "A valid 6-digit authenticator code is required."}})),
                )
                    .into_response()
            }
        }
    }

    let (session_id, csrf) = state.create_session(user.id, "email", false).await;
    session_response(&user, &csrf, &session_id, request_is_secure(&headers, &uri))
}

/// `POST /auth/register`.
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let password = body.get("password").and_then(Value::as_str).unwrap_or("");
    let full_name = body
        .get("full_name")
        .and_then(Value::as_str)
        .unwrap_or("New User")
        .to_string();
    let name_check = crate::security::validation::validate_name(&full_name);
    if !name_check.is_valid() {
        return error_envelope("VALIDATION_FAILED", &name_check.errors.join("; "));
    }
    if email.is_empty() {
        return error_envelope("VALIDATION_FAILED", "email is required");
    }
    let email_check = crate::security::validation::validate_email(&email);
    if !email_check.is_valid() {
        return error_envelope("VALIDATION_FAILED", &email_check.errors.join("; "));
    }
    if !crate::security::validation::validate_password(password).is_valid() {
        return error_envelope(
            "VALIDATION_FAILED",
            "password must be at least 8 characters",
        );
    }
    // The email pre-check avoids a duplicate insert and a confusing error.
    {
        let store = state.store.lock().unwrap();
        if store.find_user_by_email(&email).is_some() {
            return error_envelope("VALIDATION_FAILED", "email already registered");
        }
    }
    let password_hash = crate::security::password::hash_password(
        password,
        crate::security::password::DEFAULT_ITERS,
    );
    // Persist the row before creating the session. A session or the next
    // request can land on another replica, so a memory-only account must never
    // be acknowledged with 201.
    let id = match state
        .sql_insert_user_strict(&email, &full_name, "resident", Some(&password_hash))
        .await
    {
        Ok(id) => id,
        Err(error) => {
            tracing::error!(%error, "registration insert failed");
            return error_envelope(
                "REGISTRATION_FAILED",
                "Could not register the account. Try again.",
            );
        }
    };
    let user = crate::models::User {
        id,
        username: String::new(),
        email: email.clone(),
        full_name: full_name.clone(),
        role: "resident".into(),
        is_active: true,
        must_change_password: false,
        password_hash: Some(password_hash),
        totp_secret: None,
        mfa_enabled: false,
        mfa_last_step: 0,
        failed_login_attempts: 0,
        locked_until: None,
        auth_method: "email".into(),
        public_key: None,
        recovery_code_hash: None,
        identity_subject: None,
    };
    {
        let mut store = state.store.lock().unwrap();
        store.next_user_id = store.next_user_id.max(id + 1);
        store.users.push(user.clone());
    }

    let (session_id, csrf) = state.create_session(id, "email", false).await;
    let mut resp = (
        StatusCode::CREATED,
        Json(json!({
            "access_token": "",
            "token_type": "session",
            "expires_in": super::state::session_idle_seconds(),
            "csrf_token": csrf,
            "user": user_json(&user),
        })),
    )
        .into_response();
    resp.headers_mut()
        .insert("X-CSRF-Token", csrf.parse().unwrap());
    resp.headers_mut().insert(
        "Set-Cookie",
        session_cookie(&session_id, request_is_secure(&headers, &uri))
            .parse()
            .unwrap(),
    );
    resp
}

/// `POST /auth/logout` — revoke the current session and clear the cookie.
pub async fn logout(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let mut resp = (
        StatusCode::OK,
        Json(json!({"message": "Successfully logged out"})),
    )
        .into_response();
    append_clear_cookies(&mut resp);
    if let Some(Extension(cu)) = cu {
        state.delete_session(&cu.session_id).await;
    }
    resp
}

/// `POST /auth/session` — validate the session cookie and return the user.
pub async fn session(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let store = state.store.lock().unwrap();
    let Some(user) = store.find_user_by_id(cu.user_id).cloned() else {
        return unauthenticated();
    };
    drop(store);
    let mut resp = (StatusCode::OK, Json(json!({ "user": user_json(&user) }))).into_response();
    resp.headers_mut()
        .insert("X-CSRF-Token", cu.csrf_token.parse().unwrap());
    resp
}

/// `GET /auth/me`.
pub async fn me(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let store = state.store.lock().unwrap();
    match store.find_user_by_id(cu.user_id) {
        Some(user) => {
            let mut resp = (StatusCode::OK, Json(user_json(user))).into_response();
            // Lets a reloaded page re-arm the CSRF double-submit without a
            // second credential (the cookie is HttpOnly and unreadable to JS).
            resp.headers_mut()
                .insert("X-CSRF-Token", cu.csrf_token.parse().unwrap());
            resp
        }
        None => unauthenticated(),
    }
}

/// `POST /auth/mfa/enroll` — generate a TOTP secret for the session user.
pub async fn mfa_enroll(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let mut store = state.store.lock().unwrap();
    let Some(me) = store.find_user_by_id(cu.user_id).cloned() else {
        return unauthenticated();
    };
    let secret = crate::security::mfa::generate_secret();
    let uri = crate::security::mfa::otpauth_uri(&secret, &me.email, "Pinpoint");
    if let Some(user) = store.users.iter_mut().find(|u| u.id == me.id) {
        user.totp_secret = Some(secret.clone());
        user.mfa_enabled = false;
        user.mfa_last_step = 0;
    }
    (
        StatusCode::OK,
        Json(json!({ "secret": secret, "otpauth_uri": uri })),
    )
        .into_response()
}

/// `POST /auth/mfa/verify` {code} — confirm enrolment / consume the step.
pub async fn mfa_verify(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let code = body.get("code").and_then(Value::as_str).unwrap_or("");
    let now = chrono::Utc::now().timestamp() as u64;
    let mut store = state.store.lock().unwrap();
    let Some(user) = store.users.iter_mut().find(|u| u.id == cu.user_id) else {
        return unauthenticated();
    };
    let Some(secret) = user.totp_secret.clone() else {
        return error_envelope("MFA_NOT_ENROLLED", "No MFA secret to verify.");
    };
    match crate::security::mfa::verify_totp(&secret, code, now) {
        Some(step) if step > user.mfa_last_step => {
            user.mfa_enabled = true;
            user.mfa_last_step = step;
            (StatusCode::OK, Json(json!({ "mfa_enabled": true }))).into_response()
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": {"code": "INVALID_MFA_CODE", "message": "Invalid or already-used authenticator code."}})),
        )
            .into_response(),
    }
}

/// `GET /auth/mfa/status`.
pub async fn mfa_status(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let store = state.store.lock().unwrap();
    let enabled = store
        .find_user_by_id(cu.user_id)
        .map(|u| u.mfa_enabled)
        .unwrap_or(false);
    (StatusCode::OK, Json(json!({ "mfa_enabled": enabled }))).into_response()
}

/// `POST /auth/mfa/disable` - disable MFA after proving a current TOTP code.
///
/// A live session alone is not enough: a stolen cookie must not remove the
/// second factor. The code uses the same replay guard as `/auth/mfa/verify`.
pub async fn mfa_disable(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let code = body.get("code").and_then(Value::as_str).unwrap_or("");
    let now = chrono::Utc::now().timestamp() as u64;
    let mut store = state.store.lock().unwrap();
    let Some(user) = store.users.iter_mut().find(|u| u.id == cu.user_id) else {
        return unauthenticated();
    };
    let Some(secret) = user.totp_secret.clone() else {
        return error_envelope("MFA_NOT_ENROLLED", "No MFA secret to disable.");
    };
    match crate::security::mfa::verify_totp(&secret, code, now) {
        Some(step) if step > user.mfa_last_step => {
            user.totp_secret = None;
            user.mfa_enabled = false;
            user.mfa_last_step = 0;
            (StatusCode::OK, Json(json!({ "mfa_enabled": false }))).into_response()
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": {"code": "INVALID_MFA_CODE", "message": "Invalid or already-used authenticator code."}})),
        )
            .into_response(),
    }
}

/// `POST /auth/password/change` and `/auth/password/force-change`.
pub async fn change_password(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let new_password = body
        .get("new_password")
        .and_then(Value::as_str)
        .unwrap_or("");
    let current_password = body
        .get("current_password")
        .and_then(Value::as_str)
        .unwrap_or("");
    let confirm = body
        .get("confirm_password")
        .and_then(Value::as_str)
        .unwrap_or("");
    if new_password != confirm {
        return error_envelope("VALIDATION_FAILED", "new passwords do not match");
    }
    if !crate::security::validation::validate_password(new_password).is_valid() {
        return error_envelope(
            "VALIDATION_FAILED",
            "password must be at least 8 characters",
        );
    }
    {
        let mut store = state.store.lock().unwrap();
        let Some(user) = store.users.iter_mut().find(|u| u.id == cu.user_id) else {
            return unauthenticated();
        };
        // A normal change must prove the current password; the force-change
        // path is for accounts the server already flagged as must-change and
        // has no current password to check.
        if !user.must_change_password {
            let Some(hash) = user.password_hash.as_deref() else {
                return error_envelope("VALIDATION_FAILED", "This account has no password.");
            };
            if current_password.is_empty()
                || !crate::security::password::verify_password(current_password, hash)
            {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": {"code": "INVALID_CREDENTIALS", "message": "Current password is incorrect."}})),
                )
                    .into_response();
            }
        }
        user.password_hash = Some(crate::security::password::hash_password(
            new_password,
            crate::security::password::DEFAULT_ITERS,
        ));
        user.must_change_password = false;
    }
    // Revoke every session for the user (regular + forced change).
    state.delete_user_sessions(cu.user_id).await;
    let mut resp = (
        StatusCode::OK,
        Json(json!({"message": "Password changed."})),
    )
        .into_response();
    append_clear_cookies(&mut resp);
    resp
}

pub async fn force_change_password(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    change_password(State(state), cu, Json(body)).await
}

/// Public session descriptor. The full session id is a bearer secret, so only
/// an 8-hex-character prefix is exposed; it is enough to revoke one session.
fn session_json(s: &super::state::StoredSession, current_session: &str) -> Value {
    let prefix: String = s.session_id.chars().take(8).collect();
    json!({
        "id": prefix,
        "current": s.session_id == current_session,
        "auth_method": s.auth_method,
        "mfa_pending": s.mfa_pending,
        "created_at": s.created_at.to_rfc3339(),
        "last_activity": s.last_activity.to_rfc3339(),
    })
}

/// `GET /auth/sessions` — list the caller's active sessions (device management).
pub async fn list_sessions(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let sessions = state.list_sessions(cu.user_id).await;
    let items: Vec<Value> = sessions
        .iter()
        .map(|s| session_json(s, &cu.session_id))
        .collect();
    (StatusCode::OK, Json(json!({ "sessions": items }))).into_response()
}

/// `DELETE /auth/sessions/:id` — revoke one of the caller's sessions by prefix.
pub async fn revoke_session(
    State(state): State<AppState>,
    axum::extract::Path(prefix): axum::extract::Path<String>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    let prefix = prefix.trim().to_ascii_lowercase();
    if prefix.len() < 8 || !prefix.chars().all(|c| c.is_ascii_hexdigit()) {
        return error_envelope("VALIDATION_FAILED", "invalid session id");
    }
    let Some(target) = state
        .list_sessions(cu.user_id)
        .await
        .into_iter()
        .find(|s| s.session_id.starts_with(&prefix))
    else {
        return error_envelope("NOT_FOUND", "session not found");
    };
    let revoking_current = target.session_id == cu.session_id;
    state.delete_session(&target.session_id).await;
    let mut resp = (
        StatusCode::OK,
        Json(json!({"message": "Session revoked.", "current": revoking_current})),
    )
        .into_response();
    if revoking_current {
        append_clear_cookies(&mut resp);
    }
    resp
}

/// `POST /auth/logout-all` — revoke every session for the caller.
pub async fn logout_all(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let Some(Extension(cu)) = cu else {
        return unauthenticated();
    };
    state.delete_user_sessions(cu.user_id).await;
    let mut resp = (
        StatusCode::OK,
        Json(json!({"message": "All sessions revoked."})),
    )
        .into_response();
    append_clear_cookies(&mut resp);
    resp
}

// ---------------------------------------------------------------------------
// Anonymous (device-bound) accounts — ADR-0009 phase 2
// ---------------------------------------------------------------------------

/// Strip formatting so a user can type the code with or without dashes/spaces.
fn normalize_recovery_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// 160-bit one-time recovery code, shown once, stored only as a hash.
/// A plain SHA-256 is enough here (unlike passwords): the code is uniformly
/// random, so there is no dictionary to brute-force.
fn generate_recovery_code() -> String {
    let mut bytes = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut bytes);
    let hex: String = bytes.iter().map(|b| format!("{:02X}", b)).collect();
    hex.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("-")
}

fn recovery_code_hash(code: &str) -> String {
    let normalized = normalize_recovery_code(code);
    let digest = Sha256::digest(normalized.as_bytes());
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn anonymous_response(
    status: StatusCode,
    user: &crate::models::User,
    csrf: &str,
    session_id: &str,
    recovery_code: &str,
    secure: bool,
) -> axum::response::Response {
    let mut resp = (
        status,
        Json(json!({
            "token_type": "session",
            "expires_in": super::state::session_idle_seconds(),
            "csrf_token": csrf,
            "user": user_json(user),
            "recovery_code": recovery_code,
            "recovery_notice": "Store this code now. It is shown once and is the only way to recover this anonymous account.",
        })),
    )
        .into_response();
    resp.headers_mut()
        .insert("X-CSRF-Token", csrf.parse().unwrap());
    resp.headers_mut().insert(
        "Set-Cookie",
        session_cookie(session_id, secure).parse().unwrap(),
    );
    resp
}

/// `POST /auth/anonymous` — create a device-bound anonymous account.
pub async fn anonymous(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<Value>,
) -> axum::response::Response {
    // One active session per install: a repeated tap must not create a second
    // anonymous account or a second session.
    if cu.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": {"code": "ALREADY_SIGNED_IN", "message": "A session is already active. Sign out before creating another anonymous account."}})),
        )
            .into_response();
    }
    let device_name = body
        .get("device_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Anonymous device")
        .to_string();
    let name_check = crate::security::validation::validate_name(&device_name);
    if !name_check.is_valid() {
        return error_envelope("VALIDATION_FAILED", &name_check.errors.join("; "));
    }
    let public_key = body
        .get("public_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if public_key.as_deref().is_some_and(|key| {
        key.chars().count() > 512 || crate::security::validation::contains_display_control(key)
    }) {
        return error_envelope("VALIDATION_FAILED", "public_key is invalid");
    }
    let recovery_code = generate_recovery_code();
    let recovery_hash = recovery_code_hash(&recovery_code);

    let (id, user) = {
        let mut store = state.store.lock().unwrap();
        let id = store.next_user_id;
        store.next_user_id += 1;
        let user = crate::models::User {
            id,
            username: String::new(),
            // Unroutable by design: an anonymous account has no email inbox.
            email: format!("anon-{id}@device.pinpoint.invalid"),
            full_name: device_name,
            role: "resident".into(),
            is_active: true,
            must_change_password: false,
            password_hash: None,
            totp_secret: None,
            mfa_enabled: false,
            mfa_last_step: 0,
            failed_login_attempts: 0,
            locked_until: None,
            auth_method: "anonymous".into(),
            public_key,
            recovery_code_hash: Some(recovery_hash),
            identity_subject: None,
        };
        store.users.push(user.clone());
        (id, user)
    };

    let (session_id, csrf) = state.create_session(id, "anonymous", false).await;
    anonymous_response(
        StatusCode::CREATED,
        &user,
        &csrf,
        &session_id,
        &recovery_code,
        request_is_secure(&headers, &uri),
    )
}

/// `POST /auth/anonymous/recover` — restore an anonymous account on a new
/// device with a recovery code. The code rotates on every use (single-use).
pub async fn anonymous_recover(
    State(state): State<AppState>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let code = body
        .get("recovery_code")
        .and_then(Value::as_str)
        .unwrap_or("");
    if normalize_recovery_code(code).len() < 32 {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": {"code": "INVALID_RECOVERY_CODE", "message": "Invalid recovery code."}})),
        )
            .into_response();
    }
    let hash = recovery_code_hash(code);
    let new_code = generate_recovery_code();
    let new_hash = recovery_code_hash(&new_code);

    let user = {
        let mut store = state.store.lock().unwrap();
        let Some(index) = store.users.iter().position(|u| {
            u.auth_method == "anonymous" && u.recovery_code_hash.as_deref() == Some(hash.as_str())
        }) else {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": {"code": "INVALID_RECOVERY_CODE", "message": "Invalid recovery code."}})),
            )
                .into_response();
        };
        store.users[index].recovery_code_hash = Some(new_hash);
        store.users[index].clone()
    };

    let (session_id, csrf) = state.create_session(user.id, "anonymous", false).await;
    anonymous_response(
        StatusCode::OK,
        &user,
        &csrf,
        &session_id,
        &new_code,
        request_is_secure(&headers, &uri),
    )
}

#[cfg(test)]
mod registration_tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[test]
    fn session_cookie_uses_the_host_prefix_only_on_https() {
        let secure = session_cookie("abc", true);
        assert!(secure.starts_with("__Host-pinpoint_session=abc;"));
        assert!(secure.contains("; Secure"));
        let plain = session_cookie("abc", false);
        assert!(plain.starts_with("session_id=abc;"));
        assert!(!plain.to_ascii_lowercase().contains("secure"));
    }

    #[test]
    fn clearing_removes_both_cookie_names() {
        let cookies = clear_cookies();
        assert!(cookies.iter().any(|c| c.starts_with("session_id=;")));
        assert!(cookies
            .iter()
            .any(|c| c.starts_with("__Host-pinpoint_session=;") && c.contains("Secure")));
    }

    #[test]
    fn request_is_secure_reads_forwarded_proto() {
        let uri = axum::http::Uri::from_static("/api/v1/auth/login");
        let mut headers = HeaderMap::new();
        assert!(!request_is_secure(&headers, &uri));
        headers.insert("x-forwarded-proto", "HTTPS".parse().unwrap());
        assert!(request_is_secure(&headers, &uri));
        assert!(request_is_secure(
            &HeaderMap::new(),
            &axum::http::Uri::from_static("https://pinpoint.example/api/v1/auth/login")
        ));
    }

    #[tokio::test]
    async fn secure_registration_sets_a_host_prefixed_cookie() {
        let app = crate::server::router(AppState::new());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("x-forwarded-proto", "https")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"email":"host-cookie@example.com","password":"StrongPass!2026","full_name":"Host Cookie"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let cookie = response
            .headers()
            .get("set-cookie")
            .expect("registration must set the session cookie")
            .to_str()
            .unwrap();
        assert!(cookie.starts_with("__Host-pinpoint_session="));
        assert!(cookie.contains("; Secure"));
    }

    #[tokio::test]
    async fn registration_returns_a_usable_session() {
        let app = crate::server::router(AppState::new());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"email":"new-user@example.com","password":"StrongPass!2026","full_name":"New User"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let cookie = response
            .headers()
            .get("set-cookie")
            .expect("registration must set the session cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        let me = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            me.status(),
            StatusCode::OK,
            "the new session must authenticate"
        );
    }

    async fn json_body(response: axum::response::Response) -> Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn response_cookie(response: &axum::response::Response) -> String {
        response
            .headers()
            .get("set-cookie")
            .expect("response must set the session cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    async fn post_json(
        app: &axum::Router,
        uri: &str,
        cookie: &str,
        csrf: &str,
        body: Value,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .header("x-csrf-token", csrf)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn mfa_status(app: &axum::Router, cookie: &str) -> bool {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/mfa/status")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        json_body(response).await["mfa_enabled"].as_bool().unwrap()
    }

    #[tokio::test]
    async fn password_change_requires_the_current_password_and_revokes_sessions() {
        let app = crate::server::router(AppState::new());
        let response = post_json(
            &app,
            "/api/v1/auth/register",
            "",
            "",
            json!({
                "email": "password-user@example.com",
                "password": "StrongPass!2026",
                "full_name": "Password User"
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let cookie = response_cookie(&response);
        let registration = json_body(response).await;
        let csrf = registration["csrf_token"].as_str().unwrap().to_string();

        let missing = post_json(
            &app,
            "/api/v1/auth/password/change",
            &cookie,
            &csrf,
            json!({
                "new_password": "NewStrong!2026",
                "confirm_password": "NewStrong!2026"
            }),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

        let wrong = post_json(
            &app,
            "/api/v1/auth/password/change",
            &cookie,
            &csrf,
            json!({
                "current_password": "NotThePassword",
                "new_password": "NewStrong!2026",
                "confirm_password": "NewStrong!2026"
            }),
        )
        .await;
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

        let changed = post_json(
            &app,
            "/api/v1/auth/password/change",
            &cookie,
            &csrf,
            json!({
                "current_password": "StrongPass!2026",
                "new_password": "NewStrong!2026",
                "confirm_password": "NewStrong!2026"
            }),
        )
        .await;
        assert_eq!(changed.status(), StatusCode::OK);

        let me = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn mfa_disable_requires_a_current_totp_code() {
        let app = crate::server::router(AppState::new());
        let response = post_json(
            &app,
            "/api/v1/auth/register",
            "",
            "",
            json!({
                "email": "mfa-user@example.com",
                "password": "StrongPass!2026",
                "full_name": "MFA User"
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let cookie = response_cookie(&response);
        let registration = json_body(response).await;
        let csrf = registration["csrf_token"].as_str().unwrap().to_string();

        let response = post_json(&app, "/api/v1/auth/mfa/enroll", &cookie, &csrf, json!({})).await;
        assert_eq!(response.status(), StatusCode::OK);
        let enrollment = json_body(response).await;
        let secret = enrollment["secret"].as_str().unwrap().to_string();

        let now = chrono::Utc::now().timestamp() as u64;
        let code = crate::security::mfa::totp(&secret, now).unwrap();
        let response = post_json(
            &app,
            "/api/v1/auth/mfa/verify",
            &cookie,
            &csrf,
            json!({ "code": code }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        // Missing proof must not disable MFA.
        let response = post_json(&app, "/api/v1/auth/mfa/disable", &cookie, &csrf, json!({})).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(mfa_status(&app, &cookie).await);

        // A valid unused code disables it and clears the secret.
        let next = crate::security::mfa::totp(&secret, now + 30).unwrap();
        let response = post_json(
            &app,
            "/api/v1/auth/mfa/disable",
            &cookie,
            &csrf,
            json!({ "code": next }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!mfa_status(&app, &cookie).await);

        // The used code cannot be replayed after the secret is cleared.
        let response = post_json(
            &app,
            "/api/v1/auth/mfa/disable",
            &cookie,
            &csrf,
            json!({ "code": next }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn anonymous_does_not_create_another_account_for_an_active_session() {
        let state = AppState::new();
        let store = state.store.clone();
        let sessions = state.sessions.clone();
        let app = crate::server::router(state);

        let response = post_json(
            &app,
            "/api/v1/auth/anonymous",
            "",
            "",
            json!({"device_name": "test device"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let cookie = response_cookie(&response);
        let body = json_body(response).await;
        let csrf = body["csrf_token"].as_str().unwrap().to_string();

        let users_before = { store.lock().unwrap().users.len() };
        let sessions_before = { sessions.lock().unwrap().len() };

        let response = post_json(
            &app,
            "/api/v1/auth/anonymous",
            &cookie,
            &csrf,
            json!({"device_name": "test device"}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!({ store.lock().unwrap().users.len() }, users_before);
        assert_eq!({ sessions.lock().unwrap().len() }, sessions_before);
    }

    #[tokio::test]
    async fn list_sessions_is_scoped_to_one_user() {
        let state = AppState::new();
        state.create_session(1, "anonymous", false).await;
        state.create_session(2, "anonymous", false).await;

        let one = state.list_sessions(1).await;
        let two = state.list_sessions(2).await;
        assert_eq!(one.len(), 1);
        assert_eq!(two.len(), 1);
        assert!(one.iter().all(|session| session.user_id == 1));
        assert!(two.iter().all(|session| session.user_id == 2));
    }

    #[test]
    fn dummy_hash_is_a_valid_argon2id_hash() {
        let hash = super::dummy_password_hash();
        // A real Argon2id hash, so verifying against it costs what a real
        // account costs; computed once per process.
        assert!(hash.starts_with("$argon2id$"));
        assert_eq!(hash, super::dummy_password_hash());
        // Its secret is random: no guessable password, including the old
        // fixed one, unlocks it.
        for guess in ["pinpoint-login-timing", "wrong", ""] {
            assert!(!crate::security::password::verify_password(guess, hash));
        }
    }
}

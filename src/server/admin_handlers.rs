//! Admin API (`/api/v1/admin/*`) — role-gated, modular by domain.
//!
//! Every handler calls [`require_admin`]; the web console is only a view over
//! these endpoints. New admin domains (diagnostics, audit, jobs) add a handler
//! module + one route here, without touching map/social code.

use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use super::state::AppState;
use super::{error_envelope, CurrentUser, IntoResponse};

fn forbidden() -> axum::response::Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": {"code": "FORBIDDEN", "message": "Administrator role required."}})),
    )
        .into_response()
}

fn unauthenticated() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": {"code": "UNAUTHENTICATED", "message": "Sign in required."}})),
    )
        .into_response()
}

/// Fail-closed role gate shared by every admin endpoint.
#[allow(clippy::result_large_err)] // axum::Response is the error type by design
pub(crate) fn require_admin(
    state: &AppState,
    cu: Option<Extension<CurrentUser>>,
) -> Result<CurrentUser, axum::response::Response> {
    let Some(Extension(cu)) = cu else {
        return Err(unauthenticated());
    };
    let role = {
        let store = state.store.lock().unwrap();
        store.find_user_by_id(cu.user_id).map(|u| u.role.clone())
    };
    match role.as_deref() {
        Some("admin") | Some("administrator") => Ok(cu),
        Some(_) => Err(forbidden()),
        None => Err(unauthenticated()),
    }
}

fn admin_user_json(u: &crate::models::User) -> Value {
    json!({
        "id": u.id,
        "username": super::usernames::username_for(u.id),
        "email": u.email,
        "full_name": u.full_name,
        "role": u.role,
        "is_active": u.is_active,
        "must_change_password": u.must_change_password,
        "auth_method": u.auth_method,
        "mfa_enabled": u.mfa_enabled,
    })
}

/// Record an admin action in the append-only audit table (ids/handles only).
pub(crate) async fn record_admin_action(
    state: &AppState,
    actor_id: i64,
    action: &str,
    target: &str,
    detail: Option<&str>,
) {
    let actor_username = {
        let store = state.store.lock().unwrap();
        store
            .find_user_by_id(actor_id)
            .map(|u| super::usernames::username_for(u.id))
            .unwrap_or_else(|| format!("user-{actor_id}"))
    };
    state
        .record_audit(actor_id, &actor_username, action, target, detail)
        .await;
}

/// `GET /api/v1/admin/overview` — counters for the console landing page.
pub async fn overview(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&state, cu) {
        return resp;
    }
    // A replica may have started before accounts were created elsewhere.
    state.refresh_from_db().await;
    let (users, active, friendships, shares, diagnostics, audit_events) = {
        let store = state.store.lock().unwrap();
        (
            store.users.len(),
            store.users.iter().filter(|u| u.is_active).count(),
            store.friendships.len(),
            store.shares.len(),
            store
                .diagnostic_reports
                .iter()
                .filter(|r| r.status == "open")
                .count(),
            store.admin_audit.len(),
        )
    };
    let sessions = state.sessions.lock().unwrap().len();
    (
        StatusCode::OK,
        Json(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "users": users,
            "active_users": active,
            "sessions": sessions,
            "friendships": friendships,
            "shares": shares,
            "diagnostics": diagnostics,
            "audit_events": audit_events,
        })),
    )
        .into_response()
}

/// `GET /api/v1/admin/users` — private user listing (email included).
pub async fn list_users(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&state, cu) {
        return resp;
    }
    state.refresh_from_db().await;
    let store = state.store.lock().unwrap();
    let users: Vec<Value> = store.users.iter().map(admin_user_json).collect();
    (StatusCode::OK, Json(json!({ "users": users }))).into_response()
}

/// `PATCH /api/v1/admin/users/:id` — role / active flag. Self-demotion and
/// self-deactivation are refused so an admin cannot lock themselves out.
pub async fn update_user(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let admin = match require_admin(&state, cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    if admin.user_id == id {
        return error_envelope(
            "VALIDATION_FAILED",
            "Use profile settings to change your own account.",
        );
    }
    let role = body.get("role").and_then(Value::as_str).map(str::to_string);
    if let Some(role) = &role {
        if !["resident", "admin", "administrator"].contains(&role.as_str()) {
            return error_envelope("VALIDATION_FAILED", "Invalid role.");
        }
    }
    let is_active = body.get("is_active").and_then(Value::as_bool);
    let must_change = body.get("must_change_password").and_then(Value::as_bool);
    let changes = {
        let mut store = state.store.lock().unwrap();
        let Some(user) = store.users.iter_mut().find(|u| u.id == id) else {
            return error_envelope("USER_NOT_FOUND", "User not found.");
        };
        let old_role = user.role.clone();
        let old_active = user.is_active;
        let old_must_change = user.must_change_password;
        if let Some(role) = role.as_ref() {
            user.role = role.clone();
        }
        if let Some(active) = is_active {
            user.is_active = active;
        }
        if let Some(flag) = must_change {
            user.must_change_password = flag;
        }
        (
            old_role != user.role,
            old_role,
            user.role.clone(),
            old_active != user.is_active,
            old_active,
            user.is_active,
            old_must_change != user.must_change_password,
            user.must_change_password,
        )
    };
    state.persist().await;
    let (
        role_changed,
        old_role,
        new_role,
        active_changed,
        old_active,
        new_active,
        must_change_changed,
        new_must_change,
    ) = changes;
    if active_changed && !new_active {
        // A disabled account must lose every live session, not just fail the
        // next login.
        state.delete_user_sessions(id).await;
    }
    if role_changed {
        record_admin_action(
            &state,
            admin.user_id,
            "user.role_change",
            &format!("user:{id}"),
            Some(&format!("{old_role} -> {new_role}")),
        )
        .await;
    }
    if active_changed {
        record_admin_action(
            &state,
            admin.user_id,
            "user.active_change",
            &format!("user:{id}"),
            Some(&format!("{old_active} -> {new_active}")),
        )
        .await;
    }
    if must_change_changed {
        record_admin_action(
            &state,
            admin.user_id,
            "user.must_change_password",
            &format!("user:{id}"),
            Some(if new_must_change {
                "required"
            } else {
                "cleared"
            }),
        )
        .await;
    }
    (StatusCode::OK, Json(json!({"message": "User updated."}))).into_response()
}

/// `POST /api/v1/admin/users/:id/password` - issue a temporary password.
///
/// A reset revokes every session and forces a rotation at next sign-in. MFA is
/// cleared only when the admin explicitly asks, so a stolen admin session
/// cannot silently strip a second factor.
pub async fn reset_password(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let admin = match require_admin(&state, cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    if admin.user_id == id {
        return error_envelope(
            "VALIDATION_FAILED",
            "Use profile settings to change your own password.",
        );
    }
    let Some(password) = body.get("password").and_then(Value::as_str) else {
        return error_envelope("VALIDATION_FAILED", "password is required");
    };
    let check = crate::security::validation::validate_password(password);
    if !check.is_valid() {
        return error_envelope("VALIDATION_FAILED", &check.errors.join("; "));
    }
    let reset_mfa = body
        .get("reset_mfa")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let hash = crate::security::password::hash_password(
        password,
        crate::security::password::DEFAULT_ITERS,
    );
    let handle = {
        let mut store = state.store.lock().unwrap();
        let Some(user) = store.users.iter_mut().find(|u| u.id == id) else {
            return error_envelope("USER_NOT_FOUND", "User not found.");
        };
        user.password_hash = Some(hash.clone());
        user.must_change_password = true;
        user.failed_login_attempts = 0;
        user.locked_until = None;
        if reset_mfa {
            user.totp_secret = None;
            user.mfa_enabled = false;
            user.mfa_last_step = 0;
        }
        super::usernames::username_for(user.id)
    };
    if !state.sql_reset_user_auth(id, &hash, reset_mfa).await {
        return error_envelope(
            "STORAGE_UNAVAILABLE",
            "Password could not be reset. Try again.",
        );
    }
    state.delete_user_sessions(id).await;
    let detail = if reset_mfa {
        format!("@{handle} temporary password issued, MFA cleared")
    } else {
        format!("@{handle} temporary password issued")
    };
    record_admin_action(
        &state,
        admin.user_id,
        "user.password_reset",
        &format!("user:{id}"),
        Some(&detail),
    )
    .await;
    (
        StatusCode::OK,
        Json(json!({"message": "Temporary password issued."})),
    )
        .into_response()
}

/// `DELETE /api/v1/admin/users/:id` - purge the account and everything tied
/// to it, then revoke every session. Refuses self-deletion.
pub async fn delete_user(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let admin = match require_admin(&state, cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    if admin.user_id == id {
        return error_envelope(
            "VALIDATION_FAILED",
            "Cannot delete your own account from the admin console.",
        );
    }
    let handle = {
        let mut store = state.store.lock().unwrap();
        let Some(user) = store.users.iter().find(|u| u.id == id) else {
            return error_envelope("USER_NOT_FOUND", "User not found.");
        };
        // Handles only in the audit trail — no email or legal name.
        let handle = super::usernames::username_for(user.id);
        store.users.retain(|u| u.id != id);
        store
            .friendships
            .retain(|f| f.requester_id != id && f.receiver_id != id);
        store.shares.retain(|s| s.owner_id != id);
        store.locations.retain(|l| l.user_id != id);
        store.check_ins.retain(|c| c.user_id != id);
        store.emergencies.retain(|e| e.user_id != id);
        handle
    };
    state.delete_user_sessions(id).await;
    state.sql_delete_user(id).await;
    state.persist().await;
    record_admin_action(
        &state,
        admin.user_id,
        "user.delete",
        &format!("user:{id}"),
        Some(&format!("@{handle} deleted")),
    )
    .await;
    (StatusCode::OK, Json(json!({"message": "User deleted."}))).into_response()
}

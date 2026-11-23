//! Users + friendships service handlers.
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use super::admin_handlers::{record_admin_action, require_admin};
use super::auth_handlers::clear_cookie;
use super::handlers::{not_found, require_current_user, user_json};
use super::state::AppState;
use super::usernames::{find_user_by_username, public_json, username_for};
use super::{error_envelope, CurrentUser, IntoResponse};

/// Friend-visible profile: real name + email are revealed only after the
/// friendship is accepted. `role` and auth internals stay private.
fn friend_json(u: &crate::models::User) -> Value {
    json!({
        "id": u.id,
        "username": username_for(u.id),
        "display_name": u.full_name,
        "full_name": u.full_name,
        "email": u.email,
        "auth_method": u.auth_method,
    })
}

/// `GET /users/` — admin-only private listing.
pub async fn list_users(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&state, cu) {
        return resp;
    }
    let store = state.store.lock().unwrap();
    let users: Vec<Value> = store
        .users
        .iter()
        .map(|u| {
            json!({
                "id": u.id,
                "username": username_for(u.id),
                "email": u.email,
                "full_name": u.full_name,
                "role": u.role,
                "is_active": u.is_active,
                "must_change_password": u.must_change_password,
            })
        })
        .collect();
    (StatusCode::OK, Json(users)).into_response()
}

/// `POST /users/` (admin) — create user.
pub async fn create_user(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&state, cu) {
        return resp;
    }
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let full_name = body
        .get("full_name")
        .and_then(Value::as_str)
        .unwrap_or("New User")
        .to_string();
    let email_check = crate::security::validation::validate_email(&email);
    if !email_check.is_valid() {
        return error_envelope("VALIDATION_FAILED", &email_check.errors.join("; "));
    }
    // No shared default: an admin-created account gets a temporary password
    // that the user must replace at first sign-in.
    let Some(password) = body.get("password").and_then(Value::as_str) else {
        return error_envelope("VALIDATION_FAILED", "password is required");
    };
    let password_check = crate::security::validation::validate_password(password);
    if !password_check.is_valid() {
        return error_envelope("VALIDATION_FAILED", &password_check.errors.join("; "));
    }
    {
        let store = state.store.lock().unwrap();
        if store.find_user_by_email(&email).is_some() {
            return error_envelope("VALIDATION_FAILED", "email already registered");
        }
    }
    let role: String = body
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("resident")
        .to_string();
    let hash = crate::security::password::hash_password(
        password,
        crate::security::password::DEFAULT_ITERS,
    );
    // Direct SQL insert (Auth owns wipe-rewrites of this table). The forced
    // change flag is written with the row, not only into the in-memory copy,
    // so a restart or another replica keeps the temporary-password rule.
    let id = state
        .sql_insert_user(&email, &full_name, &role, Some(&hash), true)
        .await;
    let user = crate::models::User {
        id,
        username: String::new(),
        email: email.clone(),
        full_name: full_name.clone(),
        role: role.clone(),
        is_active: true,
        must_change_password: true,
        password_hash: Some(hash),
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
        store.users.push(user.clone());
    }
    (
        StatusCode::CREATED,
        Json(json!({
            "id": id,
            "email": email,
            "full_name": full_name,
            "role": user.role,
        })),
    )
        .into_response()
}

/// `GET /users/{id}` — self/admin get the private record, accepted friends get
/// the friend profile, pending contacts get the public card, everyone else
/// gets 404 so numeric ids cannot be walked.
pub async fn get_user(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let store = state.store.lock().unwrap();
    let Some(user) = store.find_user_by_id(id) else {
        return not_found("USER_NOT_FOUND", "User not found.");
    };
    let viewer_is_admin = store
        .find_user_by_id(cu.user_id)
        .is_some_and(|u| matches!(u.role.as_str(), "admin" | "administrator"));
    if cu.user_id == id || viewer_is_admin {
        return (StatusCode::OK, Json(user_json(user))).into_response();
    }
    let relation = store.friendships.iter().find(|f| {
        (f.requester_id == cu.user_id && f.receiver_id == id)
            || (f.receiver_id == cu.user_id && f.requester_id == id)
    });
    match relation.map(|f| f.status.as_str()) {
        Some("accepted") => (StatusCode::OK, Json(friend_json(user))).into_response(),
        Some("pending") => (StatusCode::OK, Json(public_json(user))).into_response(),
        _ => not_found("USER_NOT_FOUND", "User not found."),
    }
}

/// `GET /users/lookup?username=` — exact, case-insensitive handle lookup.
///
/// This is the only discovery path for non-admins: no substring, name or
/// email search, and the response is the public card (no email/legal name).
pub async fn lookup_user(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    q: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let wanted = q.get("username").map(String::as_str).unwrap_or("").trim();
    if wanted.is_empty() {
        return error_envelope("VALIDATION_FAILED", "username is required");
    }
    let store = state.store.lock().unwrap();
    let Some(user) = find_user_by_username(&store.users, wanted) else {
        return not_found("USER_NOT_FOUND", "No account with that username.");
    };
    let friendship_status = store
        .friendships
        .iter()
        .find(|f| {
            (f.requester_id == cu.user_id && f.receiver_id == user.id)
                || (f.receiver_id == cu.user_id && f.requester_id == user.id)
        })
        .map(|f| f.status.clone())
        .unwrap_or_default();
    let mut card = public_json(user);
    if let Some(obj) = card.as_object_mut() {
        obj.insert("is_self".into(), json!(user.id == cu.user_id));
        obj.insert("friendship_status".into(), json!(friendship_status));
    }
    (StatusCode::OK, Json(card)).into_response()
}

/// `PATCH /users/{id}` — real partial update (profile + admin).
/// In-memory store is the view; direct SQL mirrors it (Auth owns the
/// wipe-rewrite, this service upserts single rows).
pub async fn update_user(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let full_name = body
        .get("full_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(name) = &full_name {
        if name.trim().is_empty() {
            return error_envelope("VALIDATION_FAILED", "full_name cannot be empty");
        }
    }
    let role = body.get("role").and_then(Value::as_str).map(str::to_string);
    if let Some(r) = &role {
        if !["resident", "admin", "administrator"].contains(&r.as_str()) {
            return error_envelope(
                "VALIDATION_FAILED",
                "role must be resident, admin, or administrator",
            );
        }
    }
    let is_active = body.get("is_active").and_then(Value::as_bool);
    let must_change = body.get("must_change_password").and_then(Value::as_bool);

    let privileged =
        id != cu.user_id || role.is_some() || is_active.is_some() || must_change.is_some();
    if privileged {
        if let Err(resp) = require_admin(&state, Some(Extension(cu.clone()))) {
            return resp;
        }
        if id == cu.user_id && (role.is_some() || is_active.is_some()) {
            return error_envelope(
                "VALIDATION_FAILED",
                "Use profile settings to change your own role or active state.",
            );
        }
    }
    let (old_role, old_active) = {
        let mut store = state.store.lock().unwrap();
        let user = match store.users.iter_mut().find(|u| u.id == id) {
            Some(u) => u,
            None => return error_envelope("USER_NOT_FOUND", "User not found."),
        };
        let old = (user.role.clone(), user.is_active);
        if let Some(name) = full_name.as_ref() {
            user.full_name = name.clone();
        }
        if let Some(r) = role.as_ref() {
            user.role = r.clone();
        }
        if let Some(a) = is_active {
            user.is_active = a;
        }
        if let Some(flag) = must_change {
            user.must_change_password = flag;
        }
        old
    };
    state
        .sql_update_user(id, full_name.as_deref(), role.as_deref(), is_active)
        .await;
    if privileged {
        if let Some(r) = role.as_deref() {
            record_admin_action(
                &state,
                cu.user_id,
                "user.role_change",
                &format!("user:{id}"),
                Some(&format!("{old_role} -> {r}")),
            )
            .await;
        }
        if let Some(a) = is_active {
            record_admin_action(
                &state,
                cu.user_id,
                "user.active_change",
                &format!("user:{id}"),
                Some(&format!("{old_active} -> {a}")),
            )
            .await;
        }
    }
    let store = state.store.lock().unwrap();
    let view = user_json(store.find_user_by_id(id).unwrap());
    (StatusCode::OK, Json(view)).into_response()
}

/// Remove a user and every row owned by them. Returns the public handle when
/// the user existed. Sessions are revoked before the row is deleted.
async fn purge_user(state: &AppState, id: i64) -> Option<String> {
    let handle = {
        let mut store = state.store.lock().unwrap();
        let pos = store.users.iter().position(|u| u.id == id)?;
        let handle = username_for(store.users[pos].id);
        store.users.remove(pos);
        store
            .friendships
            .retain(|f| f.requester_id != id && f.receiver_id != id);
        store.shares.retain(|s| s.owner_id != id);
        store.locations.retain(|l| l.user_id != id);
        store.check_ins.retain(|c| c.user_id != id);
        store.emergencies.retain(|e| e.user_id != id);
        store.location_history.retain(|p| p.user_id != id);
        store.place_notes.retain(|n| n.user_id != id);
        store.message_keys.remove(&id);
        store
            .messages
            .retain(|m| m.sender_id != id && m.recipient_id != id);
        handle
    };
    state.delete_user_sessions(id).await;
    state.sql_delete_user(id).await;
    Some(handle)
}

/// `DELETE /users/{id}` - admin removal (cascades friendships/shares/locations).
pub async fn delete_user(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let cu = match require_admin(&state, cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    if cu.user_id == id {
        return error_envelope(
            "VALIDATION_FAILED",
            "Use the self-service account deletion endpoint.",
        );
    }
    let Some(handle) = purge_user(&state, id).await else {
        return error_envelope("USER_NOT_FOUND", "User not found.");
    };
    record_admin_action(
        &state,
        cu.user_id,
        "user.delete",
        &format!("user:{id}"),
        Some(&format!("@{handle} deleted")),
    )
    .await;
    (StatusCode::OK, Json(json!({"message": "User deleted."}))).into_response()
}

/// `DELETE /users/me` - delete the signed-in user's account and data.
pub async fn delete_me(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let confirm = body.get("confirm").and_then(Value::as_str).unwrap_or("");
    if confirm != "DELETE" {
        return error_envelope(
            "VALIDATION_FAILED",
            "Type DELETE to confirm account deletion.",
        );
    }
    let hash = {
        let store = state.store.lock().unwrap();
        store
            .find_user_by_id(cu.user_id)
            .and_then(|u| u.password_hash.clone())
    };
    if let Some(hash) = hash {
        let current = body
            .get("current_password")
            .and_then(Value::as_str)
            .unwrap_or("");
        if current.is_empty() || !crate::security::password::verify_password(current, &hash) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": {"code": "INVALID_CREDENTIALS", "message": "Current password is incorrect."}})),
            )
                .into_response();
        }
    }
    // Do not write a new audit row after erasure: the append-only audit table
    // can reject the cleanup DELETE, so a row written here would outlive the
    // account. Admin-initiated deletions are still audited under the admin.
    if purge_user(&state, cu.user_id).await.is_none() {
        return error_envelope("USER_NOT_FOUND", "User not found.");
    }
    let mut resp = (StatusCode::OK, Json(json!({"message": "Account deleted."}))).into_response();
    resp.headers_mut()
        .insert("Set-Cookie", clear_cookie().parse().unwrap());
    resp
}

/// `GET /users/search?q=` — admin-only substring search over handle/email/name.
/// Non-admins use the exact `/users/lookup` endpoint instead.
pub async fn search_users(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    q: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&state, cu) {
        return resp;
    }
    let needle = q.get("q").cloned().unwrap_or_default().to_lowercase();
    let store = state.store.lock().unwrap();
    let hits: Vec<Value> = store
        .users
        .iter()
        .filter(|u| {
            needle.is_empty()
                || username_for(u.id).contains(&needle)
                || u.email.to_lowercase().contains(&needle)
                || u.full_name.to_lowercase().contains(&needle)
        })
        .map(user_json)
        .collect();
    (StatusCode::OK, Json(hits)).into_response()
}

/// `POST /friendships/` — send friend request by exact username.
///
/// `receiver_username` is the public path. `receiver_email` remains accepted
/// for the desktop client (exact match only) and is never echoed in a card.
pub async fn create_friendship(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let receiver_username = body
        .get("receiver_username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .trim_start_matches('@')
        .to_string();
    let receiver_email = body
        .get("receiver_email")
        .and_then(Value::as_str)
        .unwrap_or("");
    let friendship = {
        let mut store = state.store.lock().unwrap();
        let receiver = if !receiver_username.is_empty() {
            match find_user_by_username(&store.users, &receiver_username) {
                Some(u) => u.clone(),
                None => return error_envelope("USER_NOT_FOUND", "No account with that username."),
            }
        } else if !receiver_email.is_empty() {
            match store.find_user_by_email(receiver_email) {
                Some(u) => u.clone(),
                None => return error_envelope("USER_NOT_FOUND", "User not found."),
            }
        } else {
            return error_envelope(
                "VALIDATION_FAILED",
                "receiver_username is required (receiver_email is legacy).",
            );
        };
        if !receiver.is_active {
            return error_envelope("USER_NOT_FOUND", "User not found.");
        }
        let requester = match store.find_user_by_id(cu.user_id).cloned() {
            Some(u) => u,
            None => return error_envelope("USER_NOT_FOUND", "User not found."),
        };
        if requester.id == receiver.id {
            return error_envelope(
                "VALIDATION_FAILED",
                "Cannot send a friend request to yourself.",
            );
        }
        let existing = store.friendships.iter().any(|f| {
            f.status != "rejected"
                && ((f.requester_id == requester.id && f.receiver_id == receiver.id)
                    || (f.requester_id == receiver.id && f.receiver_id == requester.id))
        });
        if existing {
            return error_envelope(
                "FRIENDSHIP_EXISTS",
                "A friendship or pending request already exists.",
            );
        }
        let id = store.next_friendship_id;
        store.next_friendship_id += 1;
        // Handles, not legal names: a pending request must not reveal the
        // requester's real name before the receiver accepts.
        let friendship = crate::models::Friendship {
            id,
            requester_id: requester.id,
            receiver_id: receiver.id,
            requester_name: username_for(requester.id),
            receiver_name: username_for(receiver.id),
            status: "pending".into(),
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            responded_at: None,
        };
        store.friendships.push(friendship.clone());
        friendship
    };
    state.sql_upsert_friendship(&friendship).await;
    (
        StatusCode::CREATED,
        Json(json!({
            "id": friendship.id,
            "requester_id": friendship.requester_id,
            "receiver_id": friendship.receiver_id,
            "requester_name": friendship.requester_name,
            "receiver_name": friendship.receiver_name,
            "status": friendship.status,
        })),
    )
        .into_response()
}

/// `GET /friendships/`.
pub async fn list_friendships(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let store = state.store.lock().unwrap();
    let friendships: Vec<Value> = store
        .friendships
        .iter()
        .filter(|f| f.requester_id == cu.user_id || f.receiver_id == cu.user_id)
        .map(|f| {
            json!({
                "id": f.id,
                "requester_id": f.requester_id,
                "receiver_id": f.receiver_id,
                "requester_name": f.requester_name,
                "receiver_name": f.receiver_name,
                "status": f.status,
            })
        })
        .collect();
    (StatusCode::OK, Json(friendships)).into_response()
}

/// `POST /friendships/{id}/respond`.
pub async fn respond_friendship(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let action = body.get("action").and_then(Value::as_str).unwrap_or("");
    let updated = {
        let mut store = state.store.lock().unwrap();
        let friendship = match store.friendships.iter_mut().find(|f| f.id == id) {
            Some(f) if f.receiver_id == cu.user_id => f,
            Some(_) => return error_envelope("FRIENDSHIP_NOT_FOUND", "Friendship not found."),
            None => return error_envelope("FRIENDSHIP_NOT_FOUND", "Friendship not found."),
        };
        friendship.status = match action {
            "accept" => "accepted".into(),
            "reject" => "rejected".into(),
            _ => return error_envelope("VALIDATION_FAILED", "action must be accept or reject"),
        };
        friendship.responded_at = Some(chrono::Utc::now().to_rfc3339());
        friendship.clone()
    };
    state.sql_upsert_friendship(&updated).await;
    let resp = json!({
        "id": updated.id,
        "requester_id": updated.requester_id,
        "receiver_id": updated.receiver_id,
        "requester_name": updated.requester_name,
        "receiver_name": updated.receiver_name,
        "status": updated.status,
    });
    (StatusCode::OK, Json(resp)).into_response()
}

/// `DELETE /friendships/{id}` — unfriend / remove request (cascades shares).
pub async fn delete_friendship(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    {
        let mut store = state.store.lock().unwrap();
        let pos = match store.friendships.iter().position(|f| {
            f.id == id && (f.requester_id == cu.user_id || f.receiver_id == cu.user_id)
        }) {
            Some(p) => p,
            None => return error_envelope("FRIENDSHIP_NOT_FOUND", "Friendship not found."),
        };
        store.friendships.remove(pos);
        store.shares.retain(|s| s.friendship_id != id);
    }
    state.sql_delete_friendship(id).await;
    (
        StatusCode::OK,
        Json(json!({"message": "Friendship removed."})),
    )
        .into_response()
}

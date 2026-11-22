//! E2E messaging endpoints. The server sees opaque envelopes and public
//! bundles only; authorization is accepted-friends-only.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use std::sync::{Mutex, OnceLock};

use super::handlers::{not_found, require_current_user};
use super::state::AppState;
use super::{error_envelope, CurrentUser, IntoResponse};
use crate::models::Message;
use crate::security::message_crypto::{MessagePublicBundle, SealedMessage};
use crate::security::rate_limit::{RateLimitResult, RateLimiter};

const MAX_MESSAGES_PER_MINUTE: u32 = 60;

fn message_limiter() -> &'static Mutex<RateLimiter> {
    static LIMITER: OnceLock<Mutex<RateLimiter>> = OnceLock::new();
    LIMITER.get_or_init(|| {
        let mut limiter = RateLimiter::with_defaults();
        limiter.register_config(
            "message_send",
            crate::security::rate_limit::RateLimitConfig {
                max_requests: MAX_MESSAGES_PER_MINUTE,
                window_seconds: 60,
            },
        );
        Mutex::new(limiter)
    })
}

fn rate_limited() -> axum::response::Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({"error": {"code": "RATE_LIMITED", "message": "Too many messages; wait a moment."}})),
    )
        .into_response()
}

fn forbidden(code: &str, message: &str) -> axum::response::Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

fn conflict(code: &str, message: &str) -> axum::response::Response {
    (
        StatusCode::CONFLICT,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

fn valid_fixed(value: &str, bytes: usize) -> bool {
    B64.decode(value)
        .map(|raw| raw.len() == bytes)
        .unwrap_or(false)
}

fn valid_ciphertext(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= super::state::messages::MAX_ENVELOPE_FIELD_BYTES
        && B64.decode(value).is_ok()
}

fn valid_envelope(sealed: &SealedMessage) -> bool {
    valid_fixed(&sealed.ephemeral_public_key, 32)
        && valid_fixed(&sealed.salt_to, 16)
        && valid_fixed(&sealed.nonce_to, 12)
        && valid_fixed(&sealed.salt_from, 16)
        && valid_fixed(&sealed.nonce_from, 12)
        && valid_fixed(&sealed.signature, 64)
        && valid_ciphertext(&sealed.ciphertext_to)
        && valid_ciphertext(&sealed.ciphertext_from)
}

/// `PUT /messages/key` - publish or rotate this device's public bundle.
pub async fn set_key(
    State(state): State<AppState>,
    cu: Option<axum::extract::Extension<CurrentUser>>,
    Json(bundle): Json<MessagePublicBundle>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(response) => return response,
    };
    if !bundle.is_valid() {
        return error_envelope(
            "INVALID_KEY_BUNDLE",
            "x25519 and ed25519 must be 32-byte base64 keys.",
        );
    }
    let Ok(raw) = serde_json::to_string(&bundle) else {
        return error_envelope("INVALID_KEY_BUNDLE", "Could not encode the key bundle.");
    };
    if !state.set_message_key(cu.user_id, raw).await {
        return error_envelope("KEY_STORE_FAILED", "Could not store the key bundle.");
    }
    (
        StatusCode::OK,
        Json(json!({ "user_id": cu.user_id, "key": bundle })),
    )
        .into_response()
}

/// `GET /messages/key/:user_id` - own or accepted friend's public bundle.
pub async fn get_key(
    State(state): State<AppState>,
    cu: Option<axum::extract::Extension<CurrentUser>>,
    Path(user_id): Path<i64>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(response) => return response,
    };
    if user_id != cu.user_id && !state.are_accepted_friends(cu.user_id, user_id) {
        return not_found("MESSAGE_KEY_NOT_FOUND", "No message key available.");
    }
    let Some(raw) = state.message_key(user_id) else {
        return not_found("MESSAGE_KEY_NOT_FOUND", "No message key available.");
    };
    let Ok(bundle) = serde_json::from_str::<MessagePublicBundle>(&raw) else {
        return not_found("MESSAGE_KEY_NOT_FOUND", "No message key available.");
    };
    (
        StatusCode::OK,
        Json(json!({ "user_id": user_id, "key": bundle })),
    )
        .into_response()
}

#[derive(Deserialize)]
pub(crate) struct SendBody {
    recipient_id: i64,
    message: SealedMessage,
}

/// `POST /messages` - store one signed envelope for an accepted friend.
pub async fn send(
    State(state): State<AppState>,
    cu: Option<axum::extract::Extension<CurrentUser>>,
    Json(body): Json<SendBody>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(response) => return response,
    };
    if body.recipient_id <= 0 || body.recipient_id == cu.user_id {
        return error_envelope("VALIDATION_FAILED", "recipient_id must be another user.");
    }
    if !state.are_accepted_friends(cu.user_id, body.recipient_id) {
        return forbidden("NOT_FRIENDS", "Messages require an accepted friendship.");
    }
    if state.message_key(body.recipient_id).is_none() {
        return conflict(
            "NO_MESSAGE_KEY",
            "This friend has not opened Pinpoint with encrypted messages yet.",
        );
    }
    if !valid_envelope(&body.message) {
        return error_envelope("INVALID_ENVELOPE", "The sealed message is malformed.");
    }
    {
        let limiter = message_limiter().lock().unwrap();
        if matches!(
            limiter.check(&format!("message_send:{}", cu.user_id)),
            RateLimitResult::RateLimited { .. }
        ) {
            return rate_limited();
        }
    }
    let message = Message {
        id: 0,
        sender_id: cu.user_id,
        recipient_id: body.recipient_id,
        ephemeral_public_key: body.message.ephemeral_public_key,
        salt_to: body.message.salt_to,
        nonce_to: body.message.nonce_to,
        ciphertext_to: body.message.ciphertext_to,
        salt_from: body.message.salt_from,
        nonce_from: body.message.nonce_from,
        ciphertext_from: body.message.ciphertext_from,
        signature: body.message.signature,
        created_at: chrono::Utc::now().to_rfc3339(),
        read_at: None,
    };
    match state.send_message(message).await {
        Some(saved) => (StatusCode::CREATED, Json(json!({ "message": saved }))).into_response(),
        None => conflict(
            "MESSAGE_REPLAY",
            "This envelope was already delivered or could not be stored.",
        ),
    }
}

#[derive(Deserialize)]
pub(crate) struct Page {
    before_id: Option<i64>,
    limit: Option<usize>,
}

/// `GET /messages/:friend_id` - the thread and the peer's public bundle.
pub async fn thread(
    State(state): State<AppState>,
    cu: Option<axum::extract::Extension<CurrentUser>>,
    Path(friend_id): Path<i64>,
    Query(page): Query<Page>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(response) => return response,
    };
    if !state.are_accepted_friends(cu.user_id, friend_id) {
        return forbidden("NOT_FRIENDS", "Messages require an accepted friendship.");
    }
    let limit = page.limit.unwrap_or(50).clamp(1, 100);
    let Some(thread) = state.message_thread_for(cu.user_id, friend_id, page.before_id, limit)
    else {
        return forbidden("NOT_FRIENDS", "Messages require an accepted friendship.");
    };
    (StatusCode::OK, Json(thread)).into_response()
}

/// `GET /messages/conversations` - inbox rows with unread counters only.
pub async fn conversations(
    State(state): State<AppState>,
    cu: Option<axum::extract::Extension<CurrentUser>>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(response) => return response,
    };
    (
        StatusCode::OK,
        Json(json!({ "conversations": state.message_conversations(cu.user_id) })),
    )
        .into_response()
}

#[derive(Deserialize)]
pub(crate) struct ReadBody {
    friend_id: i64,
}

/// `POST /messages/read` - mark a friend's messages as read for this user.
pub async fn mark_read(
    State(state): State<AppState>,
    cu: Option<axum::extract::Extension<CurrentUser>>,
    Json(body): Json<ReadBody>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(response) => return response,
    };
    if !state.are_accepted_friends(cu.user_id, body.friend_id) {
        return forbidden("NOT_FRIENDS", "Messages require an accepted friendship.");
    }
    let updated = state.mark_messages_read(cu.user_id, body.friend_id).await;
    (StatusCode::OK, Json(json!({ "updated": updated }))).into_response()
}

/// `GET /messages/unread` - total unread count for the header badge.
pub async fn unread(
    State(state): State<AppState>,
    cu: Option<axum::extract::Extension<CurrentUser>>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(response) => return response,
    };
    let total: i64 = state
        .message_conversations(cu.user_id)
        .iter()
        .map(|conversation| conversation.unread)
        .sum();
    (StatusCode::OK, Json(json!({ "unread": total }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MessageThread;
    use crate::security::message_crypto::{open, seal, MessageIdentity};
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt;

    async fn request_json(
        app: &axum::Router,
        method: &str,
        uri: &str,
        cookie: &str,
        csrf: &str,
        body: Option<Value>,
    ) -> axum::response::Response {
        let mut builder = Request::builder().method(method).uri(uri);
        if !cookie.is_empty() {
            builder = builder.header("cookie", cookie);
        }
        if !csrf.is_empty() {
            builder = builder.header("x-csrf-token", csrf);
        }
        let body = match body {
            Some(value) => {
                builder = builder.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        app.clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap()
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
            .expect("session cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    async fn register(app: &axum::Router, email: &str, name: &str) -> (String, String, i64) {
        let response = request_json(
            app,
            "POST",
            "/api/v1/auth/register",
            "",
            "",
            Some(serde_json::json!({
                "email": email,
                "password": "StrongPass!2026",
                "full_name": name
            })),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let cookie = response_cookie(&response);
        let body = json_body(response).await;
        (
            cookie,
            body["csrf_token"].as_str().unwrap().to_string(),
            body["user"]["id"].as_i64().unwrap(),
        )
    }

    async fn publish_key(app: &axum::Router, cookie: &str, csrf: &str, identity: &MessageIdentity) {
        let response = request_json(
            app,
            "PUT",
            "/api/v1/messages/key",
            cookie,
            csrf,
            Some(serde_json::to_value(identity.public_bundle()).unwrap()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn e2e_message_round_trip_read_state_and_authorization() {
        let app = crate::server::router(AppState::new());
        let (alice_cookie, alice_csrf, alice_id) =
            register(&app, "alice-msg@example.com", "Alice").await;
        let (bob_cookie, bob_csrf, bob_id) = register(&app, "bob-msg@example.com", "Bob").await;
        let (carol_cookie, carol_csrf, carol_id) =
            register(&app, "carol-msg@example.com", "Carol").await;

        // Accepted friendship is the message authorization boundary.
        let response = request_json(
            &app,
            "POST",
            "/api/v1/friendships/",
            &alice_cookie,
            &alice_csrf,
            Some(serde_json::json!({"receiver_email": "bob-msg@example.com"})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let friendship = json_body(response).await["id"].as_i64().unwrap();
        let response = request_json(
            &app,
            "POST",
            &format!("/api/v1/friendships/{friendship}/respond"),
            &bob_cookie,
            &bob_csrf,
            Some(serde_json::json!({"action": "accept"})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let alice_identity = MessageIdentity::generate();
        let bob_identity = MessageIdentity::generate();
        publish_key(&app, &alice_cookie, &alice_csrf, &alice_identity).await;
        publish_key(&app, &bob_cookie, &bob_csrf, &bob_identity).await;

        let sealed = seal(
            &alice_identity,
            &bob_identity.public_bundle(),
            alice_id,
            bob_id,
            "Meet at the fountain",
        )
        .unwrap();
        let response = request_json(
            &app,
            "POST",
            "/api/v1/messages",
            &alice_cookie,
            &alice_csrf,
            Some(serde_json::json!({"recipient_id": bob_id, "message": sealed})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let sent = json_body(response).await;
        let message_id = sent["message"]["id"].as_i64().unwrap();

        // Recipient can fetch, verify and decrypt.
        let response = request_json(
            &app,
            "GET",
            &format!("/api/v1/messages/{alice_id}"),
            &bob_cookie,
            "",
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let thread: MessageThread = serde_json::from_value(json_body(response).await).unwrap();
        assert_eq!(thread.peer.user_id, alice_id);
        assert_eq!(
            thread.peer.key.as_ref().unwrap(),
            &alice_identity.public_bundle()
        );
        let envelope = thread
            .messages
            .iter()
            .find(|message| message.id == message_id)
            .unwrap();
        assert_eq!(
            open(
                &bob_identity,
                thread.peer.key.as_ref().unwrap(),
                alice_id,
                bob_id,
                &envelope.sealed(),
                true
            )
            .unwrap(),
            "Meet at the fountain"
        );

        // Unread state.
        let response = request_json(
            &app,
            "GET",
            "/api/v1/messages/unread",
            &bob_cookie,
            "",
            None,
        )
        .await;
        assert_eq!(json_body(response).await["unread"], 1);
        let response = request_json(
            &app,
            "POST",
            "/api/v1/messages/read",
            &bob_cookie,
            &bob_csrf,
            Some(serde_json::json!({"friend_id": alice_id})),
        )
        .await;
        assert_eq!(json_body(response).await["updated"], 1);
        let response = request_json(
            &app,
            "GET",
            "/api/v1/messages/unread",
            &bob_cookie,
            "",
            None,
        )
        .await;
        assert_eq!(json_body(response).await["unread"], 0);

        // Non-friends can neither read the key nor the thread nor send.
        let response = request_json(
            &app,
            "GET",
            &format!("/api/v1/messages/key/{bob_id}"),
            &carol_cookie,
            "",
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = request_json(
            &app,
            "GET",
            &format!("/api/v1/messages/{bob_id}"),
            &carol_cookie,
            "",
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = request_json(
            &app,
            "POST",
            "/api/v1/messages",
            &carol_cookie,
            &carol_csrf,
            Some(serde_json::json!({"recipient_id": bob_id, "message": sealed})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let _ = carol_id;
    }

    #[tokio::test]
    async fn malformed_and_replayed_envelopes_are_rejected() {
        let app = crate::server::router(AppState::new());
        let (alice_cookie, alice_csrf, alice_id) =
            register(&app, "alice-replay@example.com", "Alice").await;
        let (bob_cookie, bob_csrf, bob_id) = register(&app, "bob-replay@example.com", "Bob").await;
        let response = request_json(
            &app,
            "POST",
            "/api/v1/friendships/",
            &alice_cookie,
            &alice_csrf,
            Some(serde_json::json!({"receiver_email": "bob-replay@example.com"})),
        )
        .await;
        let friendship = json_body(response).await["id"].as_i64().unwrap();
        let _ = request_json(
            &app,
            "POST",
            &format!("/api/v1/friendships/{friendship}/respond"),
            &bob_cookie,
            &bob_csrf,
            Some(serde_json::json!({"action": "accept"})),
        )
        .await;
        let alice_identity = MessageIdentity::generate();
        let bob_identity = MessageIdentity::generate();
        publish_key(&app, &alice_cookie, &alice_csrf, &alice_identity).await;
        publish_key(&app, &bob_cookie, &bob_csrf, &bob_identity).await;

        let sealed = seal(
            &alice_identity,
            &bob_identity.public_bundle(),
            alice_id,
            bob_id,
            "one",
        )
        .unwrap();
        let response = request_json(
            &app,
            "POST",
            "/api/v1/messages",
            &alice_cookie,
            &alice_csrf,
            Some(serde_json::json!({"recipient_id": bob_id, "message": sealed})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = request_json(
            &app,
            "POST",
            "/api/v1/messages",
            &alice_cookie,
            &alice_csrf,
            Some(serde_json::json!({"recipient_id": bob_id, "message": sealed})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let mut malformed = serde_json::to_value(sealed).unwrap();
        malformed["signature"] = serde_json::json!("not-base64");
        let response = request_json(
            &app,
            "POST",
            "/api/v1/messages",
            &alice_cookie,
            &alice_csrf,
            Some(serde_json::json!({"recipient_id": bob_id, "message": malformed})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

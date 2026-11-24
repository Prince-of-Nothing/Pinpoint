//! OAuth 2.0 sign-in (authorization code + PKCE) for the web client.
//!
//! The flow is deliberately narrow: a provider identity is bound to
//! `identity_subject`, never to an email that already has a password account,
//! so a provider compromise cannot take over an existing login. The session is
//! the same opaque cookie as password sign-in (ADR-0012).

use axum::{
    extract::{OriginalUri, Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::User;
use crate::security::oauth::{is_secure_provider_url, OAuthClient};
use crate::server::auth_handlers::{request_is_secure, session_cookie};
use crate::server::state::AppState;

const STATE_COOKIE: &str = "pinpoint_oauth_state";
const STATE_PATH: &str = "/api/v1/auth/oauth";
const STATE_MAX_AGE_SECS: u64 = 600;

/// `GET /auth/providers` — enabled providers for the sign-in buttons.
pub async fn providers(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({ "providers": state.oauth_providers() }))
}

/// `GET /auth/oauth/:provider/start` — redirect to the provider with a
/// single-use state and a PKCE challenge; the state is also bound to a
/// short-lived HttpOnly cookie so another site cannot inject a callback.
pub async fn start(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
) -> Response {
    let Some((token, url)) = state.begin_oauth(&provider) else {
        return oauth_error("provider_unknown");
    };
    let secure = request_is_secure(&headers, &uri);
    let mut resp = Redirect::to(&url).into_response();
    resp.headers_mut().append(
        header::SET_COOKIE,
        state_cookie(&token, secure).parse().unwrap(),
    );
    resp
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /auth/oauth/:provider/callback` — verify the state, exchange the code
/// server-side, then sign the user in with the normal session cookie.
pub async fn callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if query.error.is_some() {
        return oauth_error("provider_denied");
    }
    let (Some(code), Some(expected)) = (query.code, query.state) else {
        return oauth_error("state_mismatch");
    };
    if cookie_value(&headers, STATE_COOKIE).as_deref() != Some(expected.as_str()) {
        return oauth_error("state_mismatch");
    }
    let Some(verifier) = state.take_oauth_state(&expected, &provider) else {
        return oauth_error("state_mismatch");
    };
    let Some(client) = state.oauth_client(&provider) else {
        return oauth_error("provider_unknown");
    };
    let user = match complete_sign_in(&state, &client, &code, &verifier).await {
        Ok(user) => user,
        Err(reason) => {
            tracing::warn!(provider, reason, "oauth sign-in failed");
            return oauth_error(reason);
        }
    };
    let (session_id, _csrf) = state.create_session(user.id, "oauth", false).await;
    let secure = request_is_secure(&headers, &uri);
    signed_in(&session_id, secure)
}

async fn complete_sign_in(
    state: &AppState,
    client: &OAuthClient,
    code: &str,
    verifier: &str,
) -> Result<User, &'static str> {
    let http = reqwest::Client::new();
    let form = [
        ("client_id", client.provider.client_id.as_str()),
        ("client_secret", client.client_secret.as_str()),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", client.redirect_uri.as_str()),
        ("code_verifier", verifier),
    ];
    if !is_secure_provider_url(&client.provider.token_url) {
        return Err("invalid_provider_url");
    }
    let token_response = http
        .post(&client.provider.token_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "pinpoint-oauth/1.0")
        .form(&form)
        .send()
        .await
        .map_err(|_| "exchange_failed")?;
    if !token_response.status().is_success() {
        return Err("exchange_failed");
    }
    let token: Value = token_response.json().await.map_err(|_| "exchange_failed")?;
    let access_token = token
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or("exchange_failed")?;

    let identity_info = fetch_identity(&http, client, access_token).await?;
    let identity = format!("{}:{}", client.provider.name, identity_info.subject);
    let email = identity_info.email;
    let full_name = identity_info.name;

    let existing = {
        let store = state.store.lock().unwrap();
        store.find_user_by_identity(&identity).cloned()
    };
    if let Some(user) = existing {
        return if user.is_active {
            Ok(user)
        } else {
            Err("account_disabled")
        };
    }
    // No auto-link by email: a provider identity must never take over an
    // existing password account (an email change at the provider would
    // otherwise become an account takeover).
    if state
        .store
        .lock()
        .unwrap()
        .find_user_by_email(&email)
        .is_some()
    {
        return Err("email_exists");
    }

    let id = match state
        .sql_insert_oauth_user(&email, &full_name, "oauth", &identity)
        .await
    {
        Ok(id) => id,
        Err(_) => {
            // A concurrent callback may have won the unique index race.
            let store = state.store.lock().unwrap();
            return store
                .find_user_by_identity(&identity)
                .cloned()
                .ok_or("account_failed");
        }
    };
    let user = User {
        id,
        username: String::new(),
        email,
        full_name,
        role: "resident".into(),
        is_active: true,
        must_change_password: false,
        password_hash: None,
        totp_secret: None,
        mfa_enabled: false,
        mfa_last_step: 0,
        failed_login_attempts: 0,
        locked_until: None,
        auth_method: "oauth".into(),
        public_key: None,
        recovery_code_hash: None,
        identity_subject: Some(identity),
    };
    {
        let mut store = state.store.lock().unwrap();
        store.next_user_id = store.next_user_id.max(id + 1);
        store.users.push(user.clone());
    }
    Ok(user)
}

struct IdentityInfo {
    subject: String,
    email: String,
    name: String,
}

/// Read the provider profile. OIDC providers return `sub`, `email` and
/// `email_verified` in one call; GitHub needs `/user/emails` for a verified
/// address.
async fn fetch_identity(
    http: &reqwest::Client,
    client: &OAuthClient,
    access_token: &str,
) -> Result<IdentityInfo, &'static str> {
    if !is_secure_provider_url(&client.userinfo_url) {
        return Err("invalid_provider_url");
    }
    let response = http
        .get(&client.userinfo_url)
        .bearer_auth(access_token)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "pinpoint-oauth/1.0")
        .send()
        .await
        .map_err(|_| "userinfo_failed")?;
    if !response.status().is_success() {
        return Err("userinfo_failed");
    }
    let info: Value = response.json().await.map_err(|_| "userinfo_failed")?;
    if client.provider.name == "github" {
        return github_identity(http, client, access_token, &info).await;
    }
    let subject = info
        .get("sub")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
        .ok_or("userinfo_failed")?
        .to_string();
    let email = info
        .get("email")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .ok_or("email_missing")?
        .to_ascii_lowercase();
    if info.get("email_verified").and_then(Value::as_bool) != Some(true) {
        return Err("email_unverified");
    }
    let name = info
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Pinpoint user")
        .to_string();
    Ok(IdentityInfo {
        subject,
        email,
        name,
    })
}

async fn github_identity(
    http: &reqwest::Client,
    client: &OAuthClient,
    access_token: &str,
    info: &Value,
) -> Result<IdentityInfo, &'static str> {
    let subject = match info.get("id") {
        Some(Value::Number(id)) => id.to_string(),
        Some(Value::String(id)) if !id.is_empty() => id.clone(),
        _ => return Err("userinfo_failed"),
    };
    let login = info
        .get("login")
        .and_then(Value::as_str)
        .unwrap_or("Pinpoint user");
    let name = info
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(login)
        .to_string();
    let email_url = client.email_url.as_deref().ok_or("email_missing")?;
    if !is_secure_provider_url(email_url) {
        return Err("invalid_provider_url");
    }
    let response = http
        .get(email_url)
        .bearer_auth(access_token)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "pinpoint-oauth/1.0")
        .send()
        .await
        .map_err(|_| "userinfo_failed")?;
    if !response.status().is_success() {
        return Err("userinfo_failed");
    }
    let emails: Value = response.json().await.map_err(|_| "userinfo_failed")?;
    let list = emails.as_array().ok_or("userinfo_failed")?;
    // Only the primary address is accepted: falling back to a secondary
    // verified address would silently change the account's login email.
    let verified = list
        .iter()
        .find(|entry| {
            entry.get("primary") == Some(&Value::Bool(true))
                && entry.get("verified") == Some(&Value::Bool(true))
        })
        .ok_or("email_unverified")?;
    let email = verified
        .get("email")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .ok_or("email_missing")?
        .to_ascii_lowercase();
    Ok(IdentityInfo {
        subject,
        email,
        name,
    })
}

fn signed_in(session_id: &str, secure: bool) -> Response {
    let mut resp = Redirect::to("/").into_response();
    resp.headers_mut().append(
        header::SET_COOKIE,
        session_cookie(session_id, secure).parse().unwrap(),
    );
    resp.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_static(clear_state_cookie()),
    );
    resp
}

fn oauth_error(reason: &str) -> Response {
    let mut resp = Redirect::to(&format!("/?oauth_error={reason}")).into_response();
    resp.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_static(clear_state_cookie()),
    );
    resp
}

fn state_cookie(token: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!(
        "{STATE_COOKIE}={token}; Path={STATE_PATH}; HttpOnly; SameSite=Lax; Max-Age={STATE_MAX_AGE_SECS}{secure}"
    )
}

fn clear_state_cookie() -> &'static str {
    "pinpoint_oauth_state=; Path=/api/v1/auth/oauth; HttpOnly; SameSite=Lax; Max-Age=0"
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_cookie_is_scoped_and_httponly() {
        let plain = state_cookie("abc", false);
        assert!(plain.starts_with("pinpoint_oauth_state=abc;"));
        assert!(plain.contains("Path=/api/v1/auth/oauth;"));
        assert!(plain.contains("HttpOnly"));
        assert!(plain.contains("SameSite=Lax"));
        assert!(!plain.contains("Secure"));
        assert!(state_cookie("abc", true).contains("; Secure"));
    }

    #[test]
    fn cookie_value_reads_only_the_named_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "session_id=x; pinpoint_oauth_state=state123; other=y"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            cookie_value(&headers, STATE_COOKIE),
            Some("state123".to_string())
        );
    }
}

//! HTTP client for the Pinpoint API.
//!
//! Contract notes, verified against `backend/internal/handlers`:
//!
//! * Session cookie auth (`session_id`) plus the double-submit CSRF header
//!   (`X-CSRF-Token`) are required for every unsafe request (POST/PUT/PATCH/
//!   DELETE) once a session exists; GETs authenticate with the cookie alone.
//! * `POST /api/v1/auth/login` and `/register` answer with
//!   `{access_token, csrf_token, user}` and set both cookies.
//! * Errors use the `{error: {code, message, fields}}` envelope.
//! * Plain HTTP is accepted only for loopback development URLs; anything
//!   else must be HTTPS (fail closed â€” see ADR-0006).

use crate::models::{
    AdminAuditEntry, CheckIn, DiagnosticReport, EmergencyAlert, FriendLocation, Friendship,
    LocationShare, LoginResponse, SessionInfo, User,
};
use crate::osm::{MapCapabilities, Route, SearchPlace};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

/// Validates the backend URL before any request can be issued.
///
/// HTTP is allowed only for loopback hosts (`localhost`, `127.0.0.1`,
/// `::1`) so development keeps working while releases are forced onto TLS.
/// Internal container deployments may opt in explicitly with
/// `PINPOINT_ALLOW_PLAINTEXT_HOSTS=gateway,server` (comma-separated exact
/// hostnames); there is no wildcard and HTTPS remains the default.
pub fn validate_backend_url(raw: &str) -> anyhow::Result<Url> {
    let allowed = std::env::var("PINPOINT_ALLOW_PLAINTEXT_HOSTS")
        .unwrap_or_default()
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect::<Vec<_>>();
    validate_backend_url_with_allowlist(raw, &allowed)
}

fn validate_backend_url_with_allowlist(raw: &str, allowed: &[String]) -> anyhow::Result<Url> {
    let url = Url::parse(raw.trim()).map_err(|e| anyhow::anyhow!("invalid backend URL: {e}"))?;
    match url.scheme() {
        "https" => Ok(url),
        "http" => {
            let host = match url.host() {
                Some(url::Host::Domain(d)) => d.to_ascii_lowercase(),
                Some(url::Host::Ipv4(ip)) => ip.to_string(),
                Some(url::Host::Ipv6(ip)) => ip.to_string(),
                None => String::new(),
            };
            let loopback = match url.host() {
                Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                None => false,
            };
            // Android emulator host loopback. Debug-only: release APKs must
            // use HTTPS (the manifest also disables cleartext in release).
            let android_emulator =
                cfg!(all(target_os = "android", debug_assertions)) && host == "10.0.2.2";
            if loopback || android_emulator || allowed.iter().any(|h| h == &host) {
                Ok(url)
            } else {
                Err(anyhow::anyhow!(
                    "refusing plain HTTP to non-loopback backend \"{}\": use HTTPS outside local development",
                    url.as_str()
                ))
            }
        }
        other => Err(anyhow::anyhow!(
            "unsupported backend URL scheme \"{}\" (expected http or https)",
            other
        )),
    }
}

#[derive(Debug, Clone)]
struct Session {
    /// `name=value` pairs harvested from `Set-Cookie` response headers.
    cookies: Vec<String>,
    /// Double-submit token from the login body or `X-CSRF-Token` header.
    csrf_token: Option<String>,
}

/// Serialisable session material persisted in the OS keychain. It contains
/// only the opaque cookie and CSRF token — never a password or email.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub cookies: Vec<String>,
    #[serde(default)]
    pub csrf_token: Option<String>,
}

impl SessionSnapshot {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(raw: &str) -> anyhow::Result<Self> {
        let snapshot: Self = serde_json::from_str(raw)?;
        if snapshot.cookies.is_empty() {
            anyhow::bail!("session snapshot has no cookies");
        }
        Ok(snapshot)
    }
}

pub struct ApiClient {
    backend_url: String,
    http: reqwest::Client,
    session: Arc<Mutex<Option<Session>>>,
}

impl ApiClient {
    /// Builds a client for `backend_url`, rejecting transports that do not
    /// meet the minimum transport policy.
    pub fn new(backend_url: impl Into<String>) -> anyhow::Result<Self> {
        let raw = backend_url.into();
        let url = validate_backend_url(&raw)?;
        // Normalize to the parsed form (no trailing slash surprises).
        let backend_url = url.as_str().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            backend_url,
            http,
            session: Arc::new(Mutex::new(None)),
        })
    }

    pub fn backend_url(&self) -> &str {
        &self.backend_url
    }

    /// True while a session cookie (and therefore an authenticated client
    /// state) is held locally. Cleared only after a confirmed logout or a
    /// password change that revokes server-side sessions.
    pub fn session_active(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    fn store_session(&self, session: Option<Session>) {
        *self.session.lock().unwrap() = session;
    }

    fn session_snapshot(&self) -> Option<Session> {
        self.session.lock().unwrap().clone()
    }

    /// Export opaque session material for keychain persistence.
    pub fn export_session(&self) -> Option<String> {
        let session = self.session_snapshot()?;
        Some(
            SessionSnapshot {
                cookies: session.cookies,
                csrf_token: session.csrf_token,
            }
            .to_json(),
        )
    }

    /// Import a previously persisted session snapshot. Fails closed when the
    /// blob is malformed or contains no cookies.
    pub fn import_session(&self, raw: &str) -> anyhow::Result<()> {
        let snapshot = SessionSnapshot::from_json(raw)?;
        self.store_session(Some(Session {
            cookies: snapshot.cookies,
            csrf_token: snapshot.csrf_token,
        }));
        Ok(())
    }

    /// Issues a request, attaching the session cookie on every request and
    /// the CSRF header on unsafe methods. The session lock is never held
    /// across an `.await`.
    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> anyhow::Result<reqwest::Response> {
        let url = format!("{}{}", self.backend_url, path);
        let mut request = self.http.request(method.clone(), &url);

        if let Some(session) = self.session_snapshot() {
            if !session.cookies.is_empty() {
                request = request.header("Cookie", session.cookies.join("; "));
            }
            let unsafe_method = matches!(
                method,
                reqwest::Method::POST
                    | reqwest::Method::PUT
                    | reqwest::Method::PATCH
                    | reqwest::Method::DELETE
            );
            if unsafe_method {
                if let Some(csrf) = session.csrf_token {
                    request = request.header("X-CSRF-Token", csrf);
                }
            }
        }

        if let Some(b) = body {
            request = request.json(&b);
        }

        Ok(request.send().await?)
    }

    /// Converts a non-success response into a readable error built from the
    /// API's error envelope (never leaks internals beyond code+message).
    async fn check(
        response: reqwest::Response,
        fallback: &str,
    ) -> anyhow::Result<reqwest::Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let text = response.text().await.unwrap_or_default();
        Err(anyhow::anyhow!(format_api_error(status, &text, fallback)))
    }

    /// Reads session material from a successful login/register response.
    fn capture_session(
        &self,
        headers: &reqwest::header::HeaderMap,
        body: &LoginResponse,
    ) -> anyhow::Result<()> {
        let cookies: Vec<String> = headers
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|s| s.split(';').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();

        // Fail closed: a 200 without a session cookie means the local state
        // would silently be unauthenticated on the next request.
        if cookies.is_empty() {
            return Err(anyhow::anyhow!(
                "server accepted the credentials but issued no session cookie"
            ));
        }

        let header_csrf = headers
            .get("X-CSRF-Token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let csrf_token = body.csrf_token.clone().or(header_csrf);

        self.store_session(Some(Session {
            cookies,
            csrf_token,
        }));
        Ok(())
    }

    pub async fn login(&self, email: &str, password: &str) -> anyhow::Result<User> {
        self.login_with_totp(email, password, None).await
    }

    /// Login with an optional TOTP code. MFA-enabled accounts need the
    /// 6-digit code in the request body; non-MFA accounts ignore it.
    pub async fn login_with_totp(
        &self,
        email: &str,
        password: &str,
        totp: Option<&str>,
    ) -> anyhow::Result<User> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/auth/login",
                Some(login_body(email, password, totp)),
            )
            .await?;
        let response = Self::check(response, "Login failed").await?;

        let headers = response.headers().clone();
        let body: LoginResponse = response
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("Login failed: unexpected response body"))?;
        self.capture_session(&headers, &body)?;
        Ok(body.user)
    }

    pub async fn register(
        &self,
        email: &str,
        password: &str,
        full_name: &str,
    ) -> anyhow::Result<User> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/auth/register",
                Some(serde_json::json!({
                    "email": email,
                    "password": password,
                    "full_name": full_name
                })),
            )
            .await?;
        let response = Self::check(response, "Registration failed").await?;

        let headers = response.headers().clone();
        let body: LoginResponse = response
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("Registration failed: unexpected response body"))?;
        self.capture_session(&headers, &body)?;
        Ok(body.user)
    }

    /// Confirms logout with the server before dropping local session state;
    /// a failed revocation keeps the session so the client never claims a
    /// logout that did not happen.
    pub async fn logout(&self) -> anyhow::Result<()> {
        let response = self
            .request(reqwest::Method::POST, "/api/v1/auth/logout", None)
            .await?;
        let response = Self::check(response, "Logout failed").await?;
        let _ = response;
        self.store_session(None);
        Ok(())
    }

    /// `GET /api/v1/auth/me` — current user for session resume.
    pub async fn me(&self) -> anyhow::Result<User> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/auth/me", None)
            .await?;
        let response = Self::check(response, "Session expired").await?;
        Ok(response.json().await?)
    }

    /// `POST /api/v1/auth/anonymous` — device-bound account plus recovery code.
    pub async fn anonymous(&self, device_name: &str) -> anyhow::Result<(User, String)> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/auth/anonymous",
                Some(serde_json::json!({ "device_name": device_name })),
            )
            .await?;
        let response = Self::check(response, "Anonymous sign-in failed").await?;
        let headers = response.headers().clone();
        let body: AnonymousResponse = response.json().await?;
        let login = LoginResponse {
            access_token: String::new(),
            csrf_token: body.csrf_token.clone(),
            user: body.user.clone(),
        };
        self.capture_session(&headers, &login)?;
        Ok((body.user, body.recovery_code))
    }

    /// `POST /api/v1/auth/anonymous/recover` — rotate and restore via code.
    pub async fn recover_account(&self, recovery_code: &str) -> anyhow::Result<(User, String)> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/auth/anonymous/recover",
                Some(serde_json::json!({ "recovery_code": recovery_code })),
            )
            .await?;
        let response = Self::check(response, "Recovery failed").await?;
        let headers = response.headers().clone();
        let body: AnonymousResponse = response.json().await?;
        let login = LoginResponse {
            access_token: String::new(),
            csrf_token: body.csrf_token.clone(),
            user: body.user.clone(),
        };
        self.capture_session(&headers, &login)?;
        Ok((body.user, body.recovery_code))
    }

    /// `GET /api/v1/auth/sessions` — active sessions for the current user.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/auth/sessions", None)
            .await?;
        let response = Self::check(response, "Failed to load sessions").await?;
        let body: SessionsResponse = response.json().await?;
        Ok(body.sessions)
    }

    /// `DELETE /api/v1/auth/sessions/:id` — revoke a session by id prefix.
    pub async fn revoke_session(&self, id: &str) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("/api/v1/auth/sessions/{}", urlencoding(id)),
                None,
            )
            .await?;
        Self::check(response, "Failed to revoke session").await?;
        Ok(())
    }

    /// `POST /api/v1/auth/logout-all` — revoke every session and clear local.
    pub async fn logout_all(&self) -> anyhow::Result<()> {
        let response = self
            .request(reqwest::Method::POST, "/api/v1/auth/logout-all", None)
            .await?;
        let result = Self::check(response, "Sign out everywhere failed").await;
        self.store_session(None);
        result.map(|_| ())
    }

    /// `POST /api/v1/diagnostics/report` — PII-free user-initiated report.
    pub async fn submit_diagnostic(
        &self,
        category: &str,
        message: &str,
        platform: &str,
    ) -> anyhow::Result<i64> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/diagnostics/report",
                Some(serde_json::json!({
                    "category": category,
                    "message": message,
                    "platform": platform,
                })),
            )
            .await?;
        let response = Self::check(response, "Diagnostic report failed").await?;
        let body: serde_json::Value = response.json().await?;
        Ok(body["id"].as_i64().unwrap_or(0))
    }

    /// MFA: start enrolment; returns (secret, otpauth_uri).
    pub async fn mfa_enroll(&self) -> anyhow::Result<(String, String)> {
        let response = self
            .request(reqwest::Method::POST, "/api/v1/auth/mfa/enroll", None)
            .await?;
        let response = Self::check(response, "MFA enrolment failed").await?;
        let body: serde_json::Value = response.json().await?;
        Ok((
            body["secret"].as_str().unwrap_or("").to_string(),
            body["otpauth_uri"].as_str().unwrap_or("").to_string(),
        ))
    }

    /// MFA: verify a 6-digit code; enables MFA server-side.
    pub async fn mfa_verify(&self, code: &str) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/auth/mfa/verify",
                Some(serde_json::json!({ "code": code })),
            )
            .await?;
        Self::check(response, "MFA verification failed").await?;
        Ok(())
    }

    pub async fn mfa_status(&self) -> anyhow::Result<bool> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/auth/mfa/status", None)
            .await?;
        let response = Self::check(response, "MFA status failed").await?;
        let body: serde_json::Value = response.json().await?;
        Ok(body["mfa_enabled"].as_bool().unwrap_or(false))
    }

    pub async fn mfa_disable(&self) -> anyhow::Result<()> {
        let response = self
            .request(reqwest::Method::DELETE, "/api/v1/auth/mfa", None)
            .await?;
        Self::check(response, "MFA disable failed").await?;
        Ok(())
    }

    /// `POST /api/v1/auth/password/change` (revokes all sessions server-side,
    /// so local state is dropped) or `/password/force-change` when the server
    /// forced the rotation (`current_password` omitted).
    pub async fn change_password(
        &self,
        current_password: Option<&str>,
        new_password: &str,
        confirm_password: &str,
    ) -> anyhow::Result<bool> {
        let (path, body) = match current_password {
            Some(current) => (
                "/api/v1/auth/password/change",
                serde_json::json!({
                    "current_password": current,
                    "new_password": new_password,
                    "confirm_password": confirm_password
                }),
            ),
            None => (
                "/api/v1/auth/password/force-change",
                serde_json::json!({
                    "new_password": new_password,
                    "confirm_password": confirm_password
                }),
            ),
        };

        let response = self
            .request(reqwest::Method::POST, path, Some(body))
            .await?;
        let response = Self::check(response, "Password change failed").await?;
        let _ = response;

        // The regular change endpoint revokes every session for the user;
        // drop local state so the UI returns to the login screen. The
        // forced change keeps the current session alive.
        let revoked = current_password.is_some();
        if revoked {
            self.store_session(None);
        }
        Ok(revoked)
    }

    pub async fn get_friendships(&self) -> anyhow::Result<Vec<Friendship>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/friendships/", None)
            .await?;
        let response = Self::check(response, "Failed to fetch friendships").await?;
        Ok(response.json().await?)
    }

    pub async fn send_friend_request(&self, identifier: &str) -> anyhow::Result<()> {
        // Username is the public path; the exact email is a legacy fallback
        // for older accounts. Sending both keeps one call site and lets the
        // server resolve either without guessing the input type.
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/friendships/",
                Some(serde_json::json!({
                    "receiver_username": identifier,
                    "receiver_email": identifier,
                })),
            )
            .await?;
        Self::check(response, "Failed to send friend request").await?;
        Ok(())
    }

    pub async fn respond_to_friendship(&self, id: i64, action: &str) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/api/v1/friendships/{id}/respond"),
                Some(serde_json::json!({ "action": action })),
            )
            .await?;
        Self::check(response, "Failed to respond to friendship").await?;
        Ok(())
    }

    pub async fn get_location_shares(&self) -> anyhow::Result<Vec<LocationShare>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/locations/shares", None)
            .await?;
        let response = Self::check(response, "Failed to fetch location shares").await?;
        Ok(response.json().await?)
    }

    pub async fn create_location_share(
        &self,
        friendship_id: i64,
        direction: &str,
        precision_level: &str,
    ) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/locations/shares",
                Some(serde_json::json!({
                    "friendship_id": friendship_id,
                    "direction": direction,
                    "precision_level": precision_level
                })),
            )
            .await?;
        Self::check(response, "Failed to create location share").await?;
        Ok(())
    }

    pub async fn update_location_share(
        &self,
        share_id: i64,
        is_active: Option<bool>,
        precision: Option<String>,
        duration_hours: Option<i64>,
    ) -> anyhow::Result<()> {
        let body = update_share_body(is_active, precision.as_deref(), duration_hours);
        let response = self
            .request(
                reqwest::Method::PATCH,
                &format!("/api/v1/locations/shares/{share_id}"),
                Some(body),
            )
            .await?;
        Self::check(response, "Failed to update location share").await?;
        Ok(())
    }

    pub async fn get_friend_locations(&self) -> anyhow::Result<Vec<FriendLocation>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/locations/friends", None)
            .await?;
        let response = Self::check(response, "Failed to fetch friend locations").await?;
        Ok(response.json().await?)
    }

    pub async fn publish_location(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/locations/me",
                Some(serde_json::json!({
                    "latitude": latitude,
                    "longitude": longitude,
                    "accuracy": accuracy
                })),
            )
            .await?;
        Self::check(response, "Failed to publish location").await?;
        Ok(())
    }

    pub async fn get_my_location(&self) -> anyhow::Result<FriendLocation> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/locations/me", None)
            .await?;
        let response = Self::check(response, "Failed to fetch my location").await?;
        Ok(response.json().await?)
    }

    pub async fn delete_share(&self, share_id: i64) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("/api/v1/locations/shares/{share_id}"),
                None,
            )
            .await?;
        Self::check(response, "Failed to delete share").await?;
        Ok(())
    }

    pub async fn create_share_with_expiry(
        &self,
        friendship_id: i64,
        direction: &str,
        precision_level: &str,
        duration_hours: Option<i64>,
        label: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut body = serde_json::json!({
            "friendship_id": friendship_id,
            "direction": direction,
            "precision_level": precision_level,
        });
        if let Some(h) = duration_hours {
            body["duration_hours"] = serde_json::Value::from(h);
        }
        if let Some(l) = label {
            body["label"] = serde_json::Value::String(l.to_string());
        }
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/locations/shares",
                Some(body),
            )
            .await?;
        Self::check(response, "Failed to create location share").await?;
        Ok(())
    }

    pub async fn delete_friendship(&self, id: i64) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("/api/v1/friendships/{id}"),
                None,
            )
            .await?;
        Self::check(response, "Failed to remove friendship").await?;
        Ok(())
    }

    pub async fn list_users(&self) -> anyhow::Result<Vec<User>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/users/", None)
            .await?;
        let response = Self::check(response, "Failed to fetch users").await?;
        Ok(response.json().await?)
    }

    pub async fn search_users(&self, q: &str) -> anyhow::Result<Vec<User>> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/api/v1/users/search?q={}", urlencoding(q)),
                None,
            )
            .await?;
        let response = Self::check(response, "Failed to search users").await?;
        Ok(response.json().await?)
    }

    pub async fn update_user(&self, id: i64, body: serde_json::Value) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::PATCH,
                &format!("/api/v1/users/{id}"),
                Some(body),
            )
            .await?;
        Self::check(response, "Failed to update user").await?;
        Ok(())
    }

    pub async fn delete_user(&self, id: i64) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("/api/v1/users/{id}"),
                None,
            )
            .await?;
        Self::check(response, "Failed to delete user").await?;
        Ok(())
    }

    pub async fn admin_overview(&self) -> anyhow::Result<serde_json::Value> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/admin/overview", None)
            .await?;
        let response = Self::check(response, "Failed to load admin overview").await?;
        Ok(response.json().await?)
    }

    pub async fn admin_users(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/admin/users", None)
            .await?;
        let response = Self::check(response, "Failed to load admin users").await?;
        let body: AdminUsersResponse = response.json().await?;
        Ok(body.users)
    }

    pub async fn admin_update_user(&self, id: i64, body: serde_json::Value) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::PATCH,
                &format!("/api/v1/admin/users/{id}"),
                Some(body),
            )
            .await?;
        Self::check(response, "Failed to update admin user").await?;
        Ok(())
    }

    pub async fn admin_delete_user(&self, id: i64) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::DELETE,
                &format!("/api/v1/admin/users/{id}"),
                None,
            )
            .await?;
        Self::check(response, "Failed to delete admin user").await?;
        Ok(())
    }

    pub async fn admin_diagnostics(
        &self,
        status: Option<&str>,
    ) -> anyhow::Result<(Vec<DiagnosticReport>, i64, i64)> {
        let path = match status.filter(|s| !s.is_empty()) {
            Some(status) => format!("/api/v1/admin/diagnostics?status={}", urlencoding(status)),
            None => "/api/v1/admin/diagnostics".to_string(),
        };
        let response = self.request(reqwest::Method::GET, &path, None).await?;
        let response = Self::check(response, "Failed to load diagnostics").await?;
        let body: AdminDiagnosticsResponse = response.json().await?;
        Ok((body.reports, body.total, body.open))
    }

    pub async fn admin_update_report(&self, id: i64, status: &str) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::PATCH,
                &format!("/api/v1/admin/diagnostics/{id}"),
                Some(serde_json::json!({ "status": status })),
            )
            .await?;
        Self::check(response, "Failed to triage diagnostic").await?;
        Ok(())
    }

    pub async fn admin_audit(&self) -> anyhow::Result<Vec<AdminAuditEntry>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/admin/audit", None)
            .await?;
        let response = Self::check(response, "Failed to load audit log").await?;
        let body: AdminAuditResponse = response.json().await?;
        Ok(body.entries)
    }

    pub async fn create_check_in(
        &self,
        lat: f64,
        lon: f64,
        note: &str,
        kind: &str,
    ) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/locations/check-ins",
                Some(serde_json::json!({
                    "latitude": lat, "longitude": lon, "note": note, "kind": kind
                })),
            )
            .await?;
        Self::check(response, "Failed to create check-in").await?;
        Ok(())
    }

    pub async fn list_check_ins(&self) -> anyhow::Result<Vec<CheckIn>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/locations/check-ins", None)
            .await?;
        let response = Self::check(response, "Failed to fetch check-ins").await?;
        Ok(response.json().await?)
    }

    pub async fn trigger_emergency(&self, lat: f64, lon: f64) -> anyhow::Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                "/api/v1/locations/emergency",
                Some(serde_json::json!({ "latitude": lat, "longitude": lon })),
            )
            .await?;
        Self::check(response, "Failed to trigger emergency").await?;
        Ok(())
    }

    pub async fn list_emergencies(&self) -> anyhow::Result<Vec<EmergencyAlert>> {
        let response = self
            .request(reqwest::Method::GET, "/api/v1/locations/emergency", None)
            .await?;
        let response = Self::check(response, "Failed to fetch emergencies").await?;
        Ok(response.json().await?)
    }

    /// `GET /map/capabilities` — which routing modes/avoid options the
    /// server-side OSRM deployment supports.
    pub async fn map_capabilities(&self) -> anyhow::Result<MapCapabilities> {
        let response = self
            .request(reqwest::Method::GET, "/map/capabilities", None)
            .await?;
        let response = Self::check(response, "Failed to load map capabilities").await?;
        Ok(response.json().await?)
    }

    /// `GET /map/search?q=&lat=&lon=&limit=` — backend-proxied Nominatim.
    pub async fn map_search(
        &self,
        q: &str,
        lat: Option<f64>,
        lon: Option<f64>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchPlace>> {
        if q.trim().chars().count() < 3 {
            anyhow::bail!("Search needs at least 3 characters.");
        }
        if lat.is_some() != lon.is_some() {
            anyhow::bail!("lat and lon must be supplied together.");
        }
        let mut path = format!(
            "/map/search?q={}&limit={}",
            urlencoding(q.trim()),
            limit.clamp(1, 10)
        );
        if let (Some(lat), Some(lon)) = (lat, lon) {
            path.push_str(&format!("&lat={lat:.6}&lon={lon:.6}"));
        }
        let response = self.request(reqwest::Method::GET, &path, None).await?;
        let response = Self::check(response, "Search failed").await?;
        let body: SearchResponse = response.json().await?;
        Ok(body.results)
    }

    /// `GET /map/reverse?lat=&lon=` — backend-proxied reverse geocoding.
    pub async fn map_reverse(&self, lat: f64, lon: f64) -> anyhow::Result<SearchPlace> {
        if !lat.is_finite() || !lon.is_finite() {
            anyhow::bail!("Coordinates must be finite.");
        }
        let path = format!("/map/reverse?lat={lat:.6}&lon={lon:.6}");
        let response = self.request(reqwest::Method::GET, &path, None).await?;
        let response = Self::check(response, "Reverse geocoding failed").await?;
        Ok(response.json().await?)
    }

    /// `GET /map/route` — backend-proxied OSRM with stops and exclusions.
    pub async fn map_route(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        mode: &str,
        via: &[(f64, f64)],
        avoid: &[String],
        alternatives: bool,
    ) -> anyhow::Result<Vec<Route>> {
        if via.len() > 6 {
            anyhow::bail!("A route can have at most 6 stops.");
        }
        if avoid.len() > 3 {
            anyhow::bail!("At most 3 avoid classes are supported.");
        }
        let path = map_route_path(from, to, mode, via, avoid, alternatives);
        let response = self.request(reqwest::Method::GET, &path, None).await?;
        let response = Self::check(response, "Routing failed").await?;
        let body: RouteResponse = response.json().await?;
        Ok(body.routes)
    }
}

#[derive(serde::Deserialize)]
struct SearchResponse {
    results: Vec<SearchPlace>,
}

#[derive(serde::Deserialize)]
struct RouteResponse {
    routes: Vec<Route>,
}

#[derive(serde::Deserialize)]
struct SessionsResponse {
    sessions: Vec<SessionInfo>,
}

#[derive(serde::Deserialize)]
struct AnonymousResponse {
    user: User,
    recovery_code: String,
    #[serde(default)]
    csrf_token: Option<String>,
}

#[derive(serde::Deserialize)]
struct AdminUsersResponse {
    users: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct AdminDiagnosticsResponse {
    #[serde(default)]
    reports: Vec<DiagnosticReport>,
    #[serde(default)]
    total: i64,
    #[serde(default)]
    open: i64,
}

#[derive(serde::Deserialize)]
struct AdminAuditResponse {
    #[serde(default)]
    entries: Vec<AdminAuditEntry>,
}

fn update_share_body(
    is_active: Option<bool>,
    precision: Option<&str>,
    duration_hours: Option<i64>,
) -> serde_json::Value {
    let mut body = serde_json::json!({});
    if let Some(active) = is_active {
        body["is_active"] = serde_json::Value::Bool(active);
    }
    if let Some(p) = precision {
        body["precision_level"] = serde_json::Value::String(p.to_string());
    }
    if let Some(hours) = duration_hours {
        body["duration_hours"] = serde_json::Value::Number(hours.into());
    }
    body
}

fn login_body(email: &str, password: &str, totp: Option<&str>) -> serde_json::Value {
    match totp.map(str::trim).filter(|code| !code.is_empty()) {
        Some(code) => {
            serde_json::json!({ "email": email, "password": password, "totp": code })
        }
        None => serde_json::json!({ "email": email, "password": password }),
    }
}

/// Build the `/map/route` query string. Kept pure so the encoding is
/// testable without a network call; commas/semicolons are legal in a query.
fn map_route_path(
    from: (f64, f64),
    to: (f64, f64),
    mode: &str,
    via: &[(f64, f64)],
    avoid: &[String],
    alternatives: bool,
) -> String {
    let mut path = format!(
        "/map/route?from_lat={:.6}&from_lon={:.6}&to_lat={:.6}&to_lon={:.6}&mode={}&alternatives={}",
        from.0,
        from.1,
        to.0,
        to.1,
        urlencoding(mode),
        alternatives
    );
    if !via.is_empty() {
        let encoded = via
            .iter()
            .map(|(lat, lon)| format!("{lat:.6},{lon:.6}"))
            .collect::<Vec<_>>()
            .join(";");
        path.push_str(&format!("&via={encoded}"));
    }
    if !avoid.is_empty() {
        path.push_str(&format!("&avoid={}", avoid.join(",")));
    }
    path
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Renders the API's `{error: {code, message, fields}}` envelope as a
/// single readable line; falls back to a truncated body snippet so the UI
/// never shows a full HTML/JSON dump.
fn format_api_error(status: reqwest::StatusCode, body: &str, fallback: &str) -> String {
    #[derive(serde::Deserialize)]
    struct ErrorEnvelope {
        error: ErrorDetail,
    }
    #[derive(serde::Deserialize)]
    struct ErrorDetail {
        #[serde(default)]
        code: Option<String>,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        fields: Option<serde_json::Map<String, serde_json::Value>>,
    }

    if let Ok(envelope) = serde_json::from_str::<ErrorEnvelope>(body) {
        let code = envelope.error.code.unwrap_or_default();
        let message = envelope
            .error
            .message
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| fallback.to_string());
        let mut line = if code.is_empty() {
            message
        } else {
            format!("{code}: {message}")
        };
        if let Some(fields) = envelope.error.fields {
            let mut parts: Vec<String> = Vec::new();
            for (field, errors) in fields {
                match errors {
                    serde_json::Value::Array(list) => {
                        let msgs: Vec<String> = list
                            .iter()
                            .filter_map(|e| e.as_str().map(str::to_string))
                            .collect();
                        if !msgs.is_empty() {
                            parts.push(format!("{field}: {}", msgs.join(", ")));
                        }
                    }
                    serde_json::Value::String(s) => parts.push(format!("{field}: {s}")),
                    _ => {}
                }
            }
            if !parts.is_empty() {
                line = format!("{line} ({})", parts.join("; "));
            }
        }
        return line;
    }

    let snippet: String = body.chars().take(160).collect();
    if snippet.trim().is_empty() {
        format!("{} {fallback}", status.as_u16())
    } else {
        format!("{status}: {snippet}")
    }
}

// ── Backend trait adapter ──────────────────────────────────────────────────
// Implements the modular backend abstraction so app/UI code can target
// `backend::Backend` while this file remains the canonical REST v1 adapter.

impl crate::backend::Backend for ApiClient {
    fn backend_url(&self) -> &str {
        ApiClient::backend_url(self)
    }
    fn session_active(&self) -> bool {
        ApiClient::session_active(self)
    }
    fn export_session(&self) -> Option<String> {
        ApiClient::export_session(self)
    }
    fn import_session(&self, raw: &str) -> anyhow::Result<()> {
        ApiClient::import_session(self, raw)
    }
    fn login<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> crate::backend::BackendFuture<'a, crate::models::User> {
        Box::pin(async move { ApiClient::login(self, email, password).await })
    }
    fn login_with_totp<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        totp: Option<&'a str>,
    ) -> crate::backend::BackendFuture<'a, crate::models::User> {
        Box::pin(async move { ApiClient::login_with_totp(self, email, password, totp).await })
    }
    fn register<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        full_name: &'a str,
    ) -> crate::backend::BackendFuture<'a, crate::models::User> {
        Box::pin(async move { ApiClient::register(self, email, password, full_name).await })
    }
    fn logout<'a>(&'a self) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::logout(self).await })
    }
    fn me(&self) -> crate::backend::BackendFuture<'_, crate::models::User> {
        Box::pin(async move { ApiClient::me(self).await })
    }
    fn anonymous<'a>(
        &'a self,
        device_name: &'a str,
    ) -> crate::backend::BackendFuture<'a, (crate::models::User, String)> {
        Box::pin(async move { ApiClient::anonymous(self, device_name).await })
    }
    fn recover_account<'a>(
        &'a self,
        code: &'a str,
    ) -> crate::backend::BackendFuture<'a, (crate::models::User, String)> {
        Box::pin(async move { ApiClient::recover_account(self, code).await })
    }
    fn list_sessions(&self) -> crate::backend::BackendFuture<'_, Vec<crate::models::SessionInfo>> {
        Box::pin(async move { ApiClient::list_sessions(self).await })
    }
    fn revoke_session<'a>(&'a self, id: &'a str) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::revoke_session(self, id).await })
    }
    fn logout_all(&self) -> crate::backend::BackendFuture<'_, ()> {
        Box::pin(async move { ApiClient::logout_all(self).await })
    }
    fn submit_diagnostic<'a>(
        &'a self,
        category: &'a str,
        message: &'a str,
        platform: &'a str,
    ) -> crate::backend::BackendFuture<'a, i64> {
        Box::pin(
            async move { ApiClient::submit_diagnostic(self, category, message, platform).await },
        )
    }
    fn mfa_enroll<'a>(&'a self) -> crate::backend::BackendFuture<'a, (String, String)> {
        Box::pin(async move { ApiClient::mfa_enroll(self).await })
    }
    fn mfa_verify<'a>(&'a self, code: &'a str) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::mfa_verify(self, code).await })
    }
    fn mfa_status<'a>(&'a self) -> crate::backend::BackendFuture<'a, bool> {
        Box::pin(async move { ApiClient::mfa_status(self).await })
    }
    fn mfa_disable<'a>(&'a self) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::mfa_disable(self).await })
    }
    fn change_password<'a>(
        &'a self,
        current_password: Option<&'a str>,
        new_password: &'a str,
        confirm_password: &'a str,
    ) -> crate::backend::BackendFuture<'a, bool> {
        Box::pin(async move {
            ApiClient::change_password(self, current_password, new_password, confirm_password).await
        })
    }
    fn get_friendships<'a>(
        &'a self,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::models::Friendship>> {
        Box::pin(async move { ApiClient::get_friendships(self).await })
    }
    fn send_friend_request<'a>(&'a self, email: &'a str) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::send_friend_request(self, email).await })
    }
    fn respond_to_friendship<'a>(
        &'a self,
        id: i64,
        action: &'a str,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::respond_to_friendship(self, id, action).await })
    }
    fn delete_friendship<'a>(&'a self, id: i64) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::delete_friendship(self, id).await })
    }
    fn get_location_shares<'a>(
        &'a self,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::models::LocationShare>> {
        Box::pin(async move { ApiClient::get_location_shares(self).await })
    }
    fn create_location_share<'a>(
        &'a self,
        friendship_id: i64,
        direction: &'a str,
        precision_level: &'a str,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move {
            ApiClient::create_location_share(self, friendship_id, direction, precision_level).await
        })
    }
    fn create_share_with_expiry<'a>(
        &'a self,
        friendship_id: i64,
        direction: &'a str,
        precision_level: &'a str,
        duration_hours: Option<i64>,
        label: Option<&'a str>,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move {
            ApiClient::create_share_with_expiry(
                self,
                friendship_id,
                direction,
                precision_level,
                duration_hours,
                label,
            )
            .await
        })
    }
    fn update_location_share<'a>(
        &'a self,
        share_id: i64,
        is_active: Option<bool>,
        precision: Option<String>,
        duration_hours: Option<i64>,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move {
            ApiClient::update_location_share(self, share_id, is_active, precision, duration_hours)
                .await
        })
    }
    fn delete_share<'a>(&'a self, share_id: i64) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::delete_share(self, share_id).await })
    }
    fn get_friend_locations<'a>(
        &'a self,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::models::FriendLocation>> {
        Box::pin(async move { ApiClient::get_friend_locations(self).await })
    }
    fn publish_location<'a>(
        &'a self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(
            async move { ApiClient::publish_location(self, latitude, longitude, accuracy).await },
        )
    }
    fn get_my_location<'a>(
        &'a self,
    ) -> crate::backend::BackendFuture<'a, crate::models::FriendLocation> {
        Box::pin(async move { ApiClient::get_my_location(self).await })
    }
    fn map_capabilities(&self) -> crate::backend::BackendFuture<'_, crate::osm::MapCapabilities> {
        Box::pin(async move { ApiClient::map_capabilities(self).await })
    }
    fn map_search<'a>(
        &'a self,
        q: &'a str,
        lat: Option<f64>,
        lon: Option<f64>,
        limit: usize,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::osm::SearchPlace>> {
        Box::pin(async move { ApiClient::map_search(self, q, lat, lon, limit).await })
    }
    fn map_reverse(
        &self,
        lat: f64,
        lon: f64,
    ) -> crate::backend::BackendFuture<'_, crate::osm::SearchPlace> {
        Box::pin(async move { ApiClient::map_reverse(self, lat, lon).await })
    }
    fn map_route<'a>(
        &'a self,
        from: (f64, f64),
        to: (f64, f64),
        mode: &'a str,
        via: &'a [(f64, f64)],
        avoid: &'a [String],
        alternatives: bool,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::osm::Route>> {
        Box::pin(async move {
            ApiClient::map_route(self, from, to, mode, via, avoid, alternatives).await
        })
    }
    fn list_users<'a>(&'a self) -> crate::backend::BackendFuture<'a, Vec<crate::models::User>> {
        Box::pin(async move { ApiClient::list_users(self).await })
    }
    fn search_users<'a>(
        &'a self,
        q: &'a str,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::models::User>> {
        Box::pin(async move { ApiClient::search_users(self, q).await })
    }
    fn update_user<'a>(
        &'a self,
        id: i64,
        body: serde_json::Value,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::update_user(self, id, body).await })
    }
    fn delete_user<'a>(&'a self, id: i64) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::delete_user(self, id).await })
    }
    fn admin_overview(&self) -> crate::backend::BackendFuture<'_, serde_json::Value> {
        Box::pin(async move { ApiClient::admin_overview(self).await })
    }
    fn admin_users(&self) -> crate::backend::BackendFuture<'_, Vec<serde_json::Value>> {
        Box::pin(async move { ApiClient::admin_users(self).await })
    }
    fn admin_update_user<'a>(
        &'a self,
        id: i64,
        body: serde_json::Value,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::admin_update_user(self, id, body).await })
    }
    fn admin_delete_user<'a>(&'a self, id: i64) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::admin_delete_user(self, id).await })
    }
    fn admin_diagnostics<'a>(
        &'a self,
        status: Option<&'a str>,
    ) -> crate::backend::BackendFuture<'a, (Vec<crate::models::DiagnosticReport>, i64, i64)> {
        Box::pin(async move { ApiClient::admin_diagnostics(self, status).await })
    }
    fn admin_update_report<'a>(
        &'a self,
        id: i64,
        status: &'a str,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::admin_update_report(self, id, status).await })
    }
    fn admin_audit(
        &self,
    ) -> crate::backend::BackendFuture<'_, Vec<crate::models::AdminAuditEntry>> {
        Box::pin(async move { ApiClient::admin_audit(self).await })
    }
    fn create_check_in<'a>(
        &'a self,
        lat: f64,
        lon: f64,
        note: &'a str,
        kind: &'a str,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::create_check_in(self, lat, lon, note, kind).await })
    }
    fn list_check_ins<'a>(
        &'a self,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::models::CheckIn>> {
        Box::pin(async move { ApiClient::list_check_ins(self).await })
    }
    fn trigger_emergency<'a>(
        &'a self,
        lat: f64,
        lon: f64,
    ) -> crate::backend::BackendFuture<'a, ()> {
        Box::pin(async move { ApiClient::trigger_emergency(self, lat, lon).await })
    }
    fn list_emergencies<'a>(
        &'a self,
    ) -> crate::backend::BackendFuture<'a, Vec<crate::models::EmergencyAlert>> {
        Box::pin(async move { ApiClient::list_emergencies(self).await })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        login_body, map_route_path, update_share_body, validate_backend_url_with_allowlist,
        SessionSnapshot,
    };

    #[test]
    fn accepts_loopback_http_development_urls() {
        for url in [
            "http://localhost:8000",
            "http://127.0.0.1:8000",
            "http://[::1]:8000",
            "http://LOCALHOST:8000",
        ] {
            assert!(
                validate_backend_url_with_allowlist(url, &[]).is_ok(),
                "{url} should be accepted"
            );
        }
    }

    #[test]
    fn accepts_https_everywhere() {
        for url in ["https://api.example.com", "https://10.0.0.5:8443"] {
            assert!(
                validate_backend_url_with_allowlist(url, &[]).is_ok(),
                "{url} should be accepted"
            );
        }
    }

    // Internal container DNS is an explicit, exact-host opt-in — never a
    // wildcard and never the default.
    #[test]
    fn accepts_explicitly_allowed_plaintext_internal_host() {
        let allowed = vec!["gateway".to_string()];
        assert!(validate_backend_url_with_allowlist("http://gateway", &allowed).is_ok());
        assert!(validate_backend_url_with_allowlist("http://GATEWAY:8000", &allowed).is_ok());
        assert!(
            validate_backend_url_with_allowlist("http://db:5432", &allowed).is_err(),
            "only the listed host may use plaintext"
        );
    }

    // Fail closed: non-loopback plain HTTP is the exact regression this
    // policy exists to prevent (ADR-0006).
    #[test]
    fn rejects_plaintext_http_to_non_loopback_hosts() {
        for url in [
            "http://example.com",
            "http://api.example.com:8000",
            "http://192.168.1.10:8000",
            "http://10.0.2.2:8000",
        ] {
            assert!(
                validate_backend_url_with_allowlist(url, &[]).is_err(),
                "{url} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_unsupported_schemes_and_garbage() {
        for url in ["ftp://example.com", "ws://localhost:8000", "not a url", ""] {
            assert!(
                validate_backend_url_with_allowlist(url, &[]).is_err(),
                "{url} must be rejected"
            );
        }
    }

    #[test]
    fn search_place_deserializes_distance() {
        let value = serde_json::json!({
            "display_name": "Cafe",
            "short_name": "Cafe",
            "latitude": 47.0,
            "longitude": 28.8,
            "category": "cafe",
            "place_type": "cafe",
            "distance_m": 12.5
        });
        let place: crate::osm::SearchPlace = serde_json::from_value(value).unwrap();
        assert_eq!(place.distance_m, Some(12.5));
    }

    #[test]
    fn route_deserializes_points_and_steps() {
        let value = serde_json::json!({
            "distance_m": 4200.0,
            "duration_s": 540.0,
            "summary": "Main",
            "points": [[47.0, 28.8], [47.1, 28.9]],
            "steps": [{"text": "Turn right", "distance_m": 120.0, "duration_s": 18.0}],
            "legs": [{"distance_m": 4200.0, "duration_s": 540.0}]
        });
        let route: crate::osm::Route = serde_json::from_value(value).unwrap();
        assert_eq!(route.points.len(), 2);
        assert_eq!(route.steps[0].distance_m, Some(120.0));
    }

    #[test]
    fn session_snapshot_round_trips_without_identity_fields() {
        let snapshot = SessionSnapshot {
            cookies: vec!["session_id=abc".into()],
            csrf_token: Some("csrf".into()),
        };
        let json = snapshot.to_json();
        assert!(json.contains("session_id=abc"));
        assert!(!json.contains("password"));
        assert!(!json.contains("email"));
        let back = SessionSnapshot::from_json(&json).unwrap();
        assert_eq!(back, snapshot);
    }

    #[test]
    fn session_info_deserializes_current_flag() {
        let value = serde_json::json!({
            "id": "abcd1234",
            "current": true,
            "auth_method": "email",
            "mfa_pending": false,
            "created_at": "2026-11-14T00:00:00Z",
            "last_activity": "2026-11-14T00:00:00Z"
        });
        let session: crate::models::SessionInfo = serde_json::from_value(value).unwrap();
        assert!(session.current);
        assert_eq!(session.id, "abcd1234");
    }

    #[test]
    fn update_share_body_includes_duration_and_precision() {
        let body = update_share_body(Some(false), Some("city_only"), Some(24));
        assert_eq!(body["is_active"], false);
        assert_eq!(body["precision_level"], "city_only");
        assert_eq!(body["duration_hours"], 24);
        let unchanged = update_share_body(None, None, None);
        assert!(unchanged.as_object().unwrap().is_empty());
    }

    #[test]
    fn login_body_includes_totp_only_when_present() {
        let with = login_body("a@b.c", "pw", Some("123456"));
        assert_eq!(with["totp"], "123456");
        let without = login_body("a@b.c", "pw", None);
        assert!(without.get("totp").is_none());
        assert!(without.get("password").is_some());
    }

    #[test]
    fn map_route_path_encodes_via_avoid_and_mode() {
        let path = map_route_path(
            (47.0, 28.8),
            (47.1, 28.9),
            "foot",
            &[(47.05, 28.85)],
            &["toll".to_string(), "ferry".to_string()],
            true,
        );
        assert!(path.starts_with("/map/route?"));
        assert!(path.contains("from_lat=47.000000"));
        assert!(path.contains("mode=foot"));
        assert!(path.contains("via=47.050000,28.850000"));
        assert!(path.contains("avoid=toll,ferry"));
        assert!(path.contains("alternatives=true"));
        assert!(!path.contains("email"));
    }
}

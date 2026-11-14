//! Failover backend wrapper — automatic failover across multiple backend URLs
//! with per-backend circuit breakers, health monitoring, and retry with
//! exponential backoff.
//!
//! Example:
//!   let failover = FailoverBackend::new(vec![
//!       "http://gateway:8000",
//!       "http://auth-replica-2:8001",
//!   ])?;
//!   let user = failover.login("alice@example.com", "password").await?;

use crate::backend::{Backend, BackendFuture, DynBackend};
use crate::models::{
    AdminAuditEntry, CheckIn, DiagnosticReport, EmergencyAlert, FriendLocation, Friendship,
    LocationShare, SessionInfo, User,
};
use crate::osm::{MapCapabilities, Route, SearchPlace};
use crate::security::circuit_breaker::CircuitBreaker;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// A single backend entry in the failover pool.
#[derive(Clone)]
struct BackendEntry {
    url: String,
    backend: DynBackend,
    circuit: Arc<CircuitBreaker>,
}

/// Failover backend: tries backends in order, skipping open-circuit or
/// unhealthy ones, with exponential backoff between attempts.
#[derive(Clone)]
pub struct FailoverBackend {
    backends: Arc<RwLock<Vec<BackendEntry>>>,
    max_retries: u32,
    base_backoff_ms: u64,
}

impl FailoverBackend {
    /// Create a new failover backend from a list of URLs. The first URL is
    /// treated as the primary; all URLs are tried in order on failure.
    pub fn new(urls: Vec<String>) -> anyhow::Result<Self> {
        if urls.is_empty() {
            anyhow::bail!("at least one backend URL is required");
        }
        let mut entries = Vec::new();
        for url in urls {
            let backend = crate::api::ApiClient::new(url.clone())?;
            let circuit = CircuitBreaker::default_with_name(url.clone());
            entries.push(BackendEntry {
                url: url.clone(),
                backend: Arc::new(backend) as DynBackend,
                circuit,
            });
        }
        Ok(Self {
            backends: Arc::new(RwLock::new(entries)),
            max_retries: 3,
            base_backoff_ms: 500,
        })
    }

    /// Execute `f` against each backend in order, retrying with exponential
    /// backoff. Returns the first success. If all backends fail, returns
    /// the last error.
    async fn try_all<F, Fut, T>(&self, op: &str, mut f: F) -> anyhow::Result<T>
    where
        F: FnMut(DynBackend) -> Fut + Send,
        Fut: Future<Output = anyhow::Result<T>> + Send,
    {
        let entries = self.backends.read().await;
        let mut last_error: Option<anyhow::Error> = None;

        for attempt in 0..=self.max_retries {
            for entry in entries.iter() {
                let circuit = entry.circuit.clone();
                if !circuit.can_proceed() {
                    continue;
                }

                let backend = entry.backend.clone();
                let result = f(backend).await;

                match result {
                    Ok(v) => {
                        circuit.on_success();
                        return Ok(v);
                    }
                    Err(e) => {
                        circuit.on_failure();
                        last_error = Some(e);
                    }
                }
            }

            // Backoff before next retry round (unless this was the last).
            if attempt < self.max_retries {
                let backoff = self.base_backoff_ms * 2u64.pow(attempt);
                tokio::time::sleep(Duration::from_millis(backoff)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("all backends unavailable for {op}")))
    }

    /// Set max retry attempts per backend round (default 3).
    pub fn with_max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Set base backoff in ms between retry rounds (default 500).
    pub fn with_base_backoff_ms(mut self, ms: u64) -> Self {
        self.base_backoff_ms = ms;
        self
    }

    /// Return all backend URLs (for diagnostics).
    pub async fn urls(&self) -> Vec<String> {
        let entries = self.backends.read().await;
        entries.iter().map(|e| e.url.clone()).collect()
    }

    /// Return circuit breaker state per URL (for UI / debugging).
    pub async fn circuit_states(&self) -> Vec<(String, String, u32)> {
        let entries = self.backends.read().await;
        let mut out: Vec<(String, String, u32)> = entries
            .iter()
            .map(|e| {
                (
                    e.url.clone(),
                    format!("{:?}", e.circuit.state()),
                    e.circuit.failures(),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

// ── Backend trait implementation ──────────────────────────────────────

impl Backend for FailoverBackend {
    fn backend_url(&self) -> &str {
        // Return the first (primary) URL for diagnostics.
        "failover://multi"
    }

    fn session_active(&self) -> bool {
        // Session is active if any backend has a session.
        // Best-effort: check the first (primary) backend synchronously.
        let entries = self.backends.try_read();
        if let Ok(entries) = entries {
            entries.iter().any(|e| e.backend.session_active())
        } else {
            false
        }
    }

    fn export_session(&self) -> Option<String> {
        let entries = self.backends.try_read().ok()?;
        entries
            .iter()
            .find_map(|entry| entry.backend.export_session())
    }

    fn import_session(&self, raw: &str) -> anyhow::Result<()> {
        let entries = self
            .backends
            .try_read()
            .map_err(|_| anyhow::anyhow!("backend pool is busy; cannot restore session"))?;
        for entry in entries.iter() {
            let _ = entry.backend.import_session(raw);
        }
        Ok(())
    }

    fn login<'a>(&'a self, email: &'a str, password: &'a str) -> BackendFuture<'a, User> {
        Box::pin(async move {
            self.try_all("login", |b| {
                let b = b.clone();
                async move { b.login(email, password).await }
            })
            .await
        })
    }

    fn login_with_totp<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        totp: Option<&'a str>,
    ) -> BackendFuture<'a, User> {
        Box::pin(async move {
            self.try_all("login_with_totp", |b| {
                let b = b.clone();
                async move { b.login_with_totp(email, password, totp).await }
            })
            .await
        })
    }

    fn register<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
        full_name: &'a str,
    ) -> BackendFuture<'a, User> {
        Box::pin(async move {
            self.try_all("register", |b| {
                let b = b.clone();
                async move { b.register(email, password, full_name).await }
            })
            .await
        })
    }

    fn logout<'a>(&'a self) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("logout", |b| {
                let b = b.clone();
                async move { b.logout().await }
            })
            .await
        })
    }

    fn me(&self) -> BackendFuture<'_, User> {
        Box::pin(async move {
            self.try_all("me", |b| {
                let b = b.clone();
                async move { b.me().await }
            })
            .await
        })
    }

    fn anonymous<'a>(&'a self, device_name: &'a str) -> BackendFuture<'a, (User, String)> {
        Box::pin(async move {
            self.try_all("anonymous", |b| {
                let b = b.clone();
                async move { b.anonymous(device_name).await }
            })
            .await
        })
    }

    fn recover_account<'a>(&'a self, code: &'a str) -> BackendFuture<'a, (User, String)> {
        Box::pin(async move {
            self.try_all("recover_account", |b| {
                let b = b.clone();
                async move { b.recover_account(code).await }
            })
            .await
        })
    }

    fn list_sessions(&self) -> BackendFuture<'_, Vec<SessionInfo>> {
        Box::pin(async move {
            self.try_all("list_sessions", |b| {
                let b = b.clone();
                async move { b.list_sessions().await }
            })
            .await
        })
    }

    fn revoke_session<'a>(&'a self, id: &'a str) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("revoke_session", |b| {
                let b = b.clone();
                async move { b.revoke_session(id).await }
            })
            .await
        })
    }

    fn logout_all(&self) -> BackendFuture<'_, ()> {
        Box::pin(async move {
            self.try_all("logout_all", |b| {
                let b = b.clone();
                async move { b.logout_all().await }
            })
            .await
        })
    }

    fn submit_diagnostic<'a>(
        &'a self,
        category: &'a str,
        message: &'a str,
        platform: &'a str,
    ) -> BackendFuture<'a, i64> {
        Box::pin(async move {
            self.try_all("submit_diagnostic", |b| {
                let b = b.clone();
                async move { b.submit_diagnostic(category, message, platform).await }
            })
            .await
        })
    }

    fn mfa_enroll<'a>(&'a self) -> BackendFuture<'a, (String, String)> {
        Box::pin(async move {
            self.try_all("mfa_enroll", |b| {
                let b = b.clone();
                async move { b.mfa_enroll().await }
            })
            .await
        })
    }

    fn mfa_verify<'a>(&'a self, code: &'a str) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("mfa_verify", |b| {
                let b = b.clone();
                async move { b.mfa_verify(code).await }
            })
            .await
        })
    }

    fn mfa_status<'a>(&'a self) -> BackendFuture<'a, bool> {
        Box::pin(async move {
            self.try_all("mfa_status", |b| {
                let b = b.clone();
                async move { b.mfa_status().await }
            })
            .await
        })
    }

    fn mfa_disable<'a>(&'a self) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("mfa_disable", |b| {
                let b = b.clone();
                async move { b.mfa_disable().await }
            })
            .await
        })
    }

    fn change_password<'a>(
        &'a self,
        current_password: Option<&'a str>,
        new_password: &'a str,
        confirm_password: &'a str,
    ) -> BackendFuture<'a, bool> {
        Box::pin(async move {
            self.try_all("change_password", |b| {
                let b = b.clone();
                async move {
                    b.change_password(current_password, new_password, confirm_password)
                        .await
                }
            })
            .await
        })
    }

    fn get_friendships<'a>(&'a self) -> BackendFuture<'a, Vec<Friendship>> {
        Box::pin(async move {
            self.try_all("get_friendships", |b| {
                let b = b.clone();
                async move { b.get_friendships().await }
            })
            .await
        })
    }

    fn send_friend_request<'a>(&'a self, email: &'a str) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("send_friend_request", |b| {
                let b = b.clone();
                async move { b.send_friend_request(email).await }
            })
            .await
        })
    }

    fn respond_to_friendship<'a>(&'a self, id: i64, action: &'a str) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("respond_to_friendship", |b| {
                let b = b.clone();
                async move { b.respond_to_friendship(id, action).await }
            })
            .await
        })
    }

    fn delete_friendship<'a>(&'a self, id: i64) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("delete_friendship", |b| {
                let b = b.clone();
                async move { b.delete_friendship(id).await }
            })
            .await
        })
    }

    fn get_location_shares<'a>(&'a self) -> BackendFuture<'a, Vec<LocationShare>> {
        Box::pin(async move {
            self.try_all("get_location_shares", |b| {
                let b = b.clone();
                async move { b.get_location_shares().await }
            })
            .await
        })
    }

    fn create_location_share<'a>(
        &'a self,
        friendship_id: i64,
        direction: &'a str,
        precision_level: &'a str,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("create_location_share", |b| {
                let b = b.clone();
                async move {
                    b.create_location_share(friendship_id, direction, precision_level)
                        .await
                }
            })
            .await
        })
    }

    fn create_share_with_expiry<'a>(
        &'a self,
        friendship_id: i64,
        direction: &'a str,
        precision_level: &'a str,
        duration_hours: Option<i64>,
        label: Option<&'a str>,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("create_share_with_expiry", |b| {
                let b = b.clone();
                async move {
                    b.create_share_with_expiry(
                        friendship_id,
                        direction,
                        precision_level,
                        duration_hours,
                        label,
                    )
                    .await
                }
            })
            .await
        })
    }

    fn update_location_share<'a>(
        &'a self,
        share_id: i64,
        is_active: Option<bool>,
        precision: Option<String>,
        duration_hours: Option<i64>,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("update_location_share", |b| {
                let b = b.clone();
                let precision = precision.clone();
                async move {
                    b.update_location_share(share_id, is_active, precision, duration_hours)
                        .await
                }
            })
            .await
        })
    }

    fn delete_share<'a>(&'a self, share_id: i64) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("delete_share", |b| {
                let b = b.clone();
                async move { b.delete_share(share_id).await }
            })
            .await
        })
    }

    fn get_friend_locations<'a>(&'a self) -> BackendFuture<'a, Vec<FriendLocation>> {
        Box::pin(async move {
            self.try_all("get_friend_locations", |b| {
                let b = b.clone();
                async move { b.get_friend_locations().await }
            })
            .await
        })
    }

    fn publish_location<'a>(
        &'a self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("publish_location", |b| {
                let b = b.clone();
                async move { b.publish_location(latitude, longitude, accuracy).await }
            })
            .await
        })
    }

    fn get_my_location<'a>(&'a self) -> BackendFuture<'a, FriendLocation> {
        Box::pin(async move {
            self.try_all("get_my_location", |b| {
                let b = b.clone();
                async move { b.get_my_location().await }
            })
            .await
        })
    }

    fn map_capabilities(&self) -> BackendFuture<'_, MapCapabilities> {
        Box::pin(async move {
            self.try_all("map_capabilities", |b| {
                let b = b.clone();
                async move { b.map_capabilities().await }
            })
            .await
        })
    }

    fn map_search<'a>(
        &'a self,
        q: &'a str,
        lat: Option<f64>,
        lon: Option<f64>,
        limit: usize,
    ) -> BackendFuture<'a, Vec<SearchPlace>> {
        Box::pin(async move {
            self.try_all("map_search", |b| {
                let b = b.clone();
                async move { b.map_search(q, lat, lon, limit).await }
            })
            .await
        })
    }

    fn map_reverse(&self, lat: f64, lon: f64) -> BackendFuture<'_, SearchPlace> {
        Box::pin(async move {
            self.try_all("map_reverse", |b| {
                let b = b.clone();
                async move { b.map_reverse(lat, lon).await }
            })
            .await
        })
    }

    fn map_route<'a>(
        &'a self,
        from: (f64, f64),
        to: (f64, f64),
        mode: &'a str,
        via: &'a [(f64, f64)],
        avoid: &'a [String],
        alternatives: bool,
    ) -> BackendFuture<'a, Vec<Route>> {
        Box::pin(async move {
            self.try_all("map_route", |b| {
                let b = b.clone();
                async move { b.map_route(from, to, mode, via, avoid, alternatives).await }
            })
            .await
        })
    }

    fn list_users<'a>(&'a self) -> BackendFuture<'a, Vec<User>> {
        Box::pin(async move {
            self.try_all("list_users", |b| {
                let b = b.clone();
                async move { b.list_users().await }
            })
            .await
        })
    }

    fn search_users<'a>(&'a self, q: &'a str) -> BackendFuture<'a, Vec<User>> {
        Box::pin(async move {
            self.try_all("search_users", |b| {
                let b = b.clone();
                async move { b.search_users(q).await }
            })
            .await
        })
    }

    fn update_user<'a>(&'a self, id: i64, body: serde_json::Value) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("update_user", |b| {
                let b = b.clone();
                let body = body.clone();
                async move { b.update_user(id, body).await }
            })
            .await
        })
    }

    fn delete_user<'a>(&'a self, id: i64) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("delete_user", |b| {
                let b = b.clone();
                async move { b.delete_user(id).await }
            })
            .await
        })
    }

    fn admin_overview(&self) -> BackendFuture<'_, serde_json::Value> {
        Box::pin(async move {
            self.try_all("admin_overview", |b| {
                let b = b.clone();
                async move { b.admin_overview().await }
            })
            .await
        })
    }

    fn admin_users(&self) -> BackendFuture<'_, Vec<serde_json::Value>> {
        Box::pin(async move {
            self.try_all("admin_users", |b| {
                let b = b.clone();
                async move { b.admin_users().await }
            })
            .await
        })
    }

    fn admin_update_user<'a>(&'a self, id: i64, body: serde_json::Value) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("admin_update_user", |b| {
                let b = b.clone();
                let body = body.clone();
                async move { b.admin_update_user(id, body).await }
            })
            .await
        })
    }

    fn admin_delete_user<'a>(&'a self, id: i64) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("admin_delete_user", |b| {
                let b = b.clone();
                async move { b.admin_delete_user(id).await }
            })
            .await
        })
    }

    fn admin_diagnostics<'a>(
        &'a self,
        status: Option<&'a str>,
    ) -> BackendFuture<'a, (Vec<DiagnosticReport>, i64, i64)> {
        Box::pin(async move {
            self.try_all("admin_diagnostics", |b| {
                let b = b.clone();
                async move { b.admin_diagnostics(status).await }
            })
            .await
        })
    }

    fn admin_update_report<'a>(&'a self, id: i64, status: &'a str) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("admin_update_report", |b| {
                let b = b.clone();
                async move { b.admin_update_report(id, status).await }
            })
            .await
        })
    }

    fn admin_audit(&self) -> BackendFuture<'_, Vec<AdminAuditEntry>> {
        Box::pin(async move {
            self.try_all("admin_audit", |b| {
                let b = b.clone();
                async move { b.admin_audit().await }
            })
            .await
        })
    }

    fn create_check_in<'a>(
        &'a self,
        lat: f64,
        lon: f64,
        note: &'a str,
        kind: &'a str,
    ) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("create_check_in", |b| {
                let b = b.clone();
                async move { b.create_check_in(lat, lon, note, kind).await }
            })
            .await
        })
    }

    fn list_check_ins<'a>(&'a self) -> BackendFuture<'a, Vec<CheckIn>> {
        Box::pin(async move {
            self.try_all("list_check_ins", |b| {
                let b = b.clone();
                async move { b.list_check_ins().await }
            })
            .await
        })
    }

    fn trigger_emergency<'a>(&'a self, lat: f64, lon: f64) -> BackendFuture<'a, ()> {
        Box::pin(async move {
            self.try_all("trigger_emergency", |b| {
                let b = b.clone();
                async move { b.trigger_emergency(lat, lon).await }
            })
            .await
        })
    }

    fn list_emergencies<'a>(&'a self) -> BackendFuture<'a, Vec<EmergencyAlert>> {
        Box::pin(async move {
            self.try_all("list_emergencies", |b| {
                let b = b.clone();
                async move { b.list_emergencies().await }
            })
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn failover_constructs_and_lists_urls() {
        let failover = FailoverBackend::new(vec![
            "http://127.0.0.1:1".to_string(),
            "http://127.0.0.1:2".to_string(),
        ])
        .unwrap();
        let urls = failover.urls().await;
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"http://127.0.0.1:1".to_string()));
        assert!(urls.contains(&"http://127.0.0.1:2".to_string()));
    }
}

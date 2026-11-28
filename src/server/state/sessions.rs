//! Server-side sessions: opaque `session_id` + CSRF token.
//!
//! Stored in-memory for the single-binary demo and in `pinpoint_sessions`
//! (migration 14) when Postgres is configured, so the split auth/users/
//! locations services and replicas share the same session row.

use super::AppState;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct StoredSession {
    /// SHA-256 of the cookie value — a stolen DB dump cannot be replayed.
    pub session_id: String,
    pub user_id: i64,
    pub csrf_token: String,
    pub auth_method: String,
    pub mfa_pending: bool,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

fn random_hex() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Session ids are bearer secrets: only their hash is persisted or kept in
/// memory. Lookup hashes the cookie value first.
fn hash_session_id(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Idle timeout for an opaque session (sliding). `PINPOINT_SESSION_IDLE_MINUTES`
/// overrides it; the 2 h default keeps people signed in across a day's use, and
/// the docs show how to tighten it (30 min) for a public host.
const DEFAULT_IDLE_MINUTES: i64 = 120;

fn idle_minutes_from(value: Option<&str>) -> i64 {
    value
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|m| *m > 0)
        .unwrap_or(DEFAULT_IDLE_MINUTES)
}

fn session_ttl() -> ChronoDuration {
    let value = std::env::var("PINPOINT_SESSION_IDLE_MINUTES").ok();
    ChronoDuration::minutes(idle_minutes_from(value.as_deref()))
}

#[cfg(test)]
mod idle_timeout_tests {
    use super::{idle_minutes_from, DEFAULT_IDLE_MINUTES};

    #[test]
    fn the_default_idle_timeout_is_two_hours() {
        assert_eq!(DEFAULT_IDLE_MINUTES, 120);
        assert_eq!(idle_minutes_from(None), 120);
    }

    #[test]
    fn a_configured_value_overrides_the_default_and_bad_values_fall_back() {
        assert_eq!(idle_minutes_from(Some(" 30 ")), 30);
        assert_eq!(idle_minutes_from(Some("0")), 120, "zero is not a timeout");
        assert_eq!(idle_minutes_from(Some("soon")), 120);
    }
}

/// Absolute ceiling for a session, independent of activity.
/// `PINPOINT_SESSION_MAX_DAYS` overrides it; defaults to 30 days.
fn session_max_lifetime() -> ChronoDuration {
    let days = std::env::var("PINPOINT_SESSION_MAX_DAYS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|d| *d > 0)
        .unwrap_or(30);
    ChronoDuration::days(days)
}

/// Idle timeout in seconds, for the auth response's `expires_in` hint.
pub fn session_idle_seconds() -> i64 {
    session_ttl().num_seconds()
}

fn parse_ts(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

impl StoredSession {
    fn is_expired(&self) -> bool {
        let now = Utc::now();
        now - self.last_activity > session_ttl() || now - self.created_at > session_max_lifetime()
    }
}

impl AppState {
    /// Create a session and return `(session_id, csrf_token)`.
    pub async fn create_session(
        &self,
        user_id: i64,
        auth_method: &str,
        mfa_pending: bool,
    ) -> (String, String) {
        let raw_session_id = random_hex();
        let csrf_token = random_hex();
        let now = Utc::now();
        let session = StoredSession {
            session_id: hash_session_id(&raw_session_id),
            user_id,
            csrf_token: csrf_token.clone(),
            auth_method: auth_method.to_string(),
            mfa_pending,
            created_at: now,
            last_activity: now,
        };
        if let Some(pool) = &self.pg {
            let _ = sqlx::query(
                "INSERT INTO pinpoint_sessions (session_id, user_id, csrf_token, auth_method, mfa_pending) VALUES ($1,$2,$3,$4,$5)",
            )
            .bind(&session.session_id)
            .bind(session.user_id)
            .bind(&session.csrf_token)
            .bind(&session.auth_method)
            .bind(session.mfa_pending)
            .execute(pool)
            .await;
            // Expired rows are dead credentials. Delete them here so "until
            // expiry" is a real deletion and not only a rejected lookup.
            let cutoff = (Utc::now() - session_ttl()).to_rfc3339();
            let _ =
                sqlx::query("DELETE FROM pinpoint_sessions WHERE last_activity < $1::timestamptz")
                    .bind(cutoff)
                    .execute(pool)
                    .await;
        }
        // Opportunistic cleanup so revoked/abandoned sessions don't pile up.
        self.sessions.lock().unwrap().retain(|_, s| !s.is_expired());
        self.sessions
            .lock()
            .unwrap()
            .insert(session.session_id.clone(), session);
        (raw_session_id, csrf_token)
    }

    pub async fn get_session(&self, raw_session_id: &str) -> Option<StoredSession> {
        let session_id = hash_session_id(raw_session_id);
        if let Some(pool) = &self.pg {
            // PostgreSQL is the authority across replicas: this read confirms
            // the row still exists after a remote revocation and refreshes the
            // activity/created timestamps used for expiry.
            match sqlx::query_as::<_, (String, i64, String, String, bool, String, String)>(
                "SELECT session_id, user_id, csrf_token, auth_method, mfa_pending, created_at::text, last_activity::text FROM pinpoint_sessions WHERE session_id=$1",
            )
            .bind(&session_id)
            .fetch_optional(pool)
            .await
            {
                Ok(Some((
                    session_id,
                    user_id,
                    csrf_token,
                    auth_method,
                    mfa_pending,
                    created,
                    last,
                ))) => {
                    let s = StoredSession {
                        session_id,
                        user_id,
                        csrf_token,
                        auth_method,
                        mfa_pending,
                        created_at: parse_ts(&created),
                        last_activity: parse_ts(&last),
                    };
                    if s.is_expired() {
                        self.delete_session(&s.session_id).await;
                        return None;
                    }
                    self.sessions
                        .lock()
                        .unwrap()
                        .insert(s.session_id.clone(), s.clone());
                    return Some(s);
                }
                Ok(None) => {
                    // Revoked or pruned: drop the local copy as well.
                    self.sessions.lock().unwrap().remove(&session_id);
                    return None;
                }
                Err(_) => {
                    // Database blip: fall back to the last validated copy
                    // rather than signing every user out.
                }
            }
        }
        let cached = self.sessions.lock().unwrap().get(&session_id).cloned();
        if let Some(s) = cached {
            if s.is_expired() {
                self.delete_session(&session_id).await;
                return None;
            }
            return Some(s);
        }
        None
    }

    /// Active (non-expired) sessions for one user, newest activity first.
    pub async fn list_sessions(&self, user_id: i64) -> Vec<StoredSession> {
        let mut out: Vec<StoredSession> = self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.user_id == user_id && !s.is_expired())
            .cloned()
            .collect();
        if let Some(pool) = &self.pg {
            let rows: Vec<(String, i64, String, String, bool, String, String)> = sqlx::query_as(
                "SELECT session_id, user_id, csrf_token, auth_method, mfa_pending, created_at::text, last_activity::text FROM pinpoint_sessions WHERE user_id=$1",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            for (session_id, user_id, csrf_token, auth_method, mfa_pending, created, last) in rows {
                let s = StoredSession {
                    session_id,
                    user_id,
                    csrf_token,
                    auth_method,
                    mfa_pending,
                    created_at: parse_ts(&created),
                    last_activity: parse_ts(&last),
                };
                if s.is_expired() {
                    self.delete_session(&s.session_id).await;
                    continue;
                }
                if !out.iter().any(|e| e.session_id == s.session_id) {
                    out.push(s);
                }
            }
        }
        out.sort_by_key(|s| std::cmp::Reverse(s.last_activity));
        out
    }

    pub async fn delete_session(&self, session_id: &str) {
        self.sessions.lock().unwrap().remove(session_id);
        if let Some(pool) = &self.pg {
            let _ = sqlx::query("DELETE FROM pinpoint_sessions WHERE session_id=$1")
                .bind(session_id)
                .execute(pool)
                .await;
        }
    }

    pub async fn delete_user_sessions(&self, user_id: i64) {
        self.sessions
            .lock()
            .unwrap()
            .retain(|_, s| s.user_id != user_id);
        if let Some(pool) = &self.pg {
            let _ = sqlx::query("DELETE FROM pinpoint_sessions WHERE user_id=$1")
                .bind(user_id)
                .execute(pool)
                .await;
        }
    }

    /// Sliding activity refresh (memory every request, DB every 5 min).
    pub async fn touch_session(&self, session_id: &str) {
        let mut now = None;
        {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(s) = sessions.get_mut(session_id) {
                let current = Utc::now();
                if current - s.last_activity >= ChronoDuration::minutes(5) {
                    s.last_activity = current;
                    now = Some(current);
                }
            }
        }
        if now.is_some() {
            if let Some(pool) = &self.pg {
                let _ = sqlx::query(
                    "UPDATE pinpoint_sessions SET last_activity=now() WHERE session_id=$1",
                )
                .bind(session_id)
                .execute(pool)
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_round_trip_in_memory() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = AppState::new();
            let (sid, csrf) = state.create_session(7, "email", false).await;
            assert!(!sid.is_empty() && !csrf.is_empty());
            let s = state.get_session(&sid).await.expect("session exists");
            assert_eq!(s.user_id, 7);
            assert_eq!(s.csrf_token, csrf);
            assert!(!s.mfa_pending);
            assert_ne!(
                s.session_id, sid,
                "the stored id must be a hash, not the cookie value"
            );

            state.delete_session(&s.session_id).await;
            assert!(state.get_session(&sid).await.is_none());
        });
    }

    #[test]
    fn create_session_future_is_send() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = AppState::new();
            tokio::spawn(async move {
                let _ = state.create_session(1, "email", false).await;
            })
            .await
            .unwrap();
        });
    }

    #[test]
    fn session_absolute_lifetime_expires_even_when_active() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = AppState::new();
            let (sid, _) = state.create_session(1, "email", false).await;
            {
                let mut sessions = state.sessions.lock().unwrap();
                let session = sessions.get_mut(&hash_session_id(&sid)).unwrap();
                session.created_at =
                    Utc::now() - session_max_lifetime() - ChronoDuration::minutes(1);
                session.last_activity = Utc::now();
            }
            assert!(
                state.get_session(&sid).await.is_none(),
                "the absolute ceiling must win over fresh activity"
            );
        });
    }

    #[test]
    fn create_session_prunes_expired_rows() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = AppState::new();
            let (stale, _) = state.create_session(1, "email", false).await;
            {
                let mut sessions = state.sessions.lock().unwrap();
                let session = sessions.get_mut(&hash_session_id(&stale)).unwrap();
                session.last_activity = Utc::now() - session_ttl() - ChronoDuration::minutes(1);
            }
            let _ = state.create_session(2, "email", false).await;
            let sessions = state.sessions.lock().unwrap();
            assert_eq!(sessions.len(), 1, "expired session rows must be pruned");
            assert!(!sessions.contains_key(&hash_session_id(&stale)));
        });
    }

    #[test]
    fn delete_user_sessions_revokes_all() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = AppState::new();
            let (a, _) = state.create_session(1, "email", false).await;
            let (b, _) = state.create_session(1, "anonymous", false).await;
            let (c, _) = state.create_session(2, "email", false).await;
            state.delete_user_sessions(1).await;
            assert!(state.get_session(&a).await.is_none());
            assert!(state.get_session(&b).await.is_none());
            assert!(state.get_session(&c).await.is_some());
        });
    }
}

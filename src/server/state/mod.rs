//! In-memory store for the Rust API server (modular: per-domain sub-stores).
//!
//! `store.rs` holds the aggregate shape, `users.rs` / `friendships.rs` /
//! `locations.rs` hold per-table load/persist, and this file orchestrates:
//! ownership (`ServiceKind`), boot (`from_env`/`init_postgres`), whole-store
//! load/refresh, and the persist tick (one transaction per tick).
//!
//! Demo/local only. Production uses Postgres behind the same handler logic.

#![allow(clippy::type_complexity, clippy::field_reassign_with_default)]

mod diagnostics;
mod friendships;
mod locations;
pub(crate) mod messages;
mod sessions;
mod store;
mod users;

pub use sessions::session_idle_seconds;
pub use sessions::StoredSession;
pub use store::Store;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Remove erased accounts and every store row tied to them.
///
/// A concurrent persist can snapshot the store just before a delete commits
/// and upsert it just after. Tombstoning the ids and filtering every clone
/// before it reaches Postgres makes resurrection impossible.
fn strip_deleted_users(store: &mut Store, deleted: &HashSet<i64>) {
    if deleted.is_empty() {
        return;
    }
    let deleted_friendships: HashSet<i64> = store
        .friendships
        .iter()
        .filter(|f| deleted.contains(&f.requester_id) || deleted.contains(&f.receiver_id))
        .map(|f| f.id)
        .collect();
    store.users.retain(|u| !deleted.contains(&u.id));
    store
        .friendships
        .retain(|f| !deleted.contains(&f.requester_id) && !deleted.contains(&f.receiver_id));
    store.shares.retain(|s| {
        !deleted.contains(&s.owner_id) && !deleted_friendships.contains(&s.friendship_id)
    });
    store.locations.retain(|l| !deleted.contains(&l.user_id));
    store.check_ins.retain(|c| !deleted.contains(&c.user_id));
    store.emergencies.retain(|e| !deleted.contains(&e.user_id));
    store
        .diagnostic_reports
        .retain(|r| r.user_id.is_none_or(|id| !deleted.contains(&id)));
    store.admin_audit.retain(|a| !deleted.contains(&a.actor_id));
    store.message_keys.retain(|id, _| !deleted.contains(id));
    store
        .messages
        .retain(|m| !deleted.contains(&m.sender_id) && !deleted.contains(&m.recipient_id));
}

/// Application state shared between handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub pg: Option<sqlx::PgPool>,
    /// Which service owns this instance — scopes `persist()` to owned tables
    /// only, so services never clobber each other's rows.
    pub owner: ServiceKind,
    /// Last successful persist timestamp — used by `/sync-status` to report
    /// replication freshness across replicas.
    pub last_persist_at: Arc<Mutex<Option<SystemTime>>>,
    /// In-memory session cache (Postgres is authoritative when configured).
    pub sessions: Arc<Mutex<HashMap<String, StoredSession>>>,
    /// Accounts erased through the self/admin delete path. Stale snapshots
    /// must never upsert these ids back after the SQL delete.
    pub deleted_users: Arc<Mutex<HashSet<i64>>>,
}

/// Which deployable this `AppState` backs (controls persistence ownership).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    /// Combined binary (`pinpoint-server`) — owns everything.
    All,
    Auth,
    Users,
    Locations,
    MapOnly,
    /// Diagnostics ingestion service (port 8005): reports only.
    Diag,
    /// Admin API service (port 8006): users, audit, report triage.
    Admin,
}

impl ServiceKind {
    /// Stable lowercase name for `/status` and `/api/v1/services`.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Auth => "auth",
            Self::Users => "users",
            Self::Locations => "locations",
            Self::MapOnly => "map",
            Self::Diag => "diag",
            Self::Admin => "admin",
        }
    }

    pub fn owns_users(self) -> bool {
        matches!(self, Self::All | Self::Auth)
    }
    pub fn owns_friendships(self) -> bool {
        matches!(self, Self::All | Self::Users)
    }
    pub fn owns_shares(self) -> bool {
        matches!(self, Self::All | Self::Locations)
    }
    pub fn owns_locations(self) -> bool {
        matches!(self, Self::All | Self::Locations)
    }
    pub fn owns_checkins(self) -> bool {
        matches!(self, Self::All | Self::Locations)
    }
    pub fn owns_emergencies(self) -> bool {
        matches!(self, Self::All | Self::Locations)
    }
    pub fn owns_reports(self) -> bool {
        matches!(self, Self::All | Self::Diag | Self::Admin)
    }
    pub fn owns_audit(self) -> bool {
        matches!(self, Self::All | Self::Admin)
    }
    pub fn owns_messages(self) -> bool {
        matches!(self, Self::All)
    }
    pub fn owns_snapshot(self) -> bool {
        matches!(self, Self::All)
    }

    pub fn lock_key(self) -> i64 {
        match self {
            Self::All => 1,
            Self::Auth => 2,
            Self::Users => 3,
            Self::Locations => 4,
            Self::MapOnly => 5,
            Self::Diag => 6,
            Self::Admin => 7,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::for_owner(ServiceKind::All)
    }

    pub fn for_owner(owner: ServiceKind) -> Self {
        Self {
            store: Arc::new(Mutex::new(Store::new())),
            pg: None,
            owner,
            last_persist_at: Arc::new(Mutex::new(None)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            deleted_users: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn from_env() -> Self {
        Self::from_env_for(ServiceKind::All).await
    }

    pub async fn from_env_for(owner: ServiceKind) -> Self {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_default();
        if database_url.trim().is_empty() {
            return Self::for_owner(owner);
        }
        match sqlx::PgPool::connect(&database_url).await {
            Ok(pool) => {
                let app = AppState {
                    store: Arc::new(Mutex::new(Store::new())),
                    pg: Some(pool),
                    owner,
                    last_persist_at: Arc::new(Mutex::new(None)),
                    sessions: Arc::new(Mutex::new(HashMap::new())),
                    deleted_users: Arc::new(Mutex::new(HashSet::new())),
                };
                app.init_postgres().await;
                app
            }
            Err(e) => {
                eprintln!(
                    "warning: Postgres unavailable ({e}); running in-memory mode (data is not persisted)"
                );
                Self::for_owner(owner)
            }
        }
    }

    async fn init_postgres(&self) {
        let Some(pool) = &self.pg else { return };
        let applied = crate::server::migrations::apply_migrations(pool).await;
        if !applied.is_empty() {
            eprintln!("applied migrations: {:?}", applied);
        }
        if self.load_normalized().await {
            return;
        }
        let loaded: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT data FROM pinpoint_state WHERE id = 1")
                .fetch_optional(pool)
                .await
                .ok()
                .flatten();
        if let Some((data,)) = loaded {
            if let Ok(store) = serde_json::from_value::<Store>(data) {
                *self.store.lock().unwrap() = store;
                let _ = self.persist().await;
                return;
            }
        }
        self.persist().await;
    }

    async fn load_normalized(&self) -> bool {
        let Some(_) = &self.pg else {
            return false;
        };
        let mut store = Store::default();
        store.next_user_id = 1;
        store.next_friendship_id = 1;
        store.next_share_id = 1;
        store.next_check_in_id = 1;
        store.next_emergency_id = 1;
        store.next_report_id = 1;
        store.next_audit_id = 1;
        store.next_message_id = 1;
        if !self.load_users(&mut store).await {
            return false;
        }
        self.load_friendships(&mut store).await;
        self.load_locations_domain(&mut store).await;
        self.load_diagnostics(&mut store).await;
        if self.owner.owns_messages() {
            self.load_messages_domain(&mut store).await;
        }
        *self.store.lock().unwrap() = store;
        true
    }

    pub async fn refresh_from_db(&self) {
        let Some(_) = &self.pg else { return };
        self.load_normalized().await;
    }

    pub async fn persist(&self) {
        let Some(pool) = &self.pg else { return };
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(_) => return,
        };
        let _ = sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(self.owner.lock_key())
            .execute(&mut *tx)
            .await;
        let mut store = self.store.lock().unwrap().snapshot();
        {
            let deleted = self.deleted_users.lock().unwrap();
            strip_deleted_users(&mut store, &deleted);
        }
        if self.owner.owns_snapshot() {
            let data = serde_json::to_value(&store).unwrap_or_default();
            let _ = sqlx::query(
                "INSERT INTO pinpoint_state(id, data) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET data = EXCLUDED.data",
            )
            .bind(data)
            .execute(&mut *tx)
            .await;
        }
        self.persist_users(&mut tx, &store).await;
        self.persist_friendships(&mut tx, &store).await;
        self.persist_locations_domain(&mut tx, &store).await;
        if self.owner.owns_users() {
            let _ = sqlx::query("UPDATE pinpoint_counters SET next_user_id=$1 WHERE id=1")
                .bind(store.next_user_id)
                .execute(&mut *tx)
                .await;
        }
        if self.owner.owns_friendships() {
            let _ = sqlx::query("UPDATE pinpoint_counters SET next_friendship_id=$1 WHERE id=1")
                .bind(store.next_friendship_id)
                .execute(&mut *tx)
                .await;
        }
        if self.owner.owns_shares() {
            let _ = sqlx::query("UPDATE pinpoint_counters SET next_share_id=$1, next_checkin_id=$2, next_emergency_id=$3 WHERE id=1")
                .bind(store.next_share_id)
                .bind(store.next_check_in_id)
                .bind(store.next_emergency_id)
                .execute(&mut *tx)
                .await;
        }
        if self.owner.owns_reports() {
            let _ = sqlx::query("UPDATE pinpoint_counters SET next_report_id=$1 WHERE id=1")
                .bind(store.next_report_id)
                .execute(&mut *tx)
                .await;
        }
        if self.owner.owns_audit() {
            let _ = sqlx::query("UPDATE pinpoint_counters SET next_audit_id=$1 WHERE id=1")
                .bind(store.next_audit_id)
                .execute(&mut *tx)
                .await;
        }
        let _ = tx.commit().await;
        *self.last_persist_at.lock().unwrap() = Some(SystemTime::now());
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ServiceKind;

    #[test]
    fn ownership_matrix_is_exclusive() {
        assert!(ServiceKind::Auth.owns_users());
        assert!(!ServiceKind::Auth.owns_friendships());
        assert!(!ServiceKind::Auth.owns_shares());
        assert!(ServiceKind::Users.owns_friendships());
        assert!(!ServiceKind::Users.owns_users());
        assert!(!ServiceKind::Users.owns_locations());
        assert!(ServiceKind::Locations.owns_shares());
        assert!(ServiceKind::Locations.owns_locations());
        assert!(ServiceKind::Locations.owns_checkins());
        assert!(ServiceKind::Locations.owns_emergencies());
        assert!(!ServiceKind::Locations.owns_users());
        assert!(!ServiceKind::MapOnly.owns_users());
        assert!(!ServiceKind::MapOnly.owns_friendships());
        assert!(!ServiceKind::MapOnly.owns_locations());
        assert!(ServiceKind::Diag.owns_reports());
        assert!(!ServiceKind::Diag.owns_audit());
        assert!(!ServiceKind::Diag.owns_users());
        assert!(ServiceKind::Admin.owns_reports());
        assert!(ServiceKind::Admin.owns_audit());
        assert!(!ServiceKind::Admin.owns_users());
        assert!(!ServiceKind::Admin.owns_snapshot());
        assert!(ServiceKind::All.owns_users());
        assert!(ServiceKind::All.owns_snapshot());
        assert!(!ServiceKind::Auth.owns_snapshot());
    }

    #[test]
    fn lock_keys_are_distinct_per_service() {
        let kinds = [
            ServiceKind::All,
            ServiceKind::Auth,
            ServiceKind::Users,
            ServiceKind::Locations,
            ServiceKind::MapOnly,
            ServiceKind::Diag,
            ServiceKind::Admin,
        ];
        let keys: Vec<i64> = kinds.iter().map(|k| k.lock_key()).collect();
        let mut deduped = keys.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), keys.len(), "lock keys must be distinct");
        assert!(keys.iter().all(|k| *k > 0));
    }

    #[test]
    fn erased_users_are_stripped_from_every_persisted_domain() {
        use super::{strip_deleted_users, Store};
        use crate::models::{
            AdminAuditEntry, CheckIn, DiagnosticReport, EmergencyAlert, FriendLocation, Friendship,
            LocationShare, User,
        };
        use std::collections::HashSet;

        fn user(id: i64) -> User {
            User {
                id,
                username: String::new(),
                email: format!("user{id}@example.invalid"),
                full_name: format!("User {id}"),
                role: "resident".into(),
                is_active: true,
                must_change_password: false,
                password_hash: None,
                totp_secret: None,
                mfa_enabled: false,
                mfa_last_step: 0,
                failed_login_attempts: 0,
                locked_until: None,
                auth_method: "email".into(),
                public_key: None,
                recovery_code_hash: None,
                identity_subject: None,
            }
        }

        let mut store = Store {
            users: vec![user(1), user(2)],
            friendships: vec![Friendship {
                id: 10,
                requester_id: 2,
                receiver_id: 1,
                requester_name: "User 2".into(),
                receiver_name: "User 1".into(),
                status: "accepted".into(),
                created_at: None,
                responded_at: None,
            }],
            shares: vec![LocationShare {
                id: 20,
                friendship_id: 10,
                owner_id: 2,
                direction: "to_friend".into(),
                precision_level: "exact".into(),
                is_active: true,
                last_location_update_at: None,
                expires_at: None,
                label: None,
            }],
            locations: vec![FriendLocation {
                user_id: 2,
                user_name: "User 2".into(),
                latitude: 0.0,
                longitude: 0.0,
                precision_level: "exact".into(),
                updated_at: "now".into(),
                accuracy: None,
            }],
            check_ins: vec![CheckIn {
                id: 30,
                user_id: 2,
                user_name: "User 2".into(),
                latitude: 0.0,
                longitude: 0.0,
                note: String::new(),
                kind: "manual".into(),
                created_at: "now".into(),
            }],
            emergencies: vec![EmergencyAlert {
                id: 40,
                user_id: 2,
                user_name: "User 2".into(),
                latitude: 0.0,
                longitude: 0.0,
                created_at: "now".into(),
                resolved: false,
            }],
            diagnostic_reports: vec![DiagnosticReport {
                id: 50,
                user_id: Some(2),
                username: Some("User 2".into()),
                category: "other".into(),
                message: "test".into(),
                app_version: "1.5.2".into(),
                platform: "web".into(),
                status: "open".into(),
                created_at: "now".into(),
            }],
            admin_audit: vec![AdminAuditEntry {
                id: 60,
                actor_id: 2,
                actor_username: "User 2".into(),
                action: "user.self_delete".into(),
                target: "user:2".into(),
                detail: None,
                created_at: "now".into(),
            }],
            ..Default::default()
        };

        let deleted: HashSet<i64> = [2].into_iter().collect();
        strip_deleted_users(&mut store, &deleted);

        assert_eq!(store.users.len(), 1);
        assert_eq!(store.users[0].id, 1);
        assert!(store.friendships.is_empty());
        assert!(store.shares.is_empty());
        assert!(store.locations.is_empty());
        assert!(store.check_ins.is_empty());
        assert!(store.emergencies.is_empty());
        assert!(store.diagnostic_reports.is_empty());
        assert!(store.admin_audit.is_empty());
    }
}

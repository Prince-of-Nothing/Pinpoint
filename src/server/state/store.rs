//! Store: the shared in-memory aggregate (demo/local; Postgres-backed in prod).
//!
//! Split into per-domain sub-stores (`users`, `friendships`, `locations`)
//! so each service's data lives with its owner. `Store` is the facade the
//! handlers lock; domain modules (`users.rs`, `friendships.rs`,
//! `locations.rs`) hold the per-table load/persist logic. Serde shape is
//! unchanged (flat `users`/`friendships`/… keys) so the JSONB rollback
//! snapshot in `pinpoint_state` stays compatible.

use serde::{Deserialize, Serialize};

use crate::models::{
    AdminAuditEntry, CheckIn, DiagnosticReport, EmergencyAlert, FriendLocation, Friendship,
    LocationPoint, LocationShare, Message, PlaceNote, User,
};

/// Per-domain slices of the shared aggregate. `Store` keeps them as
/// separate fields (not one blob) so handlers and persistence already
/// work domain-by-domain; `mod.rs` orchestrates across them.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub users: Vec<User>,
    #[serde(default)]
    pub friendships: Vec<Friendship>,
    #[serde(default)]
    pub shares: Vec<LocationShare>,
    #[serde(default)]
    pub locations: Vec<FriendLocation>,
    #[serde(default)]
    pub check_ins: Vec<CheckIn>,
    #[serde(default)]
    pub emergencies: Vec<EmergencyAlert>,
    #[serde(default)]
    pub diagnostic_reports: Vec<DiagnosticReport>,
    #[serde(default)]
    pub admin_audit: Vec<AdminAuditEntry>,
    /// Private daily trail and place notes: never part of the shared JSON
    /// snapshot (photos can be large); loaded from their own tables and
    /// written directly.
    #[serde(skip)]
    pub location_history: Vec<LocationPoint>,
    #[serde(skip)]
    pub place_notes: Vec<PlaceNote>,
    /// E2E message public bundles by user id (JSON). Opaque to the server.
    #[serde(skip)]
    pub message_keys: std::collections::HashMap<i64, String>,
    /// E2E message envelopes. Never part of the shared JSON snapshot.
    #[serde(skip)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub next_user_id: i64,
    #[serde(default)]
    pub next_friendship_id: i64,
    #[serde(default)]
    pub next_share_id: i64,
    #[serde(default)]
    pub next_check_in_id: i64,
    #[serde(default)]
    pub next_emergency_id: i64,
    #[serde(default)]
    pub next_report_id: i64,
    #[serde(default)]
    pub next_audit_id: i64,
    #[serde(skip)]
    pub next_history_id: i64,
    #[serde(skip)]
    pub next_note_id: i64,
    #[serde(skip)]
    pub next_message_id: i64,
}

impl Store {
    pub fn new() -> Self {
        Self {
            next_user_id: 1,
            next_friendship_id: 1,
            next_share_id: 1,
            next_check_in_id: 1,
            next_emergency_id: 1,
            next_report_id: 1,
            next_audit_id: 1,
            next_history_id: 1,
            next_note_id: 1,
            next_message_id: 1,
            ..Default::default()
        }
    }

    /// Owned-field copy for the persist tick (lock released before I/O).
    pub(super) fn snapshot(&self) -> Self {
        Self {
            users: self.users.clone(),
            friendships: self.friendships.clone(),
            shares: self.shares.clone(),
            locations: self.locations.clone(),
            check_ins: self.check_ins.clone(),
            emergencies: self.emergencies.clone(),
            diagnostic_reports: self.diagnostic_reports.clone(),
            admin_audit: self.admin_audit.clone(),
            next_user_id: self.next_user_id,
            next_friendship_id: self.next_friendship_id,
            next_share_id: self.next_share_id,
            next_check_in_id: self.next_check_in_id,
            next_emergency_id: self.next_emergency_id,
            next_report_id: self.next_report_id,
            next_audit_id: self.next_audit_id,
            ..Default::default()
        }
    }

    pub fn find_user_by_email(&self, email: &str) -> Option<&User> {
        let email = email.trim();
        self.users
            .iter()
            .find(|u| u.email.trim().eq_ignore_ascii_case(email))
    }

    /// OAuth account lookup by `"provider:subject"` (never by email alone:
    /// that would let a provider identity take over a password account).
    pub fn find_user_by_identity(&self, subject: &str) -> Option<&User> {
        self.users
            .iter()
            .find(|u| u.identity_subject.as_deref() == Some(subject))
    }

    pub fn find_user_by_id(&self, id: i64) -> Option<&User> {
        self.users.iter().find(|u| u.id == id)
    }
}

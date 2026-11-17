//! Locations domain: `pinpoint_shares`, `pinpoint_locations`,
//! `pinpoint_checkins`, `pinpoint_emergencies` load / persist.
//!
//! Owner: Locations (`ServiceKind::owns_*`). All four tables are written
//! by the one service, parents-first (shares before the location tables
//! they reference) for upserts, children-first for deletes.

use super::{AppState, Store};
use crate::models::{CheckIn, EmergencyAlert, FriendLocation, LocationShare};

impl AppState {
    pub(super) async fn load_locations_domain(&self, store: &mut Store) {
        let Some(pool) = &self.pg else { return };
        self.load_shares(pool, store).await;
        self.load_locations(pool, store).await;
        self.load_check_ins(pool, store).await;
        self.load_emergencies(pool, store).await;
    }

    async fn load_shares(&self, pool: &sqlx::PgPool, store: &mut Store) {
        let rows: Vec<(
            i64,
            i64,
            i64,
            String,
            String,
            bool,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT id, friendship_id, owner_id, direction, precision_level, is_active, last_location_update_at, expires_at, label FROM pinpoint_shares ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (
            id,
            friendship_id,
            owner_id,
            direction,
            precision_level,
            is_active,
            last_location_update_at,
            expires_at,
            label,
        ) in rows
        {
            store.next_share_id = store.next_share_id.max(id + 1);
            store.shares.push(LocationShare {
                id,
                friendship_id,
                owner_id,
                direction,
                precision_level,
                is_active,
                last_location_update_at,
                expires_at,
                label,
            });
        }
    }

    async fn load_locations(&self, pool: &sqlx::PgPool, store: &mut Store) {
        let rows: Vec<(
            i64,
            Option<String>,
            f64,
            f64,
            Option<String>,
            Option<String>,
            Option<f64>,
        )> = sqlx::query_as(
            "SELECT user_id, user_name, latitude, longitude, precision_level, updated_at, accuracy FROM pinpoint_locations ORDER BY user_id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (user_id, user_name, latitude, longitude, precision_level, updated_at, accuracy) in rows
        {
            store.locations.push(FriendLocation {
                user_id,
                user_name: user_name.unwrap_or_default(),
                latitude,
                longitude,
                precision_level: precision_level.unwrap_or_else(|| "exact".to_string()),
                updated_at: updated_at.unwrap_or_default(),
                accuracy,
            });
        }
    }

    async fn load_check_ins(&self, pool: &sqlx::PgPool, store: &mut Store) {
        let rows: Vec<(
            i64,
            i64,
            Option<String>,
            f64,
            f64,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT id, user_id, user_name, latitude, longitude, note, kind, created_at FROM pinpoint_checkins ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (id, user_id, user_name, latitude, longitude, note, kind, created_at) in rows {
            store.next_check_in_id = store.next_check_in_id.max(id + 1);
            store.check_ins.push(CheckIn {
                id,
                user_id,
                user_name: user_name.unwrap_or_default(),
                latitude,
                longitude,
                note: note.unwrap_or_default(),
                kind: kind.unwrap_or_else(|| "manual".to_string()),
                created_at: created_at.unwrap_or_default(),
            });
        }
    }

    async fn load_emergencies(&self, pool: &sqlx::PgPool, store: &mut Store) {
        let rows: Vec<(i64, i64, Option<String>, f64, f64, Option<String>, bool)> =
            sqlx::query_as(
                "SELECT id, user_id, user_name, latitude, longitude, created_at, resolved FROM pinpoint_emergencies ORDER BY id",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        for (id, user_id, user_name, latitude, longitude, created_at, resolved) in rows {
            store.next_emergency_id = store.next_emergency_id.max(id + 1);
            store.emergencies.push(EmergencyAlert {
                id,
                user_id,
                user_name: user_name.unwrap_or_default(),
                latitude,
                longitude,
                created_at: created_at.unwrap_or_default(),
                resolved,
            });
        }
    }

    pub(super) async fn persist_locations_domain(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        store: &Store,
    ) {
        // Shares are written by sql_upsert_share/sql_delete_share only. A
        // whole-store snapshot would resurrect a share deleted by another
        // request or replica after the explicit DELETE.
        if self.owner.owns_locations() {
            for l in &store.locations {
                let _ = sqlx::query("INSERT INTO pinpoint_locations (user_id, user_name, latitude, longitude, precision_level, updated_at, accuracy) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (user_id) DO UPDATE SET user_name = EXCLUDED.user_name, latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude, precision_level = EXCLUDED.precision_level, updated_at = EXCLUDED.updated_at, accuracy = EXCLUDED.accuracy")
                    .bind(l.user_id).bind(&l.user_name).bind(l.latitude).bind(l.longitude).bind(&l.precision_level).bind(&l.updated_at).bind(l.accuracy)
                    .execute(&mut **tx).await;
            }
        }
        if self.owner.owns_checkins() {
            for c in &store.check_ins {
                let _ = sqlx::query("INSERT INTO pinpoint_checkins (id, user_id, user_name, latitude, longitude, note, kind, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (id) DO UPDATE SET user_id = EXCLUDED.user_id, user_name = EXCLUDED.user_name, latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude, note = EXCLUDED.note, kind = EXCLUDED.kind, created_at = EXCLUDED.created_at")
                    .bind(c.id).bind(c.user_id).bind(&c.user_name).bind(c.latitude).bind(c.longitude).bind(&c.note).bind(&c.kind).bind(&c.created_at)
                    .execute(&mut **tx).await;
            }
        }
        if self.owner.owns_emergencies() {
            for e in &store.emergencies {
                let _ = sqlx::query("INSERT INTO pinpoint_emergencies (id, user_id, user_name, latitude, longitude, created_at, resolved) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (id) DO UPDATE SET user_id = EXCLUDED.user_id, user_name = EXCLUDED.user_name, latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude, created_at = EXCLUDED.created_at, resolved = EXCLUDED.resolved")
                    .bind(e.id).bind(e.user_id).bind(&e.user_name).bind(e.latitude).bind(e.longitude).bind(&e.created_at).bind(e.resolved)
                    .execute(&mut **tx).await;
            }
        }
        // No delete-missing: deletions go through sql_delete_* (explicit)
        // so a stale snapshot can never erase a row another replica just
        // committed.
    }

    pub async fn sql_upsert_share(&self, share: &LocationShare) {
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query("INSERT INTO pinpoint_shares (id, friendship_id, owner_id, direction, precision_level, is_active, last_location_update_at, expires_at, label) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (id) DO UPDATE SET friendship_id = EXCLUDED.friendship_id, owner_id = EXCLUDED.owner_id, direction = EXCLUDED.direction, precision_level = EXCLUDED.precision_level, is_active = EXCLUDED.is_active, last_location_update_at = EXCLUDED.last_location_update_at, expires_at = EXCLUDED.expires_at, label = EXCLUDED.label")
            .bind(share.id)
            .bind(share.friendship_id)
            .bind(share.owner_id)
            .bind(&share.direction)
            .bind(&share.precision_level)
            .bind(share.is_active)
            .bind(&share.last_location_update_at)
            .bind(&share.expires_at)
            .bind(&share.label)
            .execute(pool)
            .await;
    }

    pub async fn sql_delete_share(&self, id: i64) {
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query("DELETE FROM pinpoint_shares WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await;
    }

    pub async fn sql_delete_check_in(&self, id: i64) {
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query("DELETE FROM pinpoint_checkins WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await;
    }

    pub async fn sql_delete_emergency(&self, id: i64) {
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query("DELETE FROM pinpoint_emergencies WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await;
    }
}

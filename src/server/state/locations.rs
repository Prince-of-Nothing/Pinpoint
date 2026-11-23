//! Locations domain: `pinpoint_shares`, `pinpoint_locations`,
//! `pinpoint_checkins`, `pinpoint_emergencies` load / persist.
//!
//! Owner: Locations (`ServiceKind::owns_*`). All four tables are written
//! by the one service, parents-first (shares before the location tables
//! they reference) for upserts, children-first for deletes.

use super::{AppState, Store};
use crate::models::{
    CheckIn, EmergencyAlert, FriendLocation, LocationPoint, LocationShare, PlaceNote,
};

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
        // Latest locations are written by sql_upsert_location only. A
        // whole-store snapshot would let a stale replica overwrite a newer
        // publish (and the location is the table's single row per user).
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

    pub async fn sql_upsert_location(&self, location: &FriendLocation) {
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query("INSERT INTO pinpoint_locations (user_id, user_name, latitude, longitude, precision_level, updated_at, accuracy) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (user_id) DO UPDATE SET user_name = EXCLUDED.user_name, latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude, precision_level = EXCLUDED.precision_level, updated_at = EXCLUDED.updated_at, accuracy = EXCLUDED.accuracy")
            .bind(location.user_id)
            .bind(&location.user_name)
            .bind(location.latitude)
            .bind(location.longitude)
            .bind(&location.precision_level)
            .bind(&location.updated_at)
            .bind(location.accuracy)
            .execute(pool)
            .await;
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

    /// Append one point to the owner's private daily trail. The store keeps a
    /// short window for immediate reads; Postgres keeps seven days.
    pub async fn record_location(
        &self,
        user_id: i64,
        latitude: f64,
        longitude: f64,
        accuracy: Option<f64>,
    ) {
        let point = {
            let mut store = self.store.lock().unwrap();
            let id = store.next_history_id;
            store.next_history_id += 1;
            let point = LocationPoint {
                id,
                user_id,
                latitude,
                longitude,
                accuracy,
                recorded_at: chrono::Utc::now().to_rfc3339(),
            };
            if self.pg.is_none() {
                store.location_history.push(point.clone());
            }
            point
        };
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query(
            "INSERT INTO pinpoint_location_history (user_id, latitude, longitude, accuracy, recorded_at) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(point.user_id)
        .bind(point.latitude)
        .bind(point.longitude)
        .bind(point.accuracy)
        .bind(&point.recorded_at)
        .execute(pool)
        .await;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let _ = sqlx::query(
            "DELETE FROM pinpoint_location_history WHERE user_id=$1 AND recorded_at < $2",
        )
        .bind(user_id)
        .bind(cutoff)
        .execute(pool)
        .await;
    }

    /// Delete trail points older than the documented seven days. Called on a
    /// fixed schedule so a user who stops publishing does not keep stale rows
    /// in Postgres (or in the in-memory demo store) indefinitely.
    pub async fn prune_location_history(&self) {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        if let Some(pool) = &self.pg {
            let _ = sqlx::query("DELETE FROM pinpoint_location_history WHERE recorded_at < $1")
                .bind(&cutoff)
                .execute(pool)
                .await;
        }
        let mut store = self.store.lock().unwrap();
        store.location_history.retain(|point| {
            chrono::DateTime::parse_from_rfc3339(&point.recorded_at)
                .map(|at| at.with_timezone(&chrono::Utc).to_rfc3339() >= cutoff)
                .unwrap_or(false)
        });
    }

    /// Hourly retention task. Only services that own locations should call
    /// this; the delete is global and idempotent across replicas.
    pub fn spawn_location_pruner(&self) {
        let state = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                ticker.tick().await;
                state.prune_location_history().await;
            }
        });
    }

    /// Insert a private note and return the durable id (the in-memory id when
    /// no database is configured).
    pub async fn sql_insert_note(&self, note: &PlaceNote) -> Option<i64> {
        let Some(pool) = &self.pg else {
            return Some(note.id);
        };
        let row: Option<(i64,)> = sqlx::query_as(
            "INSERT INTO pinpoint_place_notes (user_id, latitude, longitude, label, message, photo, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id",
        )
        .bind(note.user_id)
        .bind(note.latitude)
        .bind(note.longitude)
        .bind(&note.label)
        .bind(&note.message)
        .bind(&note.photo)
        .bind(&note.created_at)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        row.map(|(id,)| id)
    }

    pub async fn sql_delete_place_note(&self, id: i64, user_id: i64) {
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query("DELETE FROM pinpoint_place_notes WHERE id=$1 AND user_id=$2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_removes_points_older_than_seven_days() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = AppState::new();
            let point = |id: i64, days_ago: i64| LocationPoint {
                id,
                user_id: 1,
                latitude: 47.0,
                longitude: 28.0,
                accuracy: None,
                recorded_at: (chrono::Utc::now() - chrono::Duration::days(days_ago)).to_rfc3339(),
            };
            state.store.lock().unwrap().location_history = vec![point(1, 8), point(2, 1)];
            state.prune_location_history().await;
            let store = state.store.lock().unwrap();
            assert_eq!(store.location_history.len(), 1);
            assert_eq!(store.location_history[0].id, 2);
        });
    }
}

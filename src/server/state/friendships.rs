//! Friendships domain: `pinpoint_friendships` load / persist.
//!
//! Owner: Users (`ServiceKind::owns_friendships`).

use super::{AppState, Store};
use crate::models::Friendship;

impl AppState {
    pub(super) async fn load_friendships(&self, store: &mut Store) {
        let Some(pool) = &self.pg else { return };
        let rows: Vec<(
            i64,
            i64,
            i64,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT id, requester_id, receiver_id, requester_name, receiver_name, status, created_at, responded_at FROM pinpoint_friendships ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (id, requester_id, receiver_id, rn, dn, status, created_at, responded_at) in rows {
            store.next_friendship_id = store.next_friendship_id.max(id + 1);
            store.friendships.push(Friendship {
                id,
                requester_id,
                receiver_id,
                requester_name: rn.unwrap_or_default(),
                receiver_name: dn.unwrap_or_default(),
                status,
                created_at,
                responded_at,
            });
        }
    }

    pub(super) async fn persist_friendships(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _store: &Store,
    ) {
        // Friendships are written by sql_upsert_friendship/sql_delete_friendship
        // only. A whole-store snapshot would resurrect a removed friendship
        // (or a request another replica just accepted) after the explicit write.
    }

    pub async fn sql_upsert_friendship(&self, f: &Friendship) {
        let Some(pool) = &self.pg else { return };
        if let Err(error) = sqlx::query("INSERT INTO pinpoint_friendships (id, requester_id, receiver_id, requester_name, receiver_name, status, created_at, responded_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (id) DO UPDATE SET requester_id = EXCLUDED.requester_id, receiver_id = EXCLUDED.receiver_id, requester_name = EXCLUDED.requester_name, receiver_name = EXCLUDED.receiver_name, status = EXCLUDED.status, created_at = EXCLUDED.created_at, responded_at = EXCLUDED.responded_at")
            .bind(f.id)
            .bind(f.requester_id)
            .bind(f.receiver_id)
            .bind(&f.requester_name)
            .bind(&f.receiver_name)
            .bind(&f.status)
            .bind(&f.created_at)
            .bind(&f.responded_at)
            .execute(pool)
            .await
        {
            eprintln!("pinpoint_friendships upsert failed: {error}");
        }
    }

    pub async fn sql_delete_friendship(&self, id: i64) {
        let Some(pool) = &self.pg else { return };
        let _ = sqlx::query("DELETE FROM pinpoint_friendships WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await;
    }
}

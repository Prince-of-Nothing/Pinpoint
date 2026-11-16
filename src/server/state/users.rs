//! Users domain: `pinpoint_users` load / persist / direct-SQL mutations.
//!
//! Owner: Auth (`ServiceKind::owns_users`). The users service never
//! persists user rows here — it uses `sql_*` single-row helpers so it
//! cannot clobber credentials written by Auth.

use super::{AppState, Store};
use crate::models::User;

impl AppState {
    pub(super) async fn load_users(&self, store: &mut Store) -> bool {
        let Some(pool) = &self.pg else {
            return false;
        };
        let rows: Vec<(
            i64,
            String,
            String,
            String,
            bool,
            bool,
            Option<String>,
            Option<String>,
            bool,
            i64,
            i32,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT id, email, full_name, role, is_active, must_change_password, password_hash, totp_secret, mfa_enabled, mfa_last_step, failed_login_attempts, locked_until, auth_method, public_key, recovery_code_hash, identity_subject FROM pinpoint_users ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        if rows.is_empty() {
            return false;
        }
        for (
            id,
            email,
            full_name,
            role,
            is_active,
            must_change_password,
            password_hash,
            totp_secret,
            mfa_enabled,
            mfa_last_step,
            failed_login_attempts,
            locked_until,
            auth_method,
            public_key,
            recovery_code_hash,
            identity_subject,
        ) in rows
        {
            store.next_user_id = store.next_user_id.max(id + 1);
            store.users.push(User {
                id,
                username: String::new(),
                email,
                full_name,
                role,
                is_active,
                must_change_password,
                password_hash,
                totp_secret,
                mfa_enabled,
                mfa_last_step: mfa_last_step.max(0) as u64,
                failed_login_attempts: failed_login_attempts.max(0) as u32,
                locked_until,
                auth_method,
                public_key,
                recovery_code_hash,
                identity_subject,
            });
        }
        true
    }

    pub(super) async fn persist_users(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        store: &Store,
    ) {
        if !self.owner.owns_users() {
            return;
        }
        for u in &store.users {
            let _ = sqlx::query("INSERT INTO pinpoint_users (id, email, full_name, role, is_active, must_change_password, password_hash, totp_secret, mfa_enabled, mfa_last_step, failed_login_attempts, locked_until, auth_method, public_key, recovery_code_hash, identity_subject) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email, full_name = EXCLUDED.full_name, role = EXCLUDED.role, is_active = EXCLUDED.is_active, must_change_password = EXCLUDED.must_change_password, password_hash = EXCLUDED.password_hash, totp_secret = EXCLUDED.totp_secret, mfa_enabled = EXCLUDED.mfa_enabled, mfa_last_step = EXCLUDED.mfa_last_step, failed_login_attempts = EXCLUDED.failed_login_attempts, locked_until = EXCLUDED.locked_until, auth_method = EXCLUDED.auth_method, public_key = EXCLUDED.public_key, recovery_code_hash = EXCLUDED.recovery_code_hash, identity_subject = EXCLUDED.identity_subject")
                .bind(u.id).bind(&u.email).bind(&u.full_name).bind(&u.role).bind(u.is_active).bind(u.must_change_password).bind(&u.password_hash).bind(&u.totp_secret).bind(u.mfa_enabled).bind(u.mfa_last_step as i64).bind(u.failed_login_attempts as i32).bind(&u.locked_until).bind(&u.auth_method).bind(&u.public_key).bind(&u.recovery_code_hash).bind(&u.identity_subject)
                .execute(&mut **tx).await;
        }
        // NOTE: no delete-missing for users. Two services write this table
        // (Auth upserts here, Users inserts via direct SQL); a stale Auth
        // snapshot must never erase a user the Users service just created.
        // Deletions are explicit (`sql_delete_user` + handler retain) only.
        // Keep the sequence ahead of every id this tick wrote, so the next
        // `sql_insert_user` (NEXTVAL) can never collide with a persisted id.
        let _ = sqlx::query(
            "SELECT setval('pinpoint_user_id_seq', GREATEST((SELECT COALESCE(MAX(id), 0) + 1 FROM pinpoint_users), (SELECT last_value FROM pinpoint_user_id_seq) + 1), false)",
        )
        .execute(&mut **tx)
        .await;
    }

    /// Direct user insert for the users service (Auth owns `pinpoint_users`
    /// for persist upserts, so the users service must upsert single rows —
    /// never delete+rewrite — to avoid clobbering credentials written by Auth).
    /// Ids come from the `pinpoint_user_id_seq` sequence (migration v11):
    /// concurrent replicas can never pick the same id, unlike the old
    /// `MAX(id)+1` race. Falls back to the in-memory counter when the
    /// DB is unreachable (demo mode: in-memory store is authoritative).
    pub async fn sql_insert_user(
        &self,
        email: &str,
        full_name: &str,
        role: &str,
        password_hash: Option<&str>,
    ) -> i64 {
        if let Some(pool) = &self.pg {
            let row: Option<(i64,)> = sqlx::query_as(
                "INSERT INTO pinpoint_users (id, email, full_name, role, is_active, must_change_password, password_hash, auth_method, public_key, recovery_code_hash, identity_subject) VALUES (NEXTVAL('pinpoint_user_id_seq'), $1, $2, $3, true, false, $4, 'email', NULL, NULL, NULL) RETURNING id",
            )
            .bind(email)
            .bind(full_name)
            .bind(role)
            .bind(password_hash)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            if let Some((id,)) = row {
                return id;
            }
        }
        let mut store = self.store.lock().unwrap();
        let id = store.next_user_id;
        store.next_user_id += 1;
        id
    }

    /// Direct user update; no-op when DB unreachable/in-memory mode.
    pub async fn sql_update_user(
        &self,
        id: i64,
        full_name: Option<&str>,
        role: Option<&str>,
        is_active: Option<bool>,
    ) {
        let Some(pool) = &self.pg else { return };
        if let Some(n) = full_name {
            let _ = sqlx::query("UPDATE pinpoint_users SET full_name=$1 WHERE id=$2")
                .bind(n)
                .bind(id)
                .execute(pool)
                .await;
        }
        if let Some(r) = role {
            let _ = sqlx::query("UPDATE pinpoint_users SET role=$1 WHERE id=$2")
                .bind(r)
                .bind(id)
                .execute(pool)
                .await;
        }
        if let Some(a) = is_active {
            let _ = sqlx::query("UPDATE pinpoint_users SET is_active=$1 WHERE id=$2")
                .bind(a)
                .bind(id)
                .execute(pool)
                .await;
        }
    }

    /// Direct user delete; no-op in in-memory mode (store handles it).
    /// Delete a user and every row that references them. Runs in one
    /// transaction and is safe even when the production schema has foreign
    /// keys without `ON DELETE CASCADE`.
    pub async fn sql_delete_user(&self, id: i64) {
        let Some(pool) = &self.pg else { return };
        let Ok(mut tx) = pool.begin().await else {
            return;
        };
        let statements = [
            // Shares owned by the user, or attached to a friendship involving
            // the user, must go before the friendship/user rows.
            "DELETE FROM pinpoint_shares WHERE owner_id=$1 OR friendship_id IN (SELECT id FROM pinpoint_friendships WHERE requester_id=$1 OR receiver_id=$1)",
            "DELETE FROM pinpoint_friendships WHERE requester_id=$1 OR receiver_id=$1",
            "DELETE FROM pinpoint_locations WHERE user_id=$1",
            "DELETE FROM pinpoint_checkins WHERE user_id=$1",
            "DELETE FROM pinpoint_emergencies WHERE user_id=$1",
            "DELETE FROM pinpoint_diagnostic_reports WHERE user_id=$1",
            "DELETE FROM pinpoint_sessions WHERE user_id=$1",
            "DELETE FROM pinpoint_admin_audit WHERE actor_id=$1",
            "DELETE FROM pinpoint_users WHERE id=$1",
        ];
        for statement in statements {
            let _ = sqlx::query(statement).bind(id).execute(&mut *tx).await;
        }
        let _ = tx.commit().await;
    }
}

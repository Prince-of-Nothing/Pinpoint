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
        let deleted = self.deleted_users.lock().unwrap();
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
            if deleted.contains(&id) {
                continue;
            }
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
        // The sequence only exists after migration 11/12. Guard the lookup so
        // a database that skipped those migrations still persists every other
        // table in this transaction instead of aborting on a missing relation.
        let _ = sqlx::query(
            r#"DO $$ BEGIN
                   IF to_regclass('pinpoint_user_id_seq') IS NOT NULL THEN
                       EXECUTE 'SELECT setval(''pinpoint_user_id_seq'', GREATEST((SELECT COALESCE(MAX(id), 0) + 1 FROM pinpoint_users), (SELECT last_value FROM pinpoint_user_id_seq) + 1), false)';
                   END IF;
               END $$"#,
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
            // Migration 11/12 can be skipped when the service roles do not
            // exist, leaving no sequence. Persist the row anyway with the next
            // free id so registration never lives only in memory.
            let row: Option<(i64,)> = sqlx::query_as(
                "INSERT INTO pinpoint_users (id, email, full_name, role, is_active, must_change_password, password_hash, auth_method, public_key, recovery_code_hash, identity_subject) VALUES ((SELECT COALESCE(MAX(id), 0) + 1 FROM pinpoint_users), $1, $2, $3, true, false, $4, 'email', NULL, NULL, NULL) RETURNING id",
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

    /// Strict user insert for registration.
    ///
    /// Unlike [`Self::sql_insert_user`], this returns an error when Postgres is
    /// configured and the row cannot be written. Registration must never
    /// answer 201 for an account that exists only in this process's memory: a
    /// background refresh or another replica would drop it.
    pub async fn sql_insert_user_strict(
        &self,
        email: &str,
        full_name: &str,
        role: &str,
        password_hash: Option<&str>,
    ) -> Result<i64, String> {
        if let Some(pool) = &self.pg {
            let id: i64 =
                match sqlx::query_scalar::<_, i64>("SELECT NEXTVAL('pinpoint_user_id_seq')")
                    .fetch_one(pool)
                    .await
                {
                    Ok(id) => id,
                    Err(_) => sqlx::query_scalar::<_, i64>(
                        "SELECT COALESCE(MAX(id), 0) + 1 FROM pinpoint_users",
                    )
                    .fetch_one(pool)
                    .await
                    .map_err(|error| error.to_string())?,
                };
            sqlx::query(
                "INSERT INTO pinpoint_users (id, email, full_name, role, is_active, must_change_password, password_hash, auth_method, public_key, recovery_code_hash, identity_subject) VALUES ($1, $2, $3, $4, true, false, $5, 'email', NULL, NULL, NULL)",
            )
            .bind(id)
            .bind(email)
            .bind(full_name)
            .bind(role)
            .bind(password_hash)
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?;
            return Ok(id);
        }
        let mut store = self.store.lock().unwrap();
        let id = store.next_user_id;
        store.next_user_id += 1;
        Ok(id)
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
    ///
    /// Delete the account row and every row that references it. Statements run
    /// independently on purpose: a restricted runtime role, an append-only
    /// audit policy, or one optional table must never abort the cleanup and
    /// leave the account usable. Failures are logged by domain only (never
    /// credentials or coordinates). If the user row cannot be physically
    /// removed because of a foreign key, its personal fields are scrubbed and
    /// the account is disabled instead.
    pub async fn sql_delete_user(&self, id: i64) {
        // Tombstone first: a persist that snapshotted the store just before
        // this call must not upsert the account back afterwards.
        self.deleted_users.lock().unwrap().insert(id);
        let Some(pool) = &self.pg else { return };
        let statements = [
            // Shares owned by the user, or attached to a friendship involving
            // the user, must go before the friendship/user rows.
            ("shares", "DELETE FROM pinpoint_shares WHERE owner_id=$1 OR friendship_id IN (SELECT id FROM pinpoint_friendships WHERE requester_id=$1 OR receiver_id=$1)"),
            ("friendships", "DELETE FROM pinpoint_friendships WHERE requester_id=$1 OR receiver_id=$1"),
            ("locations", "DELETE FROM pinpoint_locations WHERE user_id=$1"),
            ("check-ins", "DELETE FROM pinpoint_checkins WHERE user_id=$1"),
            ("emergencies", "DELETE FROM pinpoint_emergencies WHERE user_id=$1"),
            ("location-history", "DELETE FROM pinpoint_location_history WHERE user_id=$1"),
            ("place-notes", "DELETE FROM pinpoint_place_notes WHERE user_id=$1"),
            ("diagnostic-reports", "DELETE FROM pinpoint_diagnostic_reports WHERE user_id=$1"),
            ("sessions", "DELETE FROM pinpoint_sessions WHERE user_id=$1"),
            ("admin-audit", "DELETE FROM pinpoint_admin_audit WHERE actor_id=$1"),
            // The JSONB rollback snapshot can predate normalized deletes; drop
            // it so a restart cannot restore the erased account.
            ("rollback-snapshot", "DELETE FROM pinpoint_state WHERE id=1"),
            ("user", "DELETE FROM pinpoint_users WHERE id=$1"),
        ];
        for (domain, statement) in statements {
            match sqlx::query(statement).bind(id).execute(pool).await {
                Ok(result) if result.rows_affected() > 0 => {
                    eprintln!("delete user {id}: {domain} rows={}", result.rows_affected());
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("delete user {id}: {domain} cleanup failed: {error}");
                    // Least-privilege roles may hold UPDATE but not DELETE.
                    let fallback = match domain {
                        "sessions" => Some(
                            "UPDATE pinpoint_sessions SET last_activity = to_timestamp(0) WHERE user_id=$1",
                        ),
                        "diagnostic-reports" => Some(
                            "UPDATE pinpoint_diagnostic_reports SET user_id=NULL, username=NULL WHERE user_id=$1",
                        ),
                        "admin-audit" => Some(
                            "UPDATE pinpoint_admin_audit SET actor_id=0, actor_username='deleted', detail=NULL WHERE actor_id=$1",
                        ),
                        "user" => Some(
                            "UPDATE pinpoint_users SET email=concat('deleted-', id, '@erased.invalid'), full_name='Deleted user', role='resident', is_active=false, must_change_password=false, password_hash=NULL, totp_secret=NULL, mfa_enabled=false, mfa_last_step=0, failed_login_attempts=0, locked_until=NULL, auth_method='deleted', public_key=NULL, recovery_code_hash=NULL, identity_subject=NULL WHERE id=$1",
                        ),
                        _ => None,
                    };
                    if let Some(fallback) = fallback {
                        match sqlx::query(fallback).bind(id).execute(pool).await {
                            Ok(result) => eprintln!(
                                "delete user {id}: {domain} fallback rows={}",
                                result.rows_affected()
                            ),
                            Err(error) => {
                                eprintln!("delete user {id}: {domain} fallback failed: {error}")
                            }
                        }
                    }
                }
            }
        }
        match sqlx::query_as::<_, (i64,)>("SELECT id FROM pinpoint_users WHERE id=$1")
            .bind(id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(_)) => eprintln!("delete user {id}: user row retained after scrub"),
            Ok(None) => {}
            Err(error) => eprintln!("delete user {id}: verification failed: {error}"),
        }
    }
}

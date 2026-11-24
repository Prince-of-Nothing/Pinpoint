//! Typed migration history for the modular Postgres schema.
//!
//! Each entry is an ordered (version, name, statements) triple applied once
//! and recorded in `pinpoint_schema_migrations`. All statements are
//! idempotent (`IF NOT EXISTS`), so old databases (created by the legacy
//! CREATE-block) converge by simply recording versions. New tables/columns
//! go here — never ad-hoc DDL elsewhere.

pub const MIGRATIONS: &[(u32, &str, &[&str])] = &[
    (
        1,
        "users",
        &["CREATE TABLE IF NOT EXISTS pinpoint_users (id BIGINT PRIMARY KEY, email TEXT UNIQUE NOT NULL, full_name TEXT NOT NULL, role TEXT NOT NULL, is_active BOOLEAN NOT NULL DEFAULT true, must_change_password BOOLEAN NOT NULL DEFAULT false, password_hash TEXT, totp_secret TEXT, mfa_enabled BOOLEAN NOT NULL DEFAULT false, mfa_last_step BIGINT NOT NULL DEFAULT 0)"],
    ),
    (
        2,
        "friendships",
        &["CREATE TABLE IF NOT EXISTS pinpoint_friendships (id BIGINT PRIMARY KEY, requester_id BIGINT NOT NULL, receiver_id BIGINT NOT NULL, requester_name TEXT, receiver_name TEXT, status TEXT NOT NULL, created_at TEXT, responded_at TEXT)"],
    ),
    (
        3,
        "shares",
        &["CREATE TABLE IF NOT EXISTS pinpoint_shares (id BIGINT PRIMARY KEY, friendship_id BIGINT NOT NULL, owner_id BIGINT NOT NULL, direction TEXT NOT NULL, precision_level TEXT NOT NULL, is_active BOOLEAN NOT NULL DEFAULT true, last_location_update_at TEXT, expires_at TEXT, label TEXT)"],
    ),
    (
        4,
        "locations",
        &["CREATE TABLE IF NOT EXISTS pinpoint_locations (user_id BIGINT PRIMARY KEY, user_name TEXT, latitude DOUBLE PRECISION NOT NULL, longitude DOUBLE PRECISION NOT NULL, precision_level TEXT, updated_at TEXT, accuracy DOUBLE PRECISION)"],
    ),
    (
        5,
        "checkins",
        &["CREATE TABLE IF NOT EXISTS pinpoint_checkins (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, user_name TEXT, latitude DOUBLE PRECISION, longitude DOUBLE PRECISION, note TEXT, kind TEXT, created_at TEXT)"],
    ),
    (
        6,
        "emergencies",
        &["CREATE TABLE IF NOT EXISTS pinpoint_emergencies (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, user_name TEXT, latitude DOUBLE PRECISION, longitude DOUBLE PRECISION, created_at TEXT, resolved BOOLEAN NOT NULL DEFAULT false)"],
    ),
    (
        7,
        "counters",
        &["CREATE TABLE IF NOT EXISTS pinpoint_counters (id INT PRIMARY KEY CHECK (id = 1), next_user_id BIGINT, next_friendship_id BIGINT, next_share_id BIGINT, next_checkin_id BIGINT, next_emergency_id BIGINT)"],
    ),
    (
        8,
        "state_rollback",
        &["CREATE TABLE IF NOT EXISTS pinpoint_state (id INT PRIMARY KEY CHECK (id = 1), data JSONB NOT NULL)"],
    ),
    (
        9,
        "counters_seed",
        &["INSERT INTO pinpoint_counters (id, next_user_id, next_friendship_id, next_share_id, next_checkin_id, next_emergency_id) VALUES (1, 1, 1, 1, 1, 1) ON CONFLICT (id) DO NOTHING"],
    ),
    (
        10,
        "mfa_columns",
        &[
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS totp_secret TEXT",
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS mfa_enabled BOOLEAN NOT NULL DEFAULT false",
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS mfa_last_step BIGINT NOT NULL DEFAULT 0",
        ],
    ),
    (
        11,
        "user_id_sequence",
        &[
            "CREATE SEQUENCE IF NOT EXISTS pinpoint_user_id_seq",
            "SELECT setval('pinpoint_user_id_seq', COALESCE((SELECT MAX(id) FROM pinpoint_users), 0) + 1, false)",
            r#"DO $$ BEGIN
                   IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'pinpoint_users') THEN
                       EXECUTE 'GRANT USAGE, SELECT ON SEQUENCE pinpoint_user_id_seq TO pinpoint_users';
                   END IF;
                   IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'pinpoint_auth') THEN
                       EXECUTE 'GRANT USAGE, SELECT ON SEQUENCE pinpoint_user_id_seq TO pinpoint_auth';
                   END IF;
               END $$"#,
        ],
    ),
    (
        12,
        "user_id_sequence_grant",
        &[r#"DO $$ BEGIN
                   IF to_regclass('pinpoint_user_id_seq') IS NULL THEN
                       EXECUTE 'CREATE SEQUENCE pinpoint_user_id_seq';
                       EXECUTE 'SELECT setval(''pinpoint_user_id_seq'', COALESCE((SELECT MAX(id) FROM pinpoint_users), 0) + 1, false)';
                   END IF;
                   IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'pinpoint_users') THEN
                       EXECUTE 'GRANT USAGE, SELECT ON SEQUENCE pinpoint_user_id_seq TO pinpoint_users';
                   END IF;
                   IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'pinpoint_auth') THEN
                       EXECUTE 'GRANT USAGE, SELECT ON SEQUENCE pinpoint_user_id_seq TO pinpoint_auth';
                   END IF;
               END $$"#],
    ),
    (
        13,
        "login_lockout",
        &[
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS failed_login_attempts INT NOT NULL DEFAULT 0",
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS locked_until TEXT",
        ],
    ),
    (
        14,
        "auth_methods_and_sessions",
        &[
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS auth_method TEXT NOT NULL DEFAULT 'email'",
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS public_key TEXT",
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS recovery_code_hash TEXT",
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS identity_subject TEXT",
            "CREATE TABLE IF NOT EXISTS pinpoint_sessions (session_id TEXT PRIMARY KEY, user_id BIGINT NOT NULL, csrf_token TEXT NOT NULL, auth_method TEXT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), last_activity TIMESTAMPTZ NOT NULL DEFAULT now(), mfa_pending BOOLEAN NOT NULL DEFAULT false)",
            "CREATE INDEX IF NOT EXISTS idx_sessions_user ON pinpoint_sessions(user_id)",
        ],
    ),
    (
        15,
        "observability",
        &[
            "ALTER TABLE pinpoint_users ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now()",
            "CREATE SEQUENCE IF NOT EXISTS pinpoint_report_id_seq",
            "CREATE SEQUENCE IF NOT EXISTS pinpoint_audit_id_seq",
            "CREATE TABLE IF NOT EXISTS pinpoint_diagnostic_reports (id BIGINT PRIMARY KEY, user_id BIGINT, username TEXT, category TEXT NOT NULL, message TEXT NOT NULL, app_version TEXT NOT NULL DEFAULT '', platform TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'open', created_at TIMESTAMPTZ NOT NULL DEFAULT now())",
            "CREATE INDEX IF NOT EXISTS idx_diagnostic_reports_created ON pinpoint_diagnostic_reports(created_at DESC)",
            "CREATE INDEX IF NOT EXISTS idx_diagnostic_reports_status ON pinpoint_diagnostic_reports(status)",
            "CREATE TABLE IF NOT EXISTS pinpoint_admin_audit (id BIGINT PRIMARY KEY, actor_id BIGINT NOT NULL, actor_username TEXT NOT NULL, action TEXT NOT NULL, target TEXT NOT NULL, detail TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now())",
            "CREATE INDEX IF NOT EXISTS idx_admin_audit_created ON pinpoint_admin_audit(created_at DESC)",
            "ALTER TABLE pinpoint_counters ADD COLUMN IF NOT EXISTS next_report_id BIGINT DEFAULT 1",
            "ALTER TABLE pinpoint_counters ADD COLUMN IF NOT EXISTS next_audit_id BIGINT DEFAULT 1",
        ],
    ),
    (
        16,
        "daily_trail_and_place_notes",
        &[
            "CREATE TABLE IF NOT EXISTS pinpoint_location_history (id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL, latitude DOUBLE PRECISION NOT NULL, longitude DOUBLE PRECISION NOT NULL, accuracy DOUBLE PRECISION, recorded_at TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS idx_location_history_user_time ON pinpoint_location_history(user_id, recorded_at DESC)",
            "CREATE TABLE IF NOT EXISTS pinpoint_place_notes (id BIGSERIAL PRIMARY KEY, user_id BIGINT NOT NULL, latitude DOUBLE PRECISION NOT NULL, longitude DOUBLE PRECISION NOT NULL, label TEXT NOT NULL DEFAULT '', message TEXT NOT NULL, photo TEXT, created_at TEXT NOT NULL)",
            "CREATE INDEX IF NOT EXISTS idx_place_notes_user_time ON pinpoint_place_notes(user_id, created_at DESC)",
        ],
    ),
    (
        17,
        "e2e_messages",
        &[
            "CREATE TABLE IF NOT EXISTS pinpoint_message_keys (user_id BIGINT PRIMARY KEY, bundle TEXT NOT NULL, updated_at TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS pinpoint_messages (id BIGSERIAL PRIMARY KEY, sender_id BIGINT NOT NULL, recipient_id BIGINT NOT NULL, ephemeral_public_key TEXT NOT NULL, salt_to TEXT NOT NULL, nonce_to TEXT NOT NULL, ciphertext_to TEXT NOT NULL, salt_from TEXT NOT NULL, nonce_from TEXT NOT NULL, ciphertext_from TEXT NOT NULL, signature TEXT NOT NULL, created_at TEXT NOT NULL, read_at TEXT, UNIQUE(signature))",
            "CREATE INDEX IF NOT EXISTS idx_messages_pair_time ON pinpoint_messages(sender_id, recipient_id, created_at DESC)",
            "CREATE INDEX IF NOT EXISTS idx_messages_recipient_unread ON pinpoint_messages(recipient_id, read_at)",
        ],
    ),
    (
        18,
        "oauth_identity_subject",
        &[
            "CREATE UNIQUE INDEX IF NOT EXISTS pinpoint_users_identity_subject_key ON pinpoint_users (identity_subject) WHERE identity_subject IS NOT NULL",
        ],
    ),
];

/// Apply all pending migrations (idempotent, recorded in version table).
pub async fn apply_migrations(pool: &sqlx::PgPool) -> Vec<u32> {
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS pinpoint_schema_migrations (version INT PRIMARY KEY, name TEXT NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT now())",
    )
    .execute(pool)
    .await;
    let applied: Vec<(i32,)> = sqlx::query_as("SELECT version FROM pinpoint_schema_migrations")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let done: std::collections::HashSet<i32> = applied.into_iter().map(|(v,)| v).collect();
    let mut newly = Vec::new();
    for (version, name, statements) in MIGRATIONS {
        if done.contains(&(*version as i32)) {
            continue;
        }
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(_) => return newly,
        };
        let mut ok = true;
        for stmt in *statements {
            if let Err(error) = sqlx::query(stmt).execute(&mut *tx).await {
                eprintln!("migration {version} ({name}) failed at `{stmt}`: {error}");
                ok = false;
                break;
            }
        }
        if !ok {
            // End-state fallback (v10 only): an aborted transaction cannot
            // run further queries, so roll back first, then verify the
            // desired state outside the transaction. Older DBs created the
            // MFA columns via legacy DDL; the least-privilege service roles
            // cannot ALTER, but the columns still exist — record as applied.
            let _ = tx.rollback().await;
            if *version != 10 {
                continue;
            }
            let cols: Vec<(i64,)> = sqlx::query_as(
                "SELECT count(*) FROM information_schema.columns WHERE table_name = 'pinpoint_users' AND column_name IN ('totp_secret','mfa_enabled','mfa_last_step')",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            ok = cols.first().map(|(c,)| *c == 3).unwrap_or(false);
            if !ok {
                continue;
            }
            let _ = sqlx::query(
                "INSERT INTO pinpoint_schema_migrations (version, name) VALUES ($1, $2) ON CONFLICT (version) DO NOTHING",
            )
            .bind(*version as i32)
            .bind(*name)
            .execute(pool)
            .await;
            newly.push(*version);
            continue;
        }
        if ok
            && sqlx::query(
                "INSERT INTO pinpoint_schema_migrations (version, name) VALUES ($1, $2) ON CONFLICT (version) DO NOTHING",
            )
            .bind(*version as i32)
            .bind(*name)
            .execute(&mut *tx)
            .await
            .is_ok()
        {
            newly.push(*version);
        }
        let _ = if ok {
            tx.commit().await
        } else {
            tx.rollback().await
        };
    }
    newly
}

#[cfg(test)]
mod tests {
    use super::MIGRATIONS;

    #[test]
    fn observability_migration_is_ordered_and_complete() {
        let (version, _name, statements) = MIGRATIONS
            .iter()
            .find(|(_, name, _)| *name == "observability")
            .expect("migration 15 must exist");
        assert_eq!(*version, 15);
        let sql = statements.join("\n");
        for needle in [
            "pinpoint_diagnostic_reports",
            "pinpoint_admin_audit",
            "created_at",
            "next_report_id",
        ] {
            assert!(sql.contains(needle), "missing {needle}");
        }
    }

    #[test]
    fn migration_versions_are_unique_and_sequential() {
        for (index, (version, _, _)) in MIGRATIONS.iter().enumerate() {
            assert_eq!(
                *version as usize,
                index + 1,
                "migration versions must be 1..N"
            );
        }
    }
}

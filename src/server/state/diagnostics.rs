//! Diagnostics reports + append-only admin audit persistence.
//!
//! Reports are user-initiated and deliberately PII-free: category, message,
//! app version/platform, and the public handle of a signed-in reporter. The
//! admin audit trail stores ids/handles, never emails or coordinates.
//!
//! Both tables are written through direct single-row SQL (like
//! `sql_insert_user`) so the diag and admin services never rewrite each
//! other's rows during the snapshot tick. Without Postgres the in-memory
//! store is authoritative.

use super::{AppState, Store};
use crate::models::{AdminAuditEntry, DiagnosticReport};
use chrono::{SecondsFormat, Utc};

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

impl AppState {
    pub(super) async fn load_diagnostics(&self, store: &mut Store) {
        let Some(pool) = &self.pg else {
            return;
        };
        let reports: Vec<(
            i64,
            Option<i64>,
            Option<String>,
            String,
            String,
            String,
            String,
            String,
            String,
        )> = sqlx::query_as(
            "SELECT id, user_id, username, category, message, COALESCE(app_version,''), COALESCE(platform,''), status, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM pinpoint_diagnostic_reports ORDER BY id DESC LIMIT 1000",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (id, user_id, username, category, message, app_version, platform, status, created_at) in
            reports
        {
            store.next_report_id = store.next_report_id.max(id + 1);
            store.diagnostic_reports.push(DiagnosticReport {
                id,
                user_id,
                username,
                category,
                message,
                app_version,
                platform,
                status,
                created_at,
            });
        }

        let audits: Vec<(i64, i64, String, String, String, Option<String>, String)> =
            sqlx::query_as(
                "SELECT id, actor_id, actor_username, action, target, detail, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM pinpoint_admin_audit ORDER BY id DESC LIMIT 2000",
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        for (id, actor_id, actor_username, action, target, detail, created_at) in audits {
            store.next_audit_id = store.next_audit_id.max(id + 1);
            store.admin_audit.push(AdminAuditEntry {
                id,
                actor_id,
                actor_username,
                action,
                target,
                detail,
                created_at,
            });
        }
    }

    async fn next_sequence_id(&self, sequence: &str) -> Option<i64> {
        let pool = self.pg.as_ref()?;
        sqlx::query_scalar::<_, i64>("SELECT nextval($1::regclass)")
            .bind(sequence)
            .fetch_one(pool)
            .await
            .ok()
    }

    /// Insert a report and return the stored row. Falls back to the in-memory
    /// counter when Postgres is absent or the sequence is not granted.
    pub async fn record_report(
        &self,
        user_id: Option<i64>,
        username: Option<String>,
        category: &str,
        message: &str,
        app_version: &str,
        platform: &str,
    ) -> DiagnosticReport {
        let id = match self.next_sequence_id("pinpoint_report_id_seq").await {
            Some(id) => id,
            None => {
                let mut store = self.store.lock().unwrap();
                let id = store.next_report_id.max(1);
                store.next_report_id = id + 1;
                id
            }
        };
        let report = DiagnosticReport {
            id,
            user_id,
            username,
            category: category.to_string(),
            message: message.to_string(),
            app_version: app_version.to_string(),
            platform: platform.to_string(),
            status: "open".to_string(),
            created_at: now_iso(),
        };
        if let Some(pool) = &self.pg {
            let _ = sqlx::query(
                "INSERT INTO pinpoint_diagnostic_reports (id, user_id, username, category, message, app_version, platform, status, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,now()) ON CONFLICT (id) DO NOTHING",
            )
            .bind(report.id)
            .bind(report.user_id)
            .bind(&report.username)
            .bind(&report.category)
            .bind(&report.message)
            .bind(&report.app_version)
            .bind(&report.platform)
            .bind(&report.status)
            .execute(pool)
            .await;
        }
        let mut store = self.store.lock().unwrap();
        store.next_report_id = store.next_report_id.max(id + 1);
        store.diagnostic_reports.push(report.clone());
        if store.diagnostic_reports.len() > 1000 {
            store.diagnostic_reports.sort_by_key(|r| r.id);
            let excess = store.diagnostic_reports.len() - 1000;
            store.diagnostic_reports.drain(0..excess);
        }
        report
    }

    /// Append an accountability record. There is intentionally no update or
    /// delete path for audit rows.
    pub async fn record_audit(
        &self,
        actor_id: i64,
        actor_username: &str,
        action: &str,
        target: &str,
        detail: Option<&str>,
    ) -> AdminAuditEntry {
        let id = match self.next_sequence_id("pinpoint_audit_id_seq").await {
            Some(id) => id,
            None => {
                let mut store = self.store.lock().unwrap();
                let id = store.next_audit_id.max(1);
                store.next_audit_id = id + 1;
                id
            }
        };
        let entry = AdminAuditEntry {
            id,
            actor_id,
            actor_username: actor_username.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            detail: detail.map(str::to_string),
            created_at: now_iso(),
        };
        if let Some(pool) = &self.pg {
            let _ = sqlx::query(
                "INSERT INTO pinpoint_admin_audit (id, actor_id, actor_username, action, target, detail, created_at) VALUES ($1,$2,$3,$4,$5,$6,now()) ON CONFLICT (id) DO NOTHING",
            )
            .bind(entry.id)
            .bind(entry.actor_id)
            .bind(&entry.actor_username)
            .bind(&entry.action)
            .bind(&entry.target)
            .bind(&entry.detail)
            .execute(pool)
            .await;
        }
        let mut store = self.store.lock().unwrap();
        store.next_audit_id = store.next_audit_id.max(id + 1);
        store.admin_audit.push(entry.clone());
        if store.admin_audit.len() > 5000 {
            store.admin_audit.sort_by_key(|e| e.id);
            let excess = store.admin_audit.len() - 5000;
            store.admin_audit.drain(0..excess);
        }
        entry
    }

    /// Update triage status; returns false when the report is unknown.
    pub async fn set_report_status(&self, id: i64, status: &str) -> bool {
        if let Some(pool) = &self.pg {
            let _ = sqlx::query("UPDATE pinpoint_diagnostic_reports SET status=$1 WHERE id=$2")
                .bind(status)
                .bind(id)
                .execute(pool)
                .await;
        }
        let mut store = self.store.lock().unwrap();
        match store.diagnostic_reports.iter_mut().find(|r| r.id == id) {
            Some(report) => {
                report.status = status.to_string();
                true
            }
            None => false,
        }
    }
}

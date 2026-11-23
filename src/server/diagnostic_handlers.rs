//! Diagnostics: user-initiated report ingestion + admin triage.
//!
//! Ingestion is deliberately minimal and privacy-preserving. No IP address,
//! user agent, coordinates or device identifiers are read or stored; the only
//! account data is the reporter's numeric id and public handle when signed in.
//!
//! The dedicated `pinpoint-diag` binary mounts the ingestion router on 8005;
//! the combined server also serves it so the web client can report on the
//! same origin. Admin listing/triage lives in `admin_router()` (8006).

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::admin_handlers::require_admin;
use super::handlers::not_found;
use super::state::AppState;
use super::usernames::username_for;
use super::{error_envelope, CurrentUser, IntoResponse};

const CATEGORIES: &[&str] = &["crash", "map", "routing", "connectivity", "other"];
const STATUSES: &[&str] = &["open", "triaged", "closed"];
const MAX_MESSAGE_CHARS: usize = 2000;

/// Version strings are displayed in the admin diagnostics table; keep them
/// short and free of display controls, falling back to this crate's version.
fn valid_app_version(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 40
        && !crate::security::validation::contains_display_control(value)
}

fn report_json(r: &crate::models::DiagnosticReport) -> Value {
    json!({
        "id": r.id,
        "user_id": r.user_id,
        "username": r.username,
        "reporter": r.username.as_deref().unwrap_or("anonymous"),
        "category": r.category,
        "message": r.message,
        "app_version": r.app_version,
        "platform": r.platform,
        "status": r.status,
        "created_at": r.created_at,
    })
}

fn audit_json(e: &crate::models::AdminAuditEntry) -> Value {
    json!({
        "id": e.id,
        "actor_id": e.actor_id,
        "actor_username": e.actor_username,
        "action": e.action,
        "target": e.target,
        "detail": e.detail,
        "created_at": e.created_at,
    })
}

/// `POST /api/v1/diagnostics/report` — signed-in or anonymous.
pub async fn submit_report(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let category = body
        .get("category")
        .and_then(Value::as_str)
        .unwrap_or("other")
        .trim()
        .to_ascii_lowercase();
    if !CATEGORIES.contains(&category.as_str()) {
        return error_envelope(
            "VALIDATION_FAILED",
            "category must be crash, map, routing, connectivity, or other",
        );
    }
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if message.is_empty() {
        return error_envelope("VALIDATION_FAILED", "message is required");
    }
    if message.chars().count() > MAX_MESSAGE_CHARS {
        return error_envelope(
            "VALIDATION_FAILED",
            "message must be 2000 characters or fewer",
        );
    }
    if message
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return error_envelope("VALIDATION_FAILED", "message contains control characters");
    }
    if crate::security::validation::contains_bidi_control(message) {
        return error_envelope("VALIDATION_FAILED", "message contains invalid characters");
    }
    let platform = body
        .get("platform")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase();
    if platform.len() > 40
        || !platform
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
    {
        return error_envelope("VALIDATION_FAILED", "invalid platform");
    }
    let app_version = body
        .get("app_version")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| valid_app_version(v))
        .unwrap_or(env!("CARGO_PKG_VERSION"));

    let (user_id, username) = match cu {
        Some(Extension(cu)) => {
            let store = state.store.lock().unwrap();
            match store.find_user_by_id(cu.user_id) {
                Some(user) => (Some(user.id), Some(username_for(user.id))),
                None => (None, None),
            }
        }
        None => (None, None),
    };

    let report = state
        .record_report(
            user_id,
            username,
            &category,
            message,
            app_version,
            &platform,
        )
        .await;
    (
        StatusCode::CREATED,
        Json(json!({
            "id": report.id,
            "status": report.status,
            "created_at": report.created_at,
        })),
    )
        .into_response()
}

/// `GET /api/v1/admin/diagnostics?status=&limit=` — newest first.
pub async fn list_reports(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&state, cu) {
        return resp;
    }
    let status = q
        .get("status")
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| STATUSES.contains(&s.as_str()));
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100)
        .clamp(1, 500);
    state.refresh_from_db().await;
    let store = state.store.lock().unwrap();
    let mut reports: Vec<&crate::models::DiagnosticReport> = store
        .diagnostic_reports
        .iter()
        .filter(|r| status.as_deref().is_none_or(|s| r.status == s))
        .collect();
    reports.sort_by_key(|r| std::cmp::Reverse(r.id));
    let total = reports.len();
    let open = store
        .diagnostic_reports
        .iter()
        .filter(|r| r.status == "open")
        .count();
    let items: Vec<Value> = reports.into_iter().take(limit).map(report_json).collect();
    (
        StatusCode::OK,
        Json(json!({"reports": items, "total": total, "open": open})),
    )
        .into_response()
}

/// `PATCH /api/v1/admin/diagnostics/:id` — triage status.
pub async fn update_report(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let admin = match require_admin(&state, cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if !STATUSES.contains(&status.as_str()) {
        return error_envelope(
            "VALIDATION_FAILED",
            "status must be open, triaged, or closed",
        );
    }
    state.refresh_from_db().await;
    let previous = {
        let store = state.store.lock().unwrap();
        store
            .diagnostic_reports
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.status.clone())
    };
    let Some(previous) = previous else {
        return not_found("REPORT_NOT_FOUND", "Report not found.");
    };
    if !state.set_report_status(id, &status).await {
        return not_found("REPORT_NOT_FOUND", "Report not found.");
    }
    if previous != status {
        super::admin_handlers::record_admin_action(
            &state,
            admin.user_id,
            "diagnostic.triage",
            &format!("report:{id}"),
            Some(&format!("{previous} -> {status}")),
        )
        .await;
    }
    (StatusCode::OK, Json(json!({"id": id, "status": status}))).into_response()
}

/// `GET /api/v1/admin/audit?limit=` — append-only accountability trail.
pub async fn list_audit(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if let Err(resp) = require_admin(&state, cu) {
        return resp;
    }
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(200)
        .clamp(1, 1000);
    state.refresh_from_db().await;
    let store = state.store.lock().unwrap();
    let mut entries: Vec<&crate::models::AdminAuditEntry> = store.admin_audit.iter().collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.id));
    let total = entries.len();
    let items: Vec<Value> = entries.into_iter().take(limit).map(audit_json).collect();
    (
        StatusCode::OK,
        Json(json!({"entries": items, "total": total})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_categories_and_statuses_are_closed_sets() {
        assert!(CATEGORIES.contains(&"map"));
        assert!(!CATEGORIES.contains(&"tracking"));
        assert!(STATUSES.contains(&"triaged"));
        assert!(!STATUSES.contains(&"deleted"));
    }

    #[test]
    fn public_report_shape_has_no_email_or_coordinates() {
        let report = crate::models::DiagnosticReport {
            id: 1,
            user_id: Some(2),
            username: Some("bright-badger-2".into()),
            category: "map".into(),
            message: "Tiles failed to load".into(),
            app_version: "1.2.0".into(),
            platform: "web".into(),
            status: "open".into(),
            created_at: "2026-11-13T12:00:00Z".into(),
        };
        let value = report_json(&report);
        assert_eq!(value["reporter"], "bright-badger-2");
        assert!(value.get("email").is_none());
        assert!(value.get("latitude").is_none());
        assert!(value.get("ip").is_none());
    }

    #[test]
    fn app_version_rejects_display_controls_and_overlength() {
        assert!(valid_app_version("2.6.1"));
        assert!(valid_app_version("Pinpoint Android 2.6.1"));
        assert!(!valid_app_version(""));
        assert!(!valid_app_version(&"x".repeat(41)));
        assert!(!valid_app_version("2.6\u{202e}1"));
        assert!(!valid_app_version("2.6\t1"));
    }
}

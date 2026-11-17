//! Locations + shares service handlers.
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use super::handlers::{require_current_user, share_json};
use super::state::{AppState, Store};
use super::{error_envelope, CurrentUser, IntoResponse};
use crate::models::LocationShare;

/// The active share that lets `viewer` see `owner`'s latest location.
///
/// Consent is fail-closed: an accepted friendship alone grants nothing. The
/// location owner must have an active, non-expired `to_friend`/`mutual` share
/// for that friendship.
fn visible_share(store: &Store, viewer: i64, owner: i64) -> Option<&LocationShare> {
    let friendship = store.friendships.iter().find(|f| {
        f.status == "accepted"
            && ((f.requester_id == viewer && f.receiver_id == owner)
                || (f.requester_id == owner && f.receiver_id == viewer))
    })?;
    let now = chrono::Utc::now();
    store.shares.iter().find(|s| {
        s.friendship_id == friendship.id
            && s.owner_id == owner
            && s.is_active
            && s.direction != "from_friend"
            && s.expires_at.as_deref().is_none_or(|raw| {
                chrono::DateTime::parse_from_rfc3339(raw)
                    .map(|expiry| expiry.with_timezone(&chrono::Utc) > now)
                    .unwrap_or(false)
            })
    })
}

fn round_for_precision(lat: f64, lon: f64, precision: &str) -> (f64, f64) {
    match precision {
        "approximate" => ((lat * 100.0).round() / 100.0, (lon * 100.0).round() / 100.0),
        "city_only" => ((lat * 10.0).round() / 10.0, (lon * 10.0).round() / 10.0),
        _ => (lat, lon),
    }
}

/// `POST /locations/shares`.
pub async fn create_share(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let friendship_id = body
        .get("friendship_id")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let direction = body
        .get("direction")
        .and_then(Value::as_str)
        .unwrap_or("to_friend")
        .to_string();
    let precision = body
        .get("precision_level")
        .and_then(Value::as_str)
        .unwrap_or("exact")
        .to_string();
    // Trust-boundary validation: an unknown precision must never fall through
    // to exact coordinates, and an unknown direction must not create a share.
    if !["exact", "approximate", "city_only"].contains(&precision.as_str()) {
        return error_envelope(
            "VALIDATION_FAILED",
            "precision_level must be exact, approximate, or city_only",
        );
    }
    if !["to_friend", "from_friend", "mutual"].contains(&direction.as_str()) {
        return error_envelope(
            "VALIDATION_FAILED",
            "direction must be to_friend, from_friend, or mutual",
        );
    }
    // Optional expiry: duration_hours (int) or explicit expires_at ISO-8601.
    // 0 means "no expiry".
    let expires_at: Option<String> =
        if let Some(h) = body.get("duration_hours").and_then(Value::as_i64) {
            if h < 0 {
                return error_envelope("VALIDATION_FAILED", "duration_hours must be >= 0");
            }
            if h == 0 {
                None
            } else {
                Some((chrono::Utc::now() + chrono::Duration::hours(h)).to_rfc3339())
            }
        } else {
            body.get("expires_at")
                .and_then(Value::as_str)
                .map(str::to_string)
        };
    let label = body
        .get("label")
        .and_then(Value::as_str)
        .map(str::to_string);
    if friendship_id <= 0 {
        return error_envelope("VALIDATION_FAILED", "friendship_id is required");
    }
    let mut store = state.store.lock().unwrap();
    if !store.friendships.iter().any(|f| {
        f.id == friendship_id
            && f.status == "accepted"
            && (f.requester_id == cu.user_id || f.receiver_id == cu.user_id)
    }) {
        return error_envelope(
            "FRIENDSHIP_NOT_FOUND",
            "Friendship not found or not accepted.",
        );
    }
    // Per-friend control is an upsert: one share per friendship + direction.
    if let Some(existing) = store.shares.iter_mut().find(|s| {
        s.owner_id == cu.user_id && s.friendship_id == friendship_id && s.direction == direction
    }) {
        existing.precision_level = precision;
        existing.expires_at = expires_at;
        existing.is_active = true;
        if label.is_some() {
            existing.label = label;
        }
        let view = share_json(existing);
        return (StatusCode::OK, Json(view)).into_response();
    }
    let id = store.next_share_id;
    store.next_share_id += 1;
    let share = crate::models::LocationShare {
        id,
        friendship_id,
        owner_id: cu.user_id,
        direction: direction.clone(),
        precision_level: precision.clone(),
        is_active: true,
        last_location_update_at: None,
        expires_at,
        label,
    };
    store.shares.push(share.clone());
    (StatusCode::CREATED, Json(share_json(&share))).into_response()
}

/// `GET /locations/shares`.
pub async fn list_shares(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let store = state.store.lock().unwrap();
    let shares: Vec<Value> = store
        .shares
        .iter()
        .filter(|s| s.owner_id == cu.user_id)
        .map(share_json)
        .collect();
    (StatusCode::OK, Json(shares)).into_response()
}

/// `PATCH /locations/shares/{id}`.
pub async fn update_share(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let mut store = state.store.lock().unwrap();
    let share = match store.shares.iter_mut().find(|s| s.id == id) {
        Some(s) if s.owner_id == cu.user_id => s,
        Some(_) => return error_envelope("SHARE_NOT_FOUND", "Share not found."),
        None => return error_envelope("SHARE_NOT_FOUND", "Share not found."),
    };
    if let Some(active) = body.get("is_active").and_then(Value::as_bool) {
        share.is_active = active;
    }
    if let Some(p) = body.get("precision_level").and_then(Value::as_str) {
        if !["exact", "approximate", "city_only"].contains(&p) {
            return error_envelope(
                "VALIDATION_FAILED",
                "precision_level must be exact, approximate, or city_only",
            );
        }
        share.precision_level = p.to_string();
    }
    if let Some(d) = body.get("direction").and_then(Value::as_str) {
        if !["to_friend", "from_friend", "mutual"].contains(&d) {
            return error_envelope(
                "VALIDATION_FAILED",
                "direction must be to_friend, from_friend, or mutual",
            );
        }
        share.direction = d.to_string();
    }
    if let Some(l) = body.get("label").and_then(Value::as_str) {
        share.label = Some(l.to_string());
    }
    if let Some(h) = body.get("duration_hours").and_then(Value::as_i64) {
        if h < 0 {
            return error_envelope("VALIDATION_FAILED", "duration_hours must be >= 0");
        }
        share.expires_at = if h == 0 {
            None
        } else {
            Some((chrono::Utc::now() + chrono::Duration::hours(h)).to_rfc3339())
        };
    }
    if body.get("expires_at").is_some() {
        share.expires_at = body
            .get("expires_at")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    (StatusCode::OK, Json(share_json(share))).into_response()
}

/// `DELETE /locations/shares/{id}` — revoke a share permanently.
pub async fn delete_share(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    {
        let mut store = state.store.lock().unwrap();
        let pos = match store.shares.iter().position(|s| s.id == id) {
            Some(p) => p,
            None => return error_envelope("SHARE_NOT_FOUND", "Share not found."),
        };
        if store.shares[pos].owner_id != cu.user_id {
            return error_envelope("SHARE_NOT_FOUND", "Share not found.");
        }
        store.shares.remove(pos);
    }
    state.sql_delete_share(id).await;
    (StatusCode::OK, Json(json!({"message": "Share deleted."}))).into_response()
}

/// `POST /locations/me` — publish current location.
pub async fn publish_location(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let lat = body.get("latitude").and_then(Value::as_f64).unwrap_or(0.0);
    let lon = body.get("longitude").and_then(Value::as_f64).unwrap_or(0.0);
    let accuracy = body.get("accuracy").and_then(Value::as_f64);
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return error_envelope(
            "VALIDATION_FAILED",
            "latitude must be -90..90, longitude -180..180",
        );
    }
    let mut store = state.store.lock().unwrap();
    let user_id = cu.user_id;
    let user_name = store
        .find_user_by_id(user_id)
        .map(|u| u.full_name.clone())
        .unwrap_or_default();
    if let Some(loc) = store.locations.iter_mut().find(|l| l.user_id == user_id) {
        loc.latitude = lat;
        loc.longitude = lon;
        loc.accuracy = accuracy;
        loc.updated_at = chrono::Utc::now().to_rfc3339();
    } else {
        store.locations.push(crate::models::FriendLocation {
            user_id,
            user_name,
            latitude: lat,
            longitude: lon,
            precision_level: "exact".into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            accuracy,
        });
    }
    let shares_updated = store.shares.iter().filter(|s| s.is_active).count();
    (
        StatusCode::OK,
        Json(json!({"message": "Location updated.", "shares_updated": shares_updated})),
    )
        .into_response()
}

/// `GET /locations/friends` — locations visible to the caller.
/// Consent model: only an accepted friendship plus an active, non-expired
/// `to_friend`/`mutual` share owned by the location owner. Fail closed.
pub async fn list_friend_locations(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let store = state.store.lock().unwrap();
    let me = cu.user_id;
    let locations: Vec<Value> = store
        .locations
        .iter()
        .filter(|l| l.user_id != me)
        .filter_map(|l| {
            let share = visible_share(&store, me, l.user_id)?;
            let (lat, lon) = round_for_precision(l.latitude, l.longitude, &share.precision_level);
            Some(json!({
                "user_id": l.user_id,
                "user_name": l.user_name,
                "latitude": lat,
                "longitude": lon,
                "accuracy": l.accuracy,
                "updated_at": l.updated_at,
                "precision_level": share.precision_level,
            }))
        })
        .collect();
    (StatusCode::OK, Json(locations)).into_response()
}

/// `GET /locations/me` — my own latest published location (or 404).
pub async fn get_my_location(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let store = state.store.lock().unwrap();
    match store.locations.iter().find(|l| l.user_id == cu.user_id) {
        Some(l) => (
            StatusCode::OK,
            Json(json!({
                "user_id": l.user_id, "user_name": l.user_name,
                "latitude": l.latitude, "longitude": l.longitude,
                "accuracy": l.accuracy, "updated_at": l.updated_at,
                "precision_level": l.precision_level,
            })),
        )
            .into_response(),
        None => error_envelope("LOCATION_NOT_FOUND", "No location published yet."),
    }
}

/// `POST /locations/check-ins` — create a check-in (manual/scheduled/trip).
pub async fn create_check_in(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let lat = body.get("latitude").and_then(Value::as_f64).unwrap_or(0.0);
    let lon = body.get("longitude").and_then(Value::as_f64).unwrap_or(0.0);
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return error_envelope(
            "VALIDATION_FAILED",
            "latitude must be -90..90, longitude -180..180",
        );
    }
    let note = body
        .get("note")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let kind = body
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("manual")
        .to_string();
    if !["manual", "scheduled", "trip"].contains(&kind.as_str()) {
        return error_envelope(
            "VALIDATION_FAILED",
            "kind must be manual, scheduled, or trip",
        );
    }
    let mut store = state.store.lock().unwrap();
    let me = match store.find_user_by_id(cu.user_id).cloned() {
        Some(u) => u,
        None => return error_envelope("USER_NOT_FOUND", "User not found."),
    };
    let id = store.next_check_in_id;
    store.next_check_in_id += 1;
    let ci = crate::models::CheckIn {
        id,
        user_id: me.id,
        user_name: me.full_name.clone(),
        latitude: lat,
        longitude: lon,
        note,
        kind,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    store.check_ins.push(ci.clone());
    (
        StatusCode::CREATED,
        Json(json!({
            "id": ci.id, "user_id": ci.user_id, "user_name": ci.user_name,
            "latitude": ci.latitude, "longitude": ci.longitude,
            "note": ci.note, "kind": ci.kind, "created_at": ci.created_at,
        })),
    )
        .into_response()
}

/// `GET /locations/check-ins` — all check-ins (newest first).
pub async fn list_check_ins(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.store.lock().unwrap();
    let mut items: Vec<Value> = store
        .check_ins
        .iter()
        .map(|c| {
            json!({
                "id": c.id, "user_id": c.user_id, "user_name": c.user_name,
                "latitude": c.latitude, "longitude": c.longitude,
                "note": c.note, "kind": c.kind, "created_at": c.created_at,
            })
        })
        .collect();
    items.reverse();
    (StatusCode::OK, Json(items)).into_response()
}

/// `DELETE /locations/check-ins/{id}`.
pub async fn delete_check_in(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    {
        let mut store = state.store.lock().unwrap();
        let pos = match store.check_ins.iter().position(|c| c.id == id) {
            Some(p) => p,
            None => return error_envelope("CHECK_IN_NOT_FOUND", "Check-in not found."),
        };
        store.check_ins.remove(pos);
    }
    state.sql_delete_check_in(id).await;
    (
        StatusCode::OK,
        Json(json!({"message": "Check-in deleted."})),
    )
        .into_response()
}

/// `POST /locations/emergency` — panic share: records an alert + activates all shares.
pub async fn trigger_emergency(
    State(state): State<AppState>,
    cu: Option<Extension<CurrentUser>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let cu = match require_current_user(cu) {
        Ok(cu) => cu,
        Err(resp) => return resp,
    };
    let lat = body.get("latitude").and_then(Value::as_f64).unwrap_or(0.0);
    let lon = body.get("longitude").and_then(Value::as_f64).unwrap_or(0.0);
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return error_envelope(
            "VALIDATION_FAILED",
            "latitude must be -90..90, longitude -180..180",
        );
    }
    let mut store = state.store.lock().unwrap();
    let me = match store.find_user_by_id(cu.user_id).cloned() {
        Some(u) => u,
        None => return error_envelope("USER_NOT_FOUND", "User not found."),
    };
    // Publish the emergency point as the latest location + activate shares.
    if let Some(loc) = store.locations.iter_mut().find(|l| l.user_id == me.id) {
        loc.latitude = lat;
        loc.longitude = lon;
        loc.updated_at = chrono::Utc::now().to_rfc3339();
    } else {
        store.locations.push(crate::models::FriendLocation {
            user_id: me.id,
            user_name: me.full_name.clone(),
            latitude: lat,
            longitude: lon,
            precision_level: "exact".into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            accuracy: None,
        });
    }
    for s in store.shares.iter_mut() {
        if s.owner_id == me.id {
            s.is_active = true;
        }
    }
    let id = store.next_emergency_id;
    store.next_emergency_id += 1;
    let alert = crate::models::EmergencyAlert {
        id,
        user_id: me.id,
        user_name: me.full_name.clone(),
        latitude: lat,
        longitude: lon,
        created_at: chrono::Utc::now().to_rfc3339(),
        resolved: false,
    };
    store.emergencies.push(alert.clone());
    (
        StatusCode::CREATED,
        Json(json!({
            "id": alert.id, "user_id": alert.user_id, "user_name": alert.user_name,
            "latitude": alert.latitude, "longitude": alert.longitude,
            "created_at": alert.created_at, "resolved": alert.resolved,
        })),
    )
        .into_response()
}

/// `GET /locations/emergency` — all emergency alerts (newest first).
pub async fn list_emergencies(State(state): State<AppState>) -> impl IntoResponse {
    let store = state.store.lock().unwrap();
    let mut items: Vec<Value> = store
        .emergencies
        .iter()
        .map(|e| {
            json!({
                "id": e.id, "user_id": e.user_id, "user_name": e.user_name,
                "latitude": e.latitude, "longitude": e.longitude,
                "created_at": e.created_at, "resolved": e.resolved,
            })
        })
        .collect();
    items.reverse();
    (StatusCode::OK, Json(items)).into_response()
}

/// `PATCH /locations/emergency/{id}` — resolve / re-open an alert.
pub async fn resolve_emergency(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let resolved = body
        .get("resolved")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mut store = state.store.lock().unwrap();
    let alert = match store.emergencies.iter_mut().find(|e| e.id == id) {
        Some(e) => e,
        None => return error_envelope("EMERGENCY_NOT_FOUND", "Emergency alert not found."),
    };
    alert.resolved = resolved;
    (
        StatusCode::OK,
        Json(json!({
            "id": alert.id, "user_id": alert.user_id, "user_name": alert.user_name,
            "latitude": alert.latitude, "longitude": alert.longitude,
            "created_at": alert.created_at, "resolved": alert.resolved,
        })),
    )
        .into_response()
}

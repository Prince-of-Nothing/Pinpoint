//! Pure-Rust API server for Pinpoint (replaces the archived Go backend).
//!
//! Implements the `/api/v1` wire contract consumed by the Rust client
//! (`src/api.rs`) and the archived web/mobile clients:
//! - `POST /api/v1/auth/login` → `{csrf_token, user}` + `HttpOnly` session cookie
//! - `POST /api/v1/auth/register`, `/auth/logout`, `/auth/session`
//! - `POST /api/v1/auth/password/change`, `/auth/password/force-change`
//! - `GET /api/v1/auth/me`
//! - `GET /api/v1/users/`, `GET /api/v1/users/{id}`, `PATCH /api/v1/users/{id}`, admin `POST /users/`
//! - `POST /api/v1/friendships/`, `GET /api/v1/friendships/`, `POST /friendships/{id}/respond`
//! - `POST /locations/shares`, `GET /locations/shares`, `PATCH /locations/shares/{id}`
//! - `POST /locations/me`, `GET /locations/friends`
//! - `GET /health`
//!
//! In-memory store (demo/tests/local). For production, swap `Store` for
//! Postgres behind the same handlers. All validation/consent/precision
//! rules mirror the Go handlers exactly.
//!
//! Run: `cargo run --bin pinpoint-server` (listens on 127.0.0.1:8000).

mod admin_handlers;
mod auth_handlers;
mod diagnostic_handlers;
mod handlers;
mod locations_handlers;
mod migrations;
mod state;
mod usernames;
mod users_handlers;
mod web_handlers;

pub use handlers::health;
pub use handlers::sync_status;
pub use state::{AppState, ServiceKind, Store, StoredSession};

pub use crate::models::{FriendLocation, Friendship, LocationShare, User};

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde_json::json;

/// Authenticated session extracted from the `session_id` cookie.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user_id: i64,
    pub session_id: String,
    pub csrf_token: String,
    pub mfa_pending: bool,
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|pair| {
            let (k, v) = pair.trim().split_once('=')?;
            if k == name {
                Some(v.to_string())
            } else {
                None
            }
        })
}

/// Validate the session cookie (and CSRF on unsafe methods) and inject
/// [`CurrentUser`] into request extensions. Unauthenticated requests pass
/// through without the extension; handlers that require auth fail closed.
pub async fn session_auth(
    State(state): State<AppState>,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    if let Some(session_id) = cookie_value(req.headers(), "session_id") {
        if let Some(session) = state.get_session(&session_id).await {
            let unsafe_method = !matches!(
                *req.method(),
                axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
            );
            if unsafe_method {
                let csrf = req
                    .headers()
                    .get("X-CSRF-Token")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if csrf != session.csrf_token {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({"error": {"code": "CSRF_FAILED", "message": "Invalid CSRF token."}})),
                    )
                        .into_response();
                }
            }
            req.extensions_mut().insert(CurrentUser {
                user_id: session.user_id,
                session_id: session.session_id.clone(),
                csrf_token: session.csrf_token.clone(),
                mfa_pending: session.mfa_pending,
            });
            state.touch_session(&session.session_id).await;
        }
    }
    next.run(req).await
}

/// Privacy: logs never include query strings (search terms, coordinates).
fn loggable_uri(uri: &axum::http::Uri) -> &str {
    uri.path()
}

/// Per-request convergence for multi-replica determinism.
///
/// With 2 replicas, reading only the in-memory `Store` (5s tick) serves
/// stale data and breaks read-after-write (the e2e saw 29/38 at scale 2).
/// This middleware makes every request converge: refresh from Postgres
/// before the handler, and persist after mutating requests. In-memory
/// mode (no `DATABASE_URL`) and all unit/router tests are a no-op (both
/// calls early-return), so the suite stays green.
/// State is read from request extensions (not a `State` extractor) so this
/// layer composes on `Router<AppState>` before `with_state`.
pub async fn converge(
    State(state): State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    let method = req.method().clone();
    eprintln!("CONVERGE run {} {}", method, loggable_uri(req.uri()));
    state.refresh_from_db().await;
    let res = next.run(req).await;
    if method != axum::http::Method::GET {
        state.persist().await;
        eprintln!("CONVERGE persisted");
    }
    res
}

/// Build the full Axum router with all `/api/v1` routes + health.
pub fn router(state: AppState) -> Router {
    let app_state = state.clone();
    let session_state = state.clone();
    auth_router()
        .merge(users_router())
        .merge(locations_router())
        .merge(map_router())
        .merge(admin_router())
        .merge(diagnostic_router())
        .route("/", get(web_handlers::index))
        .route("/app.js", get(web_handlers::app_js))
        .route("/map.js", get(web_handlers::map_js))
        .route("/nav.js", get(web_handlers::nav_js))
        .route("/sharing.js", get(web_handlers::sharing_js))
        .route("/appearance.js", get(web_handlers::appearance_js))
        .route("/admin.js", get(web_handlers::admin_js))
        .route("/admin/overview.js", get(web_handlers::admin_overview_js))
        .route("/admin/users.js", get(web_handlers::admin_users_js))
        .route(
            "/admin/diagnostics.js",
            get(web_handlers::admin_diagnostics_js),
        )
        .route("/admin/audit.js", get(web_handlers::admin_audit_js))
        .route("/app.css", get(web_handlers::app_css))
        .route("/favicon.svg", get(web_handlers::favicon))
        .route("/favicon.ico", get(web_handlers::favicon))
        .route("/vendor/leaflet.js", get(web_handlers::leaflet_js))
        .route("/vendor/leaflet.css", get(web_handlers::leaflet_css))
        .route(
            "/vendor/leaflet/LICENSE",
            get(web_handlers::leaflet_license),
        )
        .route("/vendor/images/:file", get(web_handlers::leaflet_image))
        .route("/health", get(handlers::health))
        .route("/api/v1/sync-status", get(handlers::sync_status))
        .with_state(state)
        .layer(axum::middleware::from_fn_with_state(app_state, converge))
        .layer(axum::middleware::from_fn_with_state(
            session_state,
            session_auth,
        ))
}

/// Auth service routes (`/api/v1/auth/*`). Independently scalable.
/// Stateless: the app state is supplied once by the binary's `with_state`.
pub fn auth_router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/login", post(auth_handlers::login))
        .route("/api/v1/auth/register", post(auth_handlers::register))
        .route("/api/v1/auth/logout", post(auth_handlers::logout))
        .route("/api/v1/auth/logout-all", post(auth_handlers::logout_all))
        .route("/api/v1/auth/anonymous", post(auth_handlers::anonymous))
        .route(
            "/api/v1/auth/anonymous/recover",
            post(auth_handlers::anonymous_recover),
        )
        .route("/api/v1/auth/sessions", get(auth_handlers::list_sessions))
        .route(
            "/api/v1/auth/sessions/:id",
            delete(auth_handlers::revoke_session),
        )
        .route("/api/v1/auth/session", post(auth_handlers::session))
        .route("/api/v1/auth/me", get(auth_handlers::me))
        .route(
            "/api/v1/auth/password/change",
            post(auth_handlers::change_password),
        )
        .route(
            "/api/v1/auth/password/force-change",
            post(auth_handlers::force_change_password),
        )
        .route("/api/v1/auth/mfa/enroll", post(auth_handlers::mfa_enroll))
        .route("/api/v1/auth/mfa/verify", post(auth_handlers::mfa_verify))
        .route("/api/v1/auth/mfa/status", get(auth_handlers::mfa_status))
        .route("/api/v1/auth/mfa", delete(auth_handlers::mfa_disable))
}

/// Admin console routes (`/api/v1/admin/*`). Role-gated per handler so the
/// same router can later run on a dedicated admin binary/port.
pub fn admin_router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/overview", get(admin_handlers::overview))
        .route("/api/v1/admin/users", get(admin_handlers::list_users))
        .route(
            "/api/v1/admin/users/:id",
            patch(admin_handlers::update_user).delete(admin_handlers::delete_user),
        )
        .route(
            "/api/v1/admin/diagnostics",
            get(diagnostic_handlers::list_reports),
        )
        .route(
            "/api/v1/admin/diagnostics/:id",
            patch(diagnostic_handlers::update_report),
        )
        .route("/api/v1/admin/audit", get(diagnostic_handlers::list_audit))
}

/// Diagnostics ingestion routes. The dedicated `pinpoint-diag` binary mounts
/// this on port 8005; the combined server also merges it so the web client can
/// report on the same origin (same data when Postgres is configured).
pub fn diagnostic_router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/diagnostics/report",
        post(diagnostic_handlers::submit_report),
    )
}

/// Users + friendships service routes. Independently scalable.
/// NOTE: matchit 0.7 (axum 0.7) uses `:param` syntax, NOT `{param}` (that is axum 0.8).
pub fn users_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/users/",
            get(users_handlers::list_users).post(users_handlers::create_user),
        )
        .route(
            "/api/v1/users/:id",
            get(users_handlers::get_user)
                .patch(users_handlers::update_user)
                .delete(users_handlers::delete_user),
        )
        .route("/api/v1/users/lookup", get(users_handlers::lookup_user))
        .route("/api/v1/users/search", get(users_handlers::search_users))
        .route(
            "/api/v1/friendships/",
            get(users_handlers::list_friendships).post(users_handlers::create_friendship),
        )
        .route(
            "/api/v1/friendships/:id/respond",
            post(users_handlers::respond_friendship),
        )
        .route(
            "/api/v1/friendships/:id",
            delete(users_handlers::delete_friendship),
        )
}

/// Locations + shares service routes. Independently scalable.
pub fn locations_router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/locations/shares",
            get(locations_handlers::list_shares).post(locations_handlers::create_share),
        )
        .route(
            "/api/v1/locations/shares/:id",
            patch(locations_handlers::update_share).delete(locations_handlers::delete_share),
        )
        .route(
            "/api/v1/locations/me",
            post(locations_handlers::publish_location).get(locations_handlers::get_my_location),
        )
        .route(
            "/api/v1/locations/friends",
            get(locations_handlers::list_friend_locations),
        )
        .route(
            "/api/v1/locations/check-ins",
            get(locations_handlers::list_check_ins).post(locations_handlers::create_check_in),
        )
        .route(
            "/api/v1/locations/check-ins/:id",
            delete(locations_handlers::delete_check_in),
        )
        .route(
            "/api/v1/locations/emergency",
            get(locations_handlers::list_emergencies).post(locations_handlers::trigger_emergency),
        )
        .route(
            "/api/v1/locations/emergency/:id",
            patch(locations_handlers::resolve_emergency),
        )
}

/// Map service routes (`/map/*`): tile proxy + attribution + health.
/// Isolated so map load (tiles/search/routing) never starves auth or locations.
pub fn map_router() -> Router<AppState> {
    Router::new()
        .route("/map/tiles/:z/:x/:y", get(map_handlers::tile))
        .route("/map/search", get(map_handlers::search))
        .route("/map/reverse", get(map_handlers::reverse))
        .route("/map/route", get(map_handlers::route))
        .route("/map/capabilities", get(map_handlers::capabilities))
        .route("/map/attribution", get(map_handlers::attribution))
        .route("/map/health", get(map_handlers::health))
}

pub mod map_handlers {
    use crate::osm::{OsmClient, SearchPlace, TravelMode};
    use axum::{
        extract::Query,
        http::{header, HeaderMap, StatusCode},
        response::IntoResponse,
        Json,
    };
    use serde::Deserialize;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    /// Cache key is the real tile coordinate (u32 — `u8` silently wrapped at
    /// z >= 9 and served the wrong tile).
    type TileKey = (u32, u32, u32);
    type TileCacheMap = HashMap<TileKey, CachedTile>;

    struct CachedTile {
        bytes: Vec<u8>,
        source: &'static str,
        inserted: Instant,
    }

    static TILE_CACHE: OnceLock<Mutex<TileCacheMap>> = OnceLock::new();
    const TILE_TTL: Duration = Duration::from_secs(3600);
    const TILE_CACHE_MAX: usize = 1024;

    /// Ordered list of tile sources tried in sequence on failure.
    /// All are ODbL-compliant (OSM-derived). Never uses tile.openstreetmap.org.
    /// `tile.openstreetmap.de` (FOSSGIS) is the demo fallback; production
    /// self-hosts via `PINPOINT_TILE_URL` (ADR-0011). CARTO is intentionally
    /// not used keyless because it serves an "API KEY REQUIRED" watermark.
    const TILE_SOURCES: &[TileSource] = &[TileSource {
        name: "osm-de",
        url_pattern: "https://tile.openstreetmap.de/{z}/{x}/{y}.png",
        attribution: "© OpenStreetMap contributors",
    }];

    struct TileSource {
        name: &'static str,
        url_pattern: &'static str,
        attribution: &'static str,
    }

    fn tile_cache() -> &'static Mutex<TileCacheMap> {
        TILE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub fn tile_cache_len() -> usize {
        tile_cache().lock().unwrap().len()
    }

    pub async fn health() -> Json<serde_json::Value> {
        Json(json!({"service": "map", "status": "healthy", "cache_entries": tile_cache_len()}))
    }

    /// What this deployment can actually do: search autocomplete (self-hosted
    /// Nominatim only) and configured routing profiles.
    pub async fn capabilities() -> Json<Value> {
        let mut modes = vec!["driving"];
        if crate::osm::osrm_base_for(TravelMode::Foot).is_some() {
            modes.push("foot");
        }
        if crate::osm::osrm_base_for(TravelMode::Bike).is_some() {
            modes.push("bike");
        }
        let autocomplete = std::env::var("PINPOINT_SEARCH_AUTOCOMPLETE")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        Json(
            json!({ "autocomplete": autocomplete, "modes": modes, "avoid": crate::osm::osrm_supports_avoid() }),
        )
    }

    pub async fn attribution() -> Json<serde_json::Value> {
        Json(json!({
            "data": "© OpenStreetMap contributors",
            "license": "https://opendatacommons.org/licenses/odbl/1-0/",
            "tiles": TILE_SOURCES.iter().map(|s| json!({
                "name": s.name,
                "url_pattern": s.url_pattern,
                "attribution": s.attribution,
            })).collect::<Vec<_>>(),
            "note": "Sources tried in order; first success wins. Never uses tile.openstreetmap.org.",
        }))
    }

    /// Proxies a single raster tile (policy-compliant, OSM ODbL).
    /// Source order: self-hosted (`PINPOINT_TILE_URL`), then the demo
    /// fallbacks. Never uses tile.openstreetmap.org. Cached in-memory
    /// (TTL 1h, 1024 entries) with the serving source recorded.
    pub async fn tile(
        axum::extract::Path((z, x, y)): axum::extract::Path<(u32, u32, u32)>,
    ) -> impl IntoResponse {
        if z > 19 {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": {"code": "INVALID_ZOOM", "message": "Zoom must be 0-19"}})),
            )
                .into_response();
        }
        let key = (z, x, y);
        if let Some(cached) = tile_cache().lock().unwrap().get(&key) {
            if cached.inserted.elapsed() < TILE_TTL {
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
                headers.insert(
                    header::CACHE_CONTROL,
                    "public, max-age=3600".parse().unwrap(),
                );
                headers.insert(
                    "X-Data-Attribution",
                    "© OpenStreetMap contributors".parse().unwrap(),
                );
                headers.insert("X-Tile-Source", cached.source.parse().unwrap());
                return (StatusCode::OK, headers, cached.bytes.clone()).into_response();
            }
        }
        // Try each ODbL-compliant tile source in order; first success wins.
        // Public providers are demo fallbacks only (ADR-0011); none needs a key.
        // Never uses tile.openstreetmap.org (OSMF policy).
        let client = match reqwest::Client::builder()
            .local_address(Some(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
            .timeout(Duration::from_secs(10))
            .user_agent(concat!(
                "Pinpoint/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/Prince-of-Nothing/Pinpoint)"
            ))
            .build()
        {
            Ok(c) => c,
            Err(_) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": {"code": "TILE_FETCH_FAILED", "message": "Tile service unavailable"}})),
                )
                    .into_response()
            }
        };

        // Self-hosted tile server (production default). `PINPOINT_TILE_URL`
        // is a template like `https://tiles.example.com/{z}/{x}/{y}.png`.
        // Public providers below remain demo fallbacks; ODbL attribution is
        // returned either way.
        if let Ok(template) = std::env::var("PINPOINT_TILE_URL") {
            let t = template.trim();
            if !t.is_empty() {
                let url = t
                    .replace("{z}", &z.to_string())
                    .replace("{x}", &x.to_string())
                    .replace("{y}", &y.to_string());
                if let Ok(resp) = client.get(&url).send().await {
                    if resp.status().is_success() {
                        if let Ok(body) = resp.bytes().await {
                            if !body.is_empty() {
                                let bytes = body.to_vec();
                                {
                                    let mut cache = tile_cache().lock().unwrap();
                                    if cache.len() >= TILE_CACHE_MAX {
                                        cache.clear();
                                    }
                                    cache.insert(
                                        key,
                                        CachedTile {
                                            bytes: bytes.clone(),
                                            source: "self-hosted",
                                            inserted: Instant::now(),
                                        },
                                    );
                                }
                                let mut headers = HeaderMap::new();
                                headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
                                headers.insert(
                                    header::CACHE_CONTROL,
                                    "public, max-age=3600".parse().unwrap(),
                                );
                                headers.insert(
                                    "X-Data-Attribution",
                                    "© OpenStreetMap contributors".parse().unwrap(),
                                );
                                headers.insert("X-Tile-Source", "self-hosted".parse().unwrap());
                                return (StatusCode::OK, headers, bytes).into_response();
                            }
                        }
                    }
                }
            }
        }

        let mut last_error: Option<anyhow::Error> = None;
        for source in TILE_SOURCES {
            let mut url = source
                .url_pattern
                .replace("{z}", &z.to_string())
                .replace("{x}", &x.to_string())
                .replace("{y}", &y.to_string());
            // CARTO accepts keyless requests; the optional key raises limits.
            if source.name.starts_with("carto") {
                if let Ok(k) = std::env::var("CARTO_API_KEY") {
                    if !k.trim().is_empty() {
                        url = format!("{}?apikey={}", url, k.trim());
                    }
                }
            }
            // Public endpoints rate-limit bursts; one short retry keeps a
            // 15-tile screenful from falling back to a watermarked provider.
            let mut attempt = 0u8;
            loop {
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                        Ok(body) => {
                            let bytes = body.to_vec();
                            {
                                let mut cache = tile_cache().lock().unwrap();
                                if cache.len() >= TILE_CACHE_MAX {
                                    cache.clear();
                                }
                                cache.insert(
                                    key,
                                    CachedTile {
                                        bytes: bytes.clone(),
                                        source: source.name,
                                        inserted: Instant::now(),
                                    },
                                );
                            }
                            let mut headers = HeaderMap::new();
                            headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
                            headers.insert(
                                header::CACHE_CONTROL,
                                "public, max-age=3600".parse().unwrap(),
                            );
                            headers.insert(
                                "X-Data-Attribution",
                                "© OpenStreetMap contributors".parse().unwrap(),
                            );
                            headers.insert(
                                "X-Tile-Source",
                                header::HeaderValue::from_str(source.name).unwrap(),
                            );
                            return (StatusCode::OK, headers, bytes).into_response();
                        }
                        Err(_e) => {
                            last_error = Some(anyhow::anyhow!("{}: read failed", source.name));
                        }
                    },
                    Ok(resp) => {
                        last_error = Some(anyhow::anyhow!("{}: {}", source.name, resp.status()));
                    }
                    Err(_e) => {
                        last_error = Some(anyhow::anyhow!("{}: {}", source.name, _e));
                    }
                }
                attempt += 1;
                if attempt >= 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(120)).await;
            }
        }

        // All sources failed.
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": {
                    "code": "TILE_FETCH_FAILED",
                    "message": format!("All tile sources unavailable: {}", last_error.map(|e| e.to_string()).unwrap_or_default()),
                    "sources": TILE_SOURCES.iter().map(|s| s.name).collect::<Vec<_>>(),
                }
            }))
        )
            .into_response()
    }

    // -----------------------------------------------------------------------
    // OSM search / reverse / routing proxies (v1.1)
    // -----------------------------------------------------------------------

    #[derive(Deserialize)]
    pub struct SearchParams {
        q: String,
        lat: Option<f64>,
        lon: Option<f64>,
        limit: Option<usize>,
    }

    #[derive(Deserialize)]
    pub struct ReverseParams {
        lat: f64,
        lon: f64,
    }

    #[derive(Deserialize)]
    pub struct RouteParams {
        from_lat: f64,
        from_lon: f64,
        to_lat: f64,
        to_lon: f64,
        mode: Option<String>,
        alternatives: Option<bool>,
        via: Option<String>,
        avoid: Option<String>,
    }

    type MapCache = HashMap<String, (Value, Instant)>;
    static MAP_CACHE: OnceLock<Mutex<MapCache>> = OnceLock::new();
    static ROUTE_PERMITS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    const MAP_CACHE_MAX: usize = 512;

    fn map_cache() -> &'static Mutex<MapCache> {
        MAP_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn cache_get(key: &str, ttl: Duration) -> Option<Value> {
        let cache = map_cache().lock().unwrap();
        let (value, inserted) = cache.get(key)?;
        if inserted.elapsed() < ttl {
            Some(value.clone())
        } else {
            None
        }
    }

    fn cache_put(key: String, value: Value) {
        let mut cache = map_cache().lock().unwrap();
        if cache.len() >= MAP_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, (value, Instant::now()));
    }

    fn valid_coord(lat: f64, lon: f64) -> bool {
        lat.is_finite()
            && lon.is_finite()
            && (-90.0..=90.0).contains(&lat)
            && (-180.0..=180.0).contains(&lon)
    }

    /// `via=lat,lon;lat,lon` (max 6 stops).
    fn parse_via(raw: Option<&str>) -> Result<Vec<(f64, f64)>, ()> {
        let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for pair in raw.split(';') {
            let mut parts = pair.split(',');
            let lat: f64 = parts.next().ok_or(())?.trim().parse().map_err(|_| ())?;
            let lon: f64 = parts.next().ok_or(())?.trim().parse().map_err(|_| ())?;
            if parts.next().is_some() || !valid_coord(lat, lon) {
                return Err(());
            }
            out.push((lat, lon));
            if out.len() > 6 {
                return Err(());
            }
        }
        Ok(out)
    }

    /// `avoid=motorway,toll,ferry`.
    fn parse_avoid(raw: Option<&str>) -> Result<Vec<String>, ()> {
        let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
            return Ok(Vec::new());
        };
        let mut out: Vec<String> = Vec::new();
        for item in raw.split(',') {
            let item = item.trim().to_ascii_lowercase();
            if !["motorway", "toll", "ferry"].contains(&item.as_str()) {
                return Err(());
            }
            if !out.contains(&item) {
                out.push(item);
            }
        }
        Ok(out)
    }

    fn parse_mode(mode: Option<&str>) -> Option<TravelMode> {
        match mode
            .unwrap_or("driving")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "driving" => Some(TravelMode::Driving),
            "foot" => Some(TravelMode::Foot),
            "bike" => Some(TravelMode::Bike),
            _ => None,
        }
    }

    fn api_error(status: StatusCode, code: &str, message: &str) -> axum::response::Response {
        (
            status,
            Json(json!({"error": {"code": code, "message": message}})),
        )
            .into_response()
    }

    fn upstream_error(what: &str) -> axum::response::Response {
        api_error(
            StatusCode::BAD_GATEWAY,
            "OSM_UNAVAILABLE",
            &format!("{what} is unavailable right now. Try again."),
        )
    }

    fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6_371_000.0_f64;
        let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
        let dp = (lat2 - lat1).to_radians();
        let dl = (lon2 - lon1).to_radians();
        let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
        2.0 * r * a.sqrt().atan2((1.0 - a).sqrt())
    }

    fn place_json(place: &SearchPlace, bias: Option<(f64, f64)>) -> Value {
        let distance = bias.map(|(lat, lon)| distance_m(lat, lon, place.latitude, place.longitude));
        json!({
            "display_name": place.display_name,
            "short_name": place.short_name,
            "latitude": place.latitude,
            "longitude": place.longitude,
            "category": place.category,
            "place_type": place.place_type,
            "distance_m": distance,
        })
    }

    fn route_json(route: &crate::osm::Route) -> Value {
        let points: Vec<[f64; 2]> = route.points.iter().map(|(lat, lon)| [*lat, *lon]).collect();
        let steps: Vec<Value> = route
            .steps
            .iter()
            .map(|s| {
                json!({
                    "text": s.text,
                    "distance_m": s.distance_m,
                    "duration_s": s.duration_s,
                })
            })
            .collect();
        let legs: Vec<Value> = route
            .legs
            .iter()
            .map(|leg| {
                json!({
                    "distance_m": leg.distance_m,
                    "duration_s": leg.duration_s,
                })
            })
            .collect();
        json!({
            "distance_m": route.distance_m,
            "duration_s": route.duration_s,
            "summary": route.summary,
            "points": points,
            "steps": steps,
            "legs": legs,
        })
    }

    /// `GET /map/search?q=&lat=&lon=&limit=`
    pub async fn search(Query(p): Query<SearchParams>) -> axum::response::Response {
        let q = p.q.trim();
        let len = q.chars().count();
        if !(3..=120).contains(&len) {
            return api_error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                "Search needs 3–120 characters.",
            );
        }
        let bias = match (p.lat, p.lon) {
            (None, None) => None,
            (Some(lat), Some(lon)) if valid_coord(lat, lon) => Some((lat, lon)),
            _ => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "VALIDATION_FAILED",
                    "lat and lon must be a valid pair.",
                )
            }
        };
        let limit = p.limit.unwrap_or(8);
        if !(1..=10).contains(&limit) {
            return api_error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                "limit must be 1–10.",
            );
        }
        let key = format!(
            "search:{}:{}:{}:{}",
            q.to_lowercase(),
            bias.map(|(lat, _)| (lat * 1000.0).round() as i64)
                .unwrap_or(0),
            bias.map(|(_, lon)| (lon * 1000.0).round() as i64)
                .unwrap_or(0),
            limit
        );
        if let Some(value) = cache_get(&key, Duration::from_secs(60)) {
            return (StatusCode::OK, Json(value)).into_response();
        }
        let client = match OsmClient::new() {
            Ok(c) => c,
            Err(_) => return upstream_error("Search"),
        };
        match client.search(q).await {
            Ok(items) => {
                let mut results: Vec<Value> = items.iter().map(|p| place_json(p, bias)).collect();
                if bias.is_some() {
                    results.sort_by(|a, b| {
                        let da = a["distance_m"].as_f64().unwrap_or(f64::MAX);
                        let db = b["distance_m"].as_f64().unwrap_or(f64::MAX);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                results.truncate(limit);
                let value = json!({ "results": results });
                cache_put(key, value.clone());
                (StatusCode::OK, Json(value)).into_response()
            }
            Err(_) => upstream_error("Search"),
        }
    }

    /// `GET /map/reverse?lat=&lon=`
    pub async fn reverse(Query(p): Query<ReverseParams>) -> axum::response::Response {
        if !valid_coord(p.lat, p.lon) {
            return api_error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                "lat and lon must be valid coordinates.",
            );
        }
        let key = format!("reverse:{:.5}:{:.5}", p.lat, p.lon);
        if let Some(value) = cache_get(&key, Duration::from_secs(600)) {
            return (StatusCode::OK, Json(value)).into_response();
        }
        let client = match OsmClient::new() {
            Ok(c) => c,
            Err(_) => return upstream_error("Reverse geocoding"),
        };
        match client.reverse(p.lat, p.lon).await {
            Ok(place) => {
                let value = place_json(&place, None);
                cache_put(key, value.clone());
                (StatusCode::OK, Json(value)).into_response()
            }
            Err(_) => upstream_error("Reverse geocoding"),
        }
    }

    /// `GET /map/route?from_lat=&from_lon=&to_lat=&to_lon=&mode=&alternatives=`
    pub async fn route(Query(p): Query<RouteParams>) -> axum::response::Response {
        if !valid_coord(p.from_lat, p.from_lon) || !valid_coord(p.to_lat, p.to_lon) {
            return api_error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                "from/to must be valid coordinates.",
            );
        }
        let Some(mode) = parse_mode(p.mode.as_deref()) else {
            return api_error(
                StatusCode::BAD_REQUEST,
                "VALIDATION_FAILED",
                "mode must be driving, foot or bike.",
            );
        };
        if crate::osm::osrm_base_for(mode).is_none() {
            return api_error(
                StatusCode::BAD_REQUEST,
                "MODE_UNAVAILABLE",
                "This routing profile is not configured on this server.",
            );
        }
        let alternatives = p.alternatives.unwrap_or(true);
        let via = match parse_via(p.via.as_deref()) {
            Ok(v) => v,
            Err(_) => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "VALIDATION_FAILED",
                    "via must be up to 6 lat,lon pairs separated by ';'.",
                )
            }
        };
        let avoid = match parse_avoid(p.avoid.as_deref()) {
            Ok(v) => v,
            Err(_) => {
                return api_error(
                    StatusCode::BAD_REQUEST,
                    "VALIDATION_FAILED",
                    "avoid must be motorway, toll or ferry.",
                )
            }
        };
        if !avoid.is_empty() && !crate::osm::osrm_supports_avoid() {
            return api_error(
                StatusCode::BAD_REQUEST,
                "AVOID_UNAVAILABLE",
                "Avoid options require a self-hosted OSRM instance.",
            );
        }
        let key = format!(
            "route:{:.5},{:.5};{:.5},{:.5}:{}:{}:{}:{}",
            p.from_lat,
            p.from_lon,
            p.to_lat,
            p.to_lon,
            mode.profile(),
            alternatives,
            via.iter()
                .map(|(lat, lon)| format!("{lat:.4},{lon:.4}"))
                .collect::<Vec<_>>()
                .join(";"),
            avoid.join(",")
        );
        if let Some(value) = cache_get(&key, Duration::from_secs(300)) {
            return (StatusCode::OK, Json(value)).into_response();
        }
        let permits = ROUTE_PERMITS.get_or_init(|| tokio::sync::Semaphore::new(2));
        let _permit = match permits.try_acquire() {
            Ok(p) => p,
            Err(_) => {
                return api_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "BUSY",
                    "Routing is busy, try again.",
                )
            }
        };
        let client = match OsmClient::new() {
            Ok(c) => c,
            Err(_) => return upstream_error("Routing"),
        };
        match client
            .route_alternatives_via(
                (p.from_lat, p.from_lon),
                (p.to_lat, p.to_lon),
                mode,
                &via,
                &avoid,
            )
            .await
        {
            Ok(mut routes) => {
                if !alternatives {
                    routes.truncate(1);
                }
                if routes.is_empty() {
                    return api_error(
                        StatusCode::NOT_FOUND,
                        "NO_ROUTE",
                        "No route found between these points.",
                    );
                }
                let value = json!({ "routes": routes.iter().map(route_json).collect::<Vec<_>>() });
                cache_put(key, value.clone());
                (StatusCode::OK, Json(value)).into_response()
            }
            Err(e) if e.to_string().contains("No route") => api_error(
                StatusCode::NOT_FOUND,
                "NO_ROUTE",
                "No route found between these points.",
            ),
            Err(_) => upstream_error("Routing"),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn mode_parsing_is_strict() {
            assert!(parse_mode(Some("walk")).is_none());
            assert_eq!(parse_mode(Some("foot")), Some(TravelMode::Foot));
            assert_eq!(parse_mode(Some("bike")), Some(TravelMode::Bike));
            assert_eq!(parse_mode(None), Some(TravelMode::Driving));
        }

        #[test]
        fn coordinate_validation_rejects_nan_and_out_of_range() {
            assert!(valid_coord(47.0, 28.8));
            assert!(!valid_coord(f64::NAN, 0.0));
            assert!(!valid_coord(999.0, 0.0));
            assert!(!valid_coord(0.0, 181.0));
        }

        #[test]
        fn place_json_has_all_fields() {
            let place = SearchPlace {
                display_name: "Pharmacy, Chișinău".into(),
                short_name: "Pharmacy".into(),
                latitude: 47.0,
                longitude: 28.8,
                category: "pharmacy".into(),
                place_type: "chemist".into(),
                distance_m: None,
            };
            let value = place_json(&place, Some((47.01, 28.81)));
            for key in [
                "display_name",
                "short_name",
                "latitude",
                "longitude",
                "category",
                "place_type",
                "distance_m",
            ] {
                assert!(value.get(key).is_some(), "missing {key}");
            }
            assert!(value["distance_m"].as_f64().unwrap() > 0.0);
        }

        #[test]
        fn upstream_failure_is_502() {
            assert_eq!(upstream_error("Search").status(), StatusCode::BAD_GATEWAY);
        }

        #[test]
        fn via_and_avoid_parsing_is_strict() {
            assert_eq!(
                parse_via(Some("47.0,28.8;47.1,28.9")).unwrap(),
                vec![(47.0, 28.8), (47.1, 28.9)]
            );
            assert!(parse_via(Some("47.0")).is_err());
            assert!(parse_via(Some("999,28.8")).is_err());
            assert_eq!(
                parse_avoid(Some("toll,ferry")).unwrap(),
                vec!["toll".to_string(), "ferry".to_string()]
            );
            assert!(parse_avoid(Some("tunnel")).is_err());
        }
    }
}

/// Error envelope matching the Go API: `{"error": {"code": ..., "message": ...}}`.
pub fn error_envelope(code: &str, message: &str) -> axum::response::Response {
    (
        axum::http::StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

pub use axum::response::IntoResponse;

#[cfg(test)]
mod log_tests {
    use super::*;

    #[test]
    fn logs_drop_query_strings() {
        let uri = axum::http::Uri::from_static("/map/route?from_lat=47.0&to_lon=28.8");
        assert_eq!(loggable_uri(&uri), "/map/route");
    }
}

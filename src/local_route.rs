//! Local, on-device routing for the loopback map servers.
//!
//! The Android and desktop map pages ask `GET /map/route?from_lat=…` for a
//! route. This module answers from [`native_router::Graph`] loaded out of a
//! local `graph.bin` cache; it never calls an online routing service and has
//! no network client.
//!
//! Graph resolution order:
//!
//! 1. `PINPOINT_ROUTER_GRAPH` when set (the value is used verbatim);
//! 2. `packs/moldova.graph.bin` under [`crate::platform::app_data_dir`].
//!
//! The cache is loaded lazily, once, in a blocking task. Missing or invalid
//! caches produce `503 ROUTING_UNAVAILABLE`; the caller must show that the
//! offline area is unavailable instead of falling back to a network router.
//!
//! The JSON response is the route UI contract used by
//! `web/app/route-ui.js`:
//!
//! ```json
//! {
//!   "distance_m": 134109.5,
//!   "duration_s": 5836.8,
//!   "points": [[47.01, 28.86]],
//!   "maneuvers": [
//!     {"instruction": "Turn right onto ...", "kind": "right",
//!      "distance_m": 865.1, "duration_s": 61.8,
//!      "point": [47.02, 28.85], "point_index": 123}
//!   ]
//! }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use axum::extract::{Extension, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use native_router::{Graph, Maneuver, Profile, RouteError};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Environment override for the local routing graph cache.
pub const GRAPH_ENV: &str = "PINPOINT_ROUTER_GRAPH";

/// Default graph file inside the app data directory (`packs/`).
pub const DEFAULT_GRAPH_FILE: &str = "moldova.graph.bin";

/// Error code used when no usable local graph is available.
pub const ROUTING_UNAVAILABLE: &str = "ROUTING_UNAVAILABLE";

/// A validated route request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteRequest {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub profile: Profile,
}

/// The JSON route shape shared with `web/app/route-ui.js`.
///
/// `nodes` from the router crate is deliberately omitted: the route UI only
/// needs geometry and maneuvers, and OSM node ids would bloat the response.
#[derive(Debug, Clone, Serialize)]
pub struct RouteResponse {
    pub distance_m: f64,
    pub duration_s: f64,
    pub points: Vec<(f64, f64)>,
    pub maneuvers: Vec<Maneuver>,
}

/// Raw query parameters. Missing, malformed or out-of-range values are
/// reported as `400` by [`RouteRequest::from_query`].
#[derive(Debug, Default, Deserialize)]
pub struct RouteQuery {
    pub from_lat: Option<String>,
    pub from_lon: Option<String>,
    pub to_lat: Option<String>,
    pub to_lon: Option<String>,
    pub profile: Option<String>,
}

/// A failed route request with the JSON error body and HTTP status to send.
#[derive(Debug)]
pub struct RouteFailure {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: &'a str,
}

impl RouteFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "INVALID_COORDINATES",
            message: message.into(),
        }
    }

    fn unsupported_profile(profile: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "UNSUPPORTED_PROFILE",
            message: format!(
                "unsupported profile '{profile}': only the offline car profile is available"
            ),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: ROUTING_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn outside_coverage(snap_m: f64) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "OUTSIDE_COVERAGE",
            message: format!(
                "origin or destination is outside the offline area (nearest road {snap_m:.0} m away)"
            ),
        }
    }

    fn no_route(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NO_ROUTE",
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "ROUTING_FAILED",
            message: message.into(),
        }
    }
}

impl IntoResponse for RouteFailure {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.code,
            message: &self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

impl RouteRequest {
    /// Parse and validate the HTTP query.
    pub fn from_query(query: &RouteQuery) -> Result<Self, RouteFailure> {
        let from_lat = parse_coordinate("from_lat", query.from_lat.as_deref(), 90.0)?;
        let from_lon = parse_coordinate("from_lon", query.from_lon.as_deref(), 180.0)?;
        let to_lat = parse_coordinate("to_lat", query.to_lat.as_deref(), 90.0)?;
        let to_lon = parse_coordinate("to_lon", query.to_lon.as_deref(), 180.0)?;
        let profile = parse_profile(query.profile.as_deref())?;
        Ok(Self {
            from: (from_lat, from_lon),
            to: (to_lat, to_lon),
            profile,
        })
    }
}

fn parse_coordinate(
    name: &'static str,
    raw: Option<&str>,
    limit: f64,
) -> Result<f64, RouteFailure> {
    let Some(raw) = raw else {
        return Err(RouteFailure::invalid(format!(
            "missing {name}; request from_lat, from_lon, to_lat and to_lon"
        )));
    };
    let value = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| RouteFailure::invalid(format!("{name} must be a decimal number")))?;
    if !value.is_finite() || value.abs() > limit {
        return Err(RouteFailure::invalid(format!(
            "{name} must be between -{limit:.0} and {limit:.0}"
        )));
    }
    Ok(value)
}

fn parse_profile(raw: Option<&str>) -> Result<Profile, RouteFailure> {
    let name = raw.unwrap_or("auto").trim();
    if name.is_empty() {
        return Ok(Profile::Car);
    }
    Profile::from_name(name)
        .ok_or_else(|| RouteFailure::unsupported_profile(&name.to_ascii_lowercase()))
}

/// Lazily loaded graph plus a route gate.
///
/// The route gate serialises A* queries: the Moldova graph is ~106 MB and one
/// query allocates working arrays on top of it, so concurrent routes would
/// multiply the phone's peak memory for no user benefit.
pub struct LocalRouter {
    graph_path: Option<PathBuf>,
    graph: Mutex<Option<LoadedGraph>>,
    route_gate: Mutex<()>,
}

struct LoadedGraph {
    graph: Arc<Graph>,
    len: u64,
    modified: Option<SystemTime>,
}

impl LoadedGraph {
    async fn matches(&self, path: &Path) -> bool {
        match tokio::fs::metadata(path).await {
            Ok(metadata) => {
                metadata.len() == self.len
                    && metadata.modified().ok() == self.modified
                    && metadata.is_file()
            }
            Err(_) => false,
        }
    }
}

impl LocalRouter {
    /// Resolve the graph path from the environment or the app data directory.
    pub fn from_environment() -> Self {
        Self::new(resolve_graph_path())
    }

    /// Resolve the environment override first, then `<packs_dir>/moldova.graph.bin`.
    /// The desktop map server uses this so a temp/test packs directory routes
    /// with the graph the download manager just installed.
    pub fn for_packs_dir(packs_dir: Option<&Path>) -> Self {
        if std::env::var_os(GRAPH_ENV).is_some() {
            return Self::from_environment();
        }
        Self::new(packs_dir.and_then(resolve_packs_graph))
    }

    /// Build a router for an explicit graph path (used by unit tests).
    pub fn new(graph_path: Option<PathBuf>) -> Self {
        Self {
            graph_path,
            graph: Mutex::new(None),
            route_gate: Mutex::new(()),
        }
    }

    /// The configured cache path, when one could be resolved.
    pub fn graph_path(&self) -> Option<&Path> {
        self.graph_path.as_deref()
    }

    /// Load the graph once, returning an `Arc` shared by later requests.
    ///
    /// The installed file's length and modification time are checked on each
    /// request, so replacing `packs/moldova.graph.bin` after a download is
    /// picked up without restarting the map server.
    async fn graph(&self) -> Result<Arc<Graph>, RouteFailure> {
        let Some(path) = self.graph_path.clone() else {
            return Err(RouteFailure::unavailable(format!(
                "no local routing graph configured; set {GRAPH_ENV} or download the \
                 offline pack containing packs/{DEFAULT_GRAPH_FILE}"
            )));
        };

        let mut guard = self.graph.lock().await;
        if let Some(loaded) = guard.as_ref() {
            if loaded.matches(&path).await {
                return Ok(Arc::clone(&loaded.graph));
            }
            tracing::info!(path = %path.display(), "local routing graph changed; reloading");
            *guard = None;
        }

        let load_path = path.clone();
        let loaded = tokio::task::spawn_blocking(move || Graph::load(&load_path)).await;
        let loaded = match loaded {
            Ok(result) => result,
            Err(error) => {
                return Err(RouteFailure::internal(format!(
                    "graph load task failed: {error}"
                )))
            }
        };

        match loaded {
            Ok(graph) => {
                let metadata = tokio::fs::metadata(&path).await.ok();
                let loaded = LoadedGraph {
                    graph: Arc::new(graph),
                    len: metadata.as_ref().map(std::fs::Metadata::len).unwrap_or(0),
                    modified: metadata
                        .as_ref()
                        .and_then(|metadata| metadata.modified().ok()),
                };
                tracing::info!(
                    path = %path.display(),
                    nodes = loaded.graph.node_count(),
                    edges = loaded.graph.edge_count(),
                    "local routing graph loaded"
                );
                let graph = Arc::clone(&loaded.graph);
                *guard = Some(loaded);
                Ok(graph)
            }
            Err(error) => Err(RouteFailure::unavailable(format!(
                "no usable local routing graph at {} ({error}); download the offline pack \
                 or set {GRAPH_ENV}",
                path.display()
            ))),
        }
    }

    /// Compute a route without any network access.
    pub async fn route(&self, request: RouteRequest) -> Result<RouteResponse, RouteFailure> {
        let graph = self.graph().await?;
        let _gate = self.route_gate.lock().await;

        let from = request.from;
        let to = request.to;
        let profile = request.profile;
        let routed =
            tokio::task::spawn_blocking(move || native_router::route(&graph, from, to, profile))
                .await
                .map_err(|error| RouteFailure::internal(format!("route task failed: {error}")))?;

        match routed {
            Ok(route) => Ok(RouteResponse {
                distance_m: route.distance_m,
                duration_s: route.duration_s,
                points: route.points,
                maneuvers: route.maneuvers,
            }),
            Err(RouteError::OutsideCoverage { snap_m }) => {
                Err(RouteFailure::outside_coverage(snap_m))
            }
            Err(RouteError::NoPath) => Err(RouteFailure::no_route(
                "no car route found between the snapped nodes",
            )),
            Err(RouteError::EmptyGraph) => {
                Err(RouteFailure::no_route("the local routing graph is empty"))
            }
        }
    }
}

/// Graph path from `PINPOINT_ROUTER_GRAPH` or `<app data>/packs/moldova.graph.bin`.
pub fn resolve_graph_path() -> Option<PathBuf> {
    if let Some(raw) = std::env::var_os(GRAPH_ENV) {
        let path = PathBuf::from(raw);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    crate::platform::app_data_dir()
        .map(|dir| dir.join("packs"))
        .and_then(|packs| resolve_packs_graph(&packs))
}

/// Resolve the installed routing graph for the catalog layout.
///
/// The v2.4+ installer writes `packs/<region>/<region>.graph.bin`; v2.3
/// installed `packs/moldova.graph.bin` at the root. Prefer the region layout,
/// then the legacy root, then the first installed region directory so a
/// single-region install still routes.
fn resolve_packs_graph(packs_dir: &Path) -> Option<PathBuf> {
    let preferred = packs_dir.join("moldova").join(DEFAULT_GRAPH_FILE);
    if preferred.is_file() {
        return Some(preferred);
    }
    let legacy = packs_dir.join(DEFAULT_GRAPH_FILE);
    if legacy.is_file() {
        return Some(legacy);
    }
    let mut regions: Vec<_> = std::fs::read_dir(packs_dir)
        .ok()?
        .filter_map(Result::ok)
        .collect();
    regions.sort_by_key(|entry| entry.file_name());
    for entry in regions {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let candidate = entry
            .path()
            .join(format!("{}.graph.bin", name.to_string_lossy()));
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// `GET /map/route` handler mounted by the Android and desktop map servers.
///
/// Each server supplies its own [`LocalRouter`], bound to the same `packs/`
/// directory it serves, so a downloaded graph is used without a restart.
pub async fn map_route(
    Extension(router): Extension<Arc<LocalRouter>>,
    Query(query): Query<RouteQuery>,
) -> Response {
    let request = match RouteRequest::from_query(&query) {
        Ok(request) => request,
        Err(failure) => return failure.into_response(),
    };
    match router.route(request).await {
        Ok(route) => (StatusCode::OK, Json(route)).into_response(),
        Err(failure) => failure.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use native_router::{Edge, Node, RoadClass};

    fn synthetic_graph() -> Graph {
        let nodes = vec![
            Node {
                osm_id: 1,
                lat: 47.0,
                lon: 28.0,
            },
            Node {
                osm_id: 2,
                lat: 47.0,
                lon: 28.005,
            },
            Node {
                osm_id: 3,
                lat: 47.002,
                lon: 28.01,
            },
            Node {
                osm_id: 4,
                lat: 47.004,
                lon: 28.015,
            },
            Node {
                osm_id: 5,
                lat: 47.004,
                lon: 28.02,
            },
        ];
        let names = vec![Box::from("Strada Testului")];
        let mut edges = Vec::new();
        for index in 0..nodes.len() - 1 {
            for (from, to) in [(index, index + 1), (index + 1, index)] {
                edges.push(Edge {
                    name: 0,
                    ..Edge::new(from as u32, to as u32, 400.0, 50.0, RoadClass::Residential)
                });
            }
        }
        Graph::from_edges_with_names(nodes, edges, names)
    }

    fn save_synthetic_graph(dir: &tempfile::TempDir) -> PathBuf {
        let path = dir.path().join("graph.bin");
        synthetic_graph().save(&path).expect("save synthetic graph");
        path
    }

    fn valid_query() -> RouteQuery {
        RouteQuery {
            from_lat: Some("47.0".into()),
            from_lon: Some("28.0".into()),
            to_lat: Some("47.004".into()),
            to_lon: Some("28.02".into()),
            profile: Some("auto".into()),
        }
    }

    #[test]
    fn graph_cache_round_trips_through_graph_bin() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = save_synthetic_graph(&dir);
        let original = synthetic_graph();
        let loaded = Graph::load(&path).expect("load saved graph");

        assert_eq!(loaded.node_count(), original.node_count());
        assert_eq!(loaded.edge_count(), original.edge_count());
        assert_eq!(loaded.names(), original.names());
        assert_eq!(
            loaded.node(0).map(|node| node.osm_id),
            original.node(0).map(|node| node.osm_id)
        );
    }

    #[tokio::test]
    async fn routes_over_a_saved_synthetic_graph() {
        let dir = tempfile::tempdir().expect("temp dir");
        let router = LocalRouter::new(Some(save_synthetic_graph(&dir)));
        let request = RouteRequest::from_query(&valid_query()).expect("valid request");
        let route = router.route(request).await.expect("route succeeds");

        assert!(route.distance_m > 0.0);
        assert!(route.duration_s > 0.0);
        assert_eq!(route.points.len(), 5);
        assert_eq!(route.points.first(), Some(&(47.0, 28.0)));
        assert_eq!(route.points.last(), Some(&(47.004, 28.02)));
        assert!(route.maneuvers.len() >= 2);
        assert!(route.maneuvers[0].instruction.contains("Strada Testului"));
        assert_eq!(
            route
                .maneuvers
                .last()
                .map(|maneuver| maneuver.kind.as_str()),
            Some("arrive")
        );

        let json = serde_json::to_value(&route).expect("route JSON");
        assert!(json["distance_m"].as_f64().unwrap() > 0.0);
        assert_eq!(json["points"].as_array().unwrap().len(), 5);
        assert!(!json["maneuvers"].as_array().unwrap().is_empty());

        // The loaded graph is reused by the second query.
        router.route(request).await.expect("cached route succeeds");
    }

    #[test]
    fn rejects_missing_and_invalid_coordinates() {
        let mut missing = valid_query();
        missing.to_lat = None;
        let failure = RouteRequest::from_query(&missing).expect_err("missing coordinate");
        assert_eq!(failure.status, StatusCode::BAD_REQUEST);
        assert_eq!(failure.code, "INVALID_COORDINATES");
        assert!(failure.message.contains("to_lat"), "{}", failure.message);

        for (field, value) in [
            ("from_lat", "not-a-number"),
            ("from_lat", "91"),
            ("to_lon", "-181"),
            ("to_lat", "NaN"),
        ] {
            let mut query = valid_query();
            match field {
                "from_lat" => query.from_lat = Some(value.into()),
                "to_lat" => query.to_lat = Some(value.into()),
                "to_lon" => query.to_lon = Some(value.into()),
                _ => unreachable!(),
            }
            let failure =
                RouteRequest::from_query(&query).expect_err("invalid coordinate should fail");
            assert_eq!(failure.status, StatusCode::BAD_REQUEST);
            assert_eq!(failure.code, "INVALID_COORDINATES");
        }
    }

    #[test]
    fn rejects_pedestrian_and_bicycle_profiles_honestly() {
        for profile in ["pedestrian", "bicycle"] {
            let mut query = valid_query();
            query.profile = Some(profile.into());
            let failure =
                RouteRequest::from_query(&query).expect_err("unsupported profile should fail");
            assert_eq!(failure.status, StatusCode::BAD_REQUEST);
            assert_eq!(failure.code, "UNSUPPORTED_PROFILE");
            assert!(failure.message.contains(profile), "{}", failure.message);
        }
    }

    #[tokio::test]
    async fn outside_coverage_and_no_path_are_404() {
        let dir = tempfile::tempdir().expect("temp dir");
        let router = LocalRouter::new(Some(save_synthetic_graph(&dir)));

        let outside = RouteRequest {
            from: (0.0, 0.0),
            to: (47.004, 28.02),
            profile: Profile::Car,
        };
        let failure = router.route(outside).await.expect_err("outside coverage");
        assert_eq!(failure.status, StatusCode::NOT_FOUND);
        assert_eq!(failure.code, "OUTSIDE_COVERAGE");

        let disconnected = Graph::from_edges_with_names(
            vec![
                Node {
                    osm_id: 10,
                    lat: 47.0,
                    lon: 28.0,
                },
                Node {
                    osm_id: 11,
                    lat: 47.1,
                    lon: 28.1,
                },
            ],
            Vec::new(),
            Vec::new(),
        );
        let path = dir.path().join("disconnected.graph.bin");
        disconnected.save(&path).expect("save disconnected graph");
        let router = LocalRouter::new(Some(path));
        let no_path = RouteRequest {
            from: (47.0, 28.0),
            to: (47.1, 28.1),
            profile: Profile::Car,
        };
        let failure = router.route(no_path).await.expect_err("no path");
        assert_eq!(failure.status, StatusCode::NOT_FOUND);
        assert_eq!(failure.code, "NO_ROUTE");
    }

    #[tokio::test]
    async fn missing_graph_is_503_routing_unavailable() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("does-not-exist.graph.bin");
        let router = LocalRouter::new(Some(missing));

        let request = RouteRequest::from_query(&valid_query()).expect("valid request");
        let failure = router.route(request).await.expect_err("missing graph");
        assert_eq!(failure.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(failure.code, ROUTING_UNAVAILABLE);

        let response = failure.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("error body");
        let error: serde_json::Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(error["error"], ROUTING_UNAVAILABLE);
        assert!(
            error["message"]
                .as_str()
                .unwrap_or_default()
                .contains("graph"),
            "error message should name the missing graph: {error}"
        );
    }

    #[test]
    fn packs_graph_resolution_prefers_the_catalog_layout() {
        let dir = tempfile::tempdir().expect("temp dir");
        let catalog = dir.path().join("moldova").join(DEFAULT_GRAPH_FILE);
        std::fs::create_dir_all(catalog.parent().expect("region dir")).expect("create region");
        std::fs::write(&catalog, b"graph").expect("write graph");
        assert_eq!(resolve_packs_graph(dir.path()), Some(catalog.clone()));

        std::fs::remove_file(&catalog).expect("remove catalog graph");
        let legacy = dir.path().join(DEFAULT_GRAPH_FILE);
        std::fs::write(&legacy, b"legacy").expect("write legacy");
        assert_eq!(resolve_packs_graph(dir.path()), Some(legacy));

        let other = tempfile::tempdir().expect("other temp dir");
        let romania = other.path().join("romania").join("romania.graph.bin");
        std::fs::create_dir_all(romania.parent().expect("region dir")).expect("create other");
        std::fs::write(&romania, b"romania").expect("write other");
        assert_eq!(resolve_packs_graph(other.path()), Some(romania));
    }
}

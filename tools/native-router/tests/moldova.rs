//! Real-extract acceptance test for the M1 Moldova pack.
//!
//! The PBF is not committed (101 MB). Point `PINPOINT_MOLDOVA_PBF` at it and
//! run:
//!
//! ```powershell
//! $env:PINPOINT_MOLDOVA_PBF = "C:\...\dist\offline\moldova\moldova.osm.pbf"
//! cargo test --release --test moldova -- --ignored --nocapture
//! ```
//!
//! Without the variable the test prints a skip notice and returns. Nothing in
//! this test or in the crate performs network I/O.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use native_router::{haversine_m, load_pbf, route, Graph, Profile};

const CHISINAU: (f64, f64) = (47.01, 28.86);
const BALTI: (f64, f64) = (47.76, 27.93);

struct TempGraph(PathBuf);

impl TempGraph {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = env::temp_dir();
        path.push(format!(
            "pinpoint-moldova-{}-{nanos}.graph.bin",
            std::process::id()
        ));
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempGraph {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
#[ignore = "requires the M1 Moldova PBF via PINPOINT_MOLDOVA_PBF"]
fn chisinau_to_balti_is_routed_locally() {
    let Ok(pbf) = env::var("PINPOINT_MOLDOVA_PBF") else {
        println!("skipping: set PINPOINT_MOLDOVA_PBF to dist/offline/moldova/moldova.osm.pbf");
        return;
    };

    let load_started = Instant::now();
    let graph = load_pbf(&pbf).expect("Moldova PBF should build a car graph");
    let load_s = load_started.elapsed().as_secs_f64();
    println!(
        "graph: nodes={} edges={} names={} load_s={load_s:.2}",
        graph.node_count(),
        graph.edge_count(),
        graph.names().len()
    );
    assert!(
        !graph.names().is_empty(),
        "the Moldova extract must contribute road names"
    );

    let route_started = Instant::now();
    let route =
        route(&graph, CHISINAU, BALTI, Profile::Car).expect("Chișinău–Bălți must be routable");
    let route_ms = route_started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "route: distance_m={:.1} duration_s={:.1} nodes={} points={} maneuvers={} query_ms={route_ms:.1}",
        route.distance_m,
        route.duration_s,
        route.nodes.len(),
        route.points.len(),
        route.maneuvers.len()
    );

    // The M14/road route is roughly 130 km; allow for mapping and graph detail.
    assert!(
        (100_000.0..190_000.0).contains(&route.distance_m),
        "unexpected distance {} m",
        route.distance_m
    );
    assert!(
        (3_000.0..15_000.0).contains(&route.duration_s),
        "unexpected duration {} s",
        route.duration_s
    );
    assert!(route.nodes.len() > 50, "route has too few nodes");
    assert_eq!(route.points.len(), route.nodes.len());

    let first = *route.points.first().unwrap();
    let last = *route.points.last().unwrap();
    assert!(
        haversine_m(CHISINAU, first) < 5_000.0,
        "origin snapped too far: {} m",
        haversine_m(CHISINAU, first)
    );
    assert!(
        haversine_m(BALTI, last) < 5_000.0,
        "destination snapped too far: {} m",
        haversine_m(BALTI, last)
    );

    assert!(
        route.maneuvers.len() > 1,
        "expected a real turn-by-turn list, got {} maneuver(s)",
        route.maneuvers.len()
    );
    assert_eq!(route.maneuvers.first().unwrap().kind, "depart");
    assert_eq!(route.maneuvers.first().unwrap().point_index, 0);
    assert_eq!(route.maneuvers.last().unwrap().kind, "arrive");
    assert_eq!(route.maneuvers.last().unwrap().distance_m, 0.0);
    assert_eq!(route.maneuvers.last().unwrap().duration_s, 0.0);

    let named = route
        .maneuvers
        .iter()
        .filter(|maneuver| {
            maneuver.instruction.contains(" on ") || maneuver.instruction.contains(" onto ")
        })
        .count();
    println!(
        "maneuvers: kinds={:?} named={named}",
        route
            .maneuvers
            .iter()
            .map(|maneuver| maneuver.kind.clone())
            .collect::<std::collections::BTreeSet<_>>()
    );
    for maneuver in route.maneuvers.iter().take(10) {
        println!(
            "  [{:>12}] {:>7.1} m  {:>5.0} s  {}",
            maneuver.kind, maneuver.distance_m, maneuver.duration_s, maneuver.instruction
        );
    }
    assert!(
        named > 0,
        "the graph has {} names but no maneuver carried one",
        graph.names().len()
    );
    println!(
        "maneuvers_json: {}",
        serde_json::to_string(&route.maneuvers).expect("maneuvers serialize")
    );

    // Build the persisted graph once, then serve the same route from it.
    let cache = TempGraph::new();
    let save_started = Instant::now();
    graph.save(cache.path()).expect("graph cache should save");
    let save_s = save_started.elapsed().as_secs_f64();
    let bytes = fs::metadata(cache.path()).expect("cache metadata").len();

    let cache_load_started = Instant::now();
    let loaded = Graph::load(cache.path()).expect("graph cache should load");
    let cache_load_s = cache_load_started.elapsed().as_secs_f64();
    println!("graph cache: bytes={bytes} save_s={save_s:.2} load_s={cache_load_s:.3}");
    assert_eq!(loaded.node_count(), graph.node_count());
    assert_eq!(loaded.edge_count(), graph.edge_count());
    assert_eq!(loaded.names().len(), graph.names().len());

    let cached_started = Instant::now();
    let cached = native_router::route(&loaded, CHISINAU, BALTI, Profile::Car)
        .expect("cached graph must route");
    let cached_ms = cached_started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "cached route: distance_m={:.1} duration_s={:.1} maneuvers={} query_ms={cached_ms:.1}",
        cached.distance_m,
        cached.duration_s,
        cached.maneuvers.len()
    );
    assert!(
        (cached.distance_m - route.distance_m).abs() <= 1.0,
        "cached distance {} m vs {} m",
        cached.distance_m,
        route.distance_m
    );
    assert!(
        (cached.duration_s - route.duration_s).abs() <= 1.0,
        "cached duration {} s vs {} s",
        cached.duration_s,
        route.duration_s
    );
    assert_eq!(cached.points, route.points);
    assert_eq!(cached.nodes, route.nodes);
    assert_eq!(cached.maneuvers, route.maneuvers);
}

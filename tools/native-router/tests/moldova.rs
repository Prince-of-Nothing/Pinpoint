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
//! Nothing in this test or in the crate performs network I/O.

use std::env;
use std::time::Instant;

use native_router::{haversine_m, load_pbf, route, Profile};

const CHISINAU: (f64, f64) = (47.01, 28.86);
const BALTI: (f64, f64) = (47.76, 27.93);

#[test]
#[ignore = "requires the M1 Moldova PBF via PINPOINT_MOLDOVA_PBF"]
fn chisinau_to_balti_is_routed_locally() {
    let pbf = env::var("PINPOINT_MOLDOVA_PBF")
        .expect("set PINPOINT_MOLDOVA_PBF to dist/offline/moldova/moldova.osm.pbf");

    let load_started = Instant::now();
    let graph = load_pbf(&pbf).expect("Moldova PBF should build a car graph");
    let load_s = load_started.elapsed().as_secs_f64();
    println!(
        "graph: nodes={} edges={} load_s={load_s:.2}",
        graph.node_count(),
        graph.edge_count()
    );

    let route_started = Instant::now();
    let route =
        route(&graph, CHISINAU, BALTI, Profile::Car).expect("Chișinău–Bălți must be routable");
    let route_ms = route_started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "route: distance_m={:.1} duration_s={:.1} nodes={} points={} query_ms={route_ms:.1}",
        route.distance_m,
        route.duration_s,
        route.nodes.len(),
        route.points.len()
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
}

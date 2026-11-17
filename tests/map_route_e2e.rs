//! Browser E2E for the local routing panel on the shared map page.
//!
//! This starts the real desktop loopback [`MapServer`] and drives
//! `web/map/index.html` in headless Chromium through `web/map/test/route-e2e.mjs`.
//! By default it writes a synthetic `graph.bin`; set `PINPOINT_E2E_GRAPH` to a
//! real Moldova graph cache to exercise the production data.
//!
//! Ignored because it needs Node and Puppeteer (`NODE_PATH` in this workspace)
//! and a Chromium download:
//!
//! ```powershell
//! $env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
//! cargo test --locked --test map_route_e2e -- --ignored --nocapture
//! ```
//!
//! The graph file is never committed.

use std::path::PathBuf;
use std::process::Command;

use native_router::{haversine_m, Edge, Graph, Node, RoadClass};

const CHISINAU: (f64, f64) = (47.0105, 28.8638);
const BALTI: (f64, f64) = (47.76, 27.93);

/// A synthetic road from Chișinău to Bălți with bends and a road name.
fn synthetic_moldova_graph() -> Graph {
    let waypoints = [
        CHISINAU,
        (47.10, 28.75),
        (47.25, 28.55),
        (47.45, 28.35),
        (47.60, 28.15),
        BALTI,
    ];
    let nodes = waypoints
        .iter()
        .enumerate()
        .map(|(index, (lat, lon))| Node {
            osm_id: 1_000 + index as i64,
            lat: *lat,
            lon: *lon,
        })
        .collect::<Vec<_>>();

    let names = vec![Box::from("M1 Chișinău–Bălți")];
    let mut edges = Vec::new();
    for index in 0..waypoints.len() - 1 {
        let distance = haversine_m(waypoints[index], waypoints[index + 1]) as f32;
        for (from, to) in [(index, index + 1), (index + 1, index)] {
            edges.push(Edge {
                name: 0,
                ..Edge::new(
                    from as u32,
                    to as u32,
                    distance.max(1.0),
                    90.0,
                    RoadClass::Primary,
                )
            });
        }
    }

    Graph::from_edges_with_names(nodes, edges, names)
}

fn graph_path() -> (PathBuf, tempfile::TempDir) {
    if let Some(real) = std::env::var_os("PINPOINT_E2E_GRAPH") {
        return (PathBuf::from(real), tempfile::tempdir().expect("temp dir"));
    }

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("synthetic.graph.bin");
    synthetic_moldova_graph()
        .save(&path)
        .expect("save synthetic graph");
    (path, dir)
}

#[test]
#[ignore = "requires Node/Puppeteer and starts headless Chromium"]
fn map_page_routes_locally_in_a_browser() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let (graph, _graph_dir) = graph_path();
    assert!(
        graph.is_file(),
        "graph cache is missing: {}",
        graph.display()
    );

    // The route handler resolves this lazily on the first request.
    std::env::set_var(pinpoint_desktop::local_route::GRAPH_ENV, &graph);

    let server = pinpoint_desktop::map_server::MapServer::start(repo.join("web"))
        .expect("desktop map server starts");
    let script = repo.join("web/map/test/route-e2e.mjs");
    let node = std::env::var_os("NODE").unwrap_or_else(|| "node".into());

    let output = Command::new(node)
        .arg(&script)
        .arg(server.index_url())
        .output()
        .expect("run the Node browser test");

    println!("{}", String::from_utf8_lossy(&output.stdout));
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));

    assert!(
        output.status.success(),
        "browser E2E failed for graph {}",
        graph.display()
    );
}

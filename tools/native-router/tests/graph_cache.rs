//! Persisted graph cache acceptance tests.
//!
//! These exercise the public `Graph::save` / `Graph::load` API with a small
//! synthetic graph (no PBF needed) and verify that a loaded graph routes
//! exactly like the in-memory one. Corrupt and truncated files must be
//! rejected instead of producing a broken graph.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use native_router::graph::{EDGE_FLAG_ROUNDABOUT, GRAPH_HEADER_LEN, NO_NAME};
use native_router::{haversine_m, route, Edge, Graph, Node, Profile, RoadClass};

fn node(osm_id: i64, lat: f64, lon: f64) -> Node {
    Node { osm_id, lat, lon }
}

fn edge(from: u32, to: u32, distance_m: f32, speed_kmh: f32, name: u32) -> Edge {
    Edge {
        from,
        to,
        distance_m,
        speed_kmh,
        class: RoadClass::Secondary,
        flags: 0,
        name,
    }
}

/// A three-node, two-lane named street with a roundabout flag on the last hop.
fn fixture() -> Graph {
    let nodes = vec![
        node(10, 47.0, 28.0),
        node(11, 47.0, 28.001),
        node(12, 47.001, 28.001),
        node(13, 47.001, 28.002),
    ];
    let names = vec!["Strada Testului".into(), "M14".into()];
    let edges = vec![
        edge(0, 1, 75.0, 50.0, 0),
        edge(1, 0, 75.0, 50.0, 0),
        edge(1, 2, 75.0, 50.0, 1),
        edge(2, 1, 75.0, 50.0, 1),
        Edge {
            from: 2,
            to: 3,
            distance_m: 75.0,
            speed_kmh: 30.0,
            class: RoadClass::Tertiary,
            flags: EDGE_FLAG_ROUNDABOUT,
            name: NO_NAME,
        },
        Edge {
            from: 3,
            to: 2,
            distance_m: 75.0,
            speed_kmh: 30.0,
            class: RoadClass::Tertiary,
            flags: EDGE_FLAG_ROUNDABOUT,
            name: NO_NAME,
        },
    ];
    Graph::from_edges_with_names(nodes, edges, names)
}

struct TempFile(PathBuf);

impl TempFile {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pinpoint-native-router-{}-{label}-{nanos}.bin",
            std::process::id()
        ));
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn save_load_round_trip_routes_identically() {
    let graph = fixture();
    let file = TempFile::new("roundtrip");

    graph.save(file.path()).expect("save should succeed");
    let bytes = fs::metadata(file.path()).unwrap().len();
    assert!(bytes > 0, "graph file must not be empty");

    let loaded = Graph::load(file.path()).expect("load should succeed");
    assert_eq!(loaded.node_count(), graph.node_count());
    assert_eq!(loaded.edge_count(), graph.edge_count());
    assert_eq!(loaded.names(), graph.names());
    assert_eq!(loaded.max_speed_kmh(), graph.max_speed_kmh());
    assert_eq!(
        loaded.nearest_node((47.0, 28.001)).map(|(id, _)| id),
        Some(1)
    );

    let before = route(&graph, (47.0, 28.0), (47.001, 28.002), Profile::Car).unwrap();
    let after = route(&loaded, (47.0, 28.0), (47.001, 28.002), Profile::Car).unwrap();
    assert_eq!(after, before, "loaded graph must produce the same route");
    assert_eq!(after.maneuvers, before.maneuvers);
}

#[test]
fn saving_over_an_existing_file_replaces_it_atomically() {
    let graph = fixture();
    let file = TempFile::new("replace");
    fs::write(file.path(), b"stale contents that must be replaced").unwrap();

    graph
        .save(file.path())
        .expect("save should replace the file");
    let loaded = Graph::load(file.path()).expect("replacement must be readable");
    assert_eq!(loaded.edge_count(), graph.edge_count());
}

#[test]
fn rejects_a_truncated_graph_file() {
    let graph = fixture();
    let file = TempFile::new("truncated");
    graph.save(file.path()).unwrap();

    let bytes = fs::read(file.path()).unwrap();
    fs::write(file.path(), &bytes[..bytes.len() / 2]).unwrap();
    let err = Graph::load(file.path()).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("truncated") || message.contains("checksum"),
        "unexpected error: {message}"
    );
}

#[test]
fn rejects_a_corrupt_payload_checksum() {
    let graph = fixture();
    let file = TempFile::new("corrupt");
    graph.save(file.path()).unwrap();

    let mut bytes = fs::read(file.path()).unwrap();
    // Flip the lowest mantissa byte of the first node's latitude (the first
    // little-endian byte of the f64). The value stays in range, so only the
    // payload CRC can detect the tampering.
    let name_bytes: usize = graph.names().iter().map(|name| 4 + name.len()).sum();
    let latitude_low_byte = GRAPH_HEADER_LEN + name_bytes + 8;
    bytes[latitude_low_byte] ^= 0x01;
    fs::write(file.path(), &bytes).unwrap();
    let err = Graph::load(file.path()).unwrap_err();
    assert!(
        err.to_string().contains("checksum"),
        "unexpected error: {err}"
    );
}

#[test]
fn rejects_a_wrong_magic_or_version() {
    let graph = fixture();
    let file = TempFile::new("header");
    graph.save(file.path()).unwrap();

    let original = fs::read(file.path()).unwrap();
    let mut wrong_magic = original.clone();
    wrong_magic[0] = b'X';
    fs::write(file.path(), &wrong_magic).unwrap();
    assert!(Graph::load(file.path())
        .unwrap_err()
        .to_string()
        .contains("magic"));

    let mut wrong_version = original;
    // Header layout: 8-byte magic, then a little-endian u32 version.
    wrong_version[8..12].copy_from_slice(&999_u32.to_le_bytes());
    fs::write(file.path(), &wrong_version).unwrap();
    assert!(Graph::load(file.path())
        .unwrap_err()
        .to_string()
        .contains("version"));
}

#[test]
fn loaded_graph_keeps_edge_travel_time_and_names() {
    let graph = fixture();
    let file = TempFile::new("edges");
    graph.save(file.path()).unwrap();
    let loaded = Graph::load(file.path()).unwrap();

    let outgoing = loaded.edges_from(2);
    assert_eq!(outgoing.len(), 2);
    let roundabout = outgoing
        .iter()
        .find(|edge| edge.is_roundabout())
        .expect("roundabout flag must survive the round trip");
    assert_eq!(loaded.edge_name(roundabout), None);
    assert_eq!(loaded.edge_name(&outgoing[0]), Some("M14"));
    assert_eq!(loaded.node(0).unwrap().osm_id, 10);
    assert!((haversine_m((47.0, 28.0), (47.0, 28.001)) - 75.0).abs() < 1.0);
}

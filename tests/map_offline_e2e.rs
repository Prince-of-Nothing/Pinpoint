//! Browser E2E for the native offline pack install.
//!
//! Starts the real desktop loopback [`MapServer`] with a temporary `packs/`
//! directory and a local artifact host that streams the committed Monaco
//! PMTiles fixture plus a gzip-compressed synthetic `native_router` graph. The
//! page clicks the **Offline map** download button and the test asserts
//! progress, the installed state, `/map/config`, the reload onto
//! `/packs/moldova.pmtiles`, and a local route from the downloaded graph.
//!
//! Ignored because it needs Node, Puppeteer (`NODE_PATH`) and Chromium:
//!
//! ```powershell
//! $env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
//! cargo test --locked --test map_offline_e2e -- --ignored --nocapture
//! ```
//!
//! The 139 MB transfer (204 MB installed) and temporary packs are never
//! committed.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use native_router::{haversine_m, Edge, Graph, Node, RoadClass};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

use pinpoint_desktop::offline_install::{DownloadPlan, GRAPH_FILE, GRAPH_GZ_FILE, PACK_FILE};

const CHISINAU: (f64, f64) = (47.0105, 28.8638);
const BALTI: (f64, f64) = (47.76, 27.93);

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

fn gzip(bytes: &[u8]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use std::io::Write as _;

    let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("gzip synthetic graph");
    encoder.finish().expect("gzip synthetic graph")
}

struct ArtifactHost {
    base_url: String,
    /// Keeps the fixture server's runtime alive for the whole test.
    _runtime: tokio::runtime::Runtime,
}

struct HostState {
    pack: Vec<u8>,
    graph_gz: Vec<u8>,
    delay: Duration,
}

/// A local stand-in for the GitHub data release. Bodies stream in 1 KiB
/// chunks so the page has time to render a real progress bar.
fn artifact_host(pack: Vec<u8>, graph_gz: Vec<u8>) -> ArtifactHost {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("artifact-host runtime");

    let state = Arc::new(HostState {
        pack,
        graph_gz,
        delay: Duration::from_millis(20),
    });
    let app = Router::new()
        .route("/:file", get(artifact))
        .with_state(state);
    let base_url = runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("artifact host bind");
        let address = listener.local_addr().expect("artifact host address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{address}")
    });

    ArtifactHost {
        base_url,
        _runtime: runtime,
    }
}

async fn artifact(State(state): State<Arc<HostState>>, Path(name): Path<String>) -> Response {
    let bytes = match name.as_str() {
        PACK_FILE => state.pack.clone(),
        GRAPH_GZ_FILE => state.graph_gz.clone(),
        _ => return (StatusCode::NOT_FOUND, "Not Found").into_response(),
    };
    let total = bytes.len() as u64;
    let delay = state.delay;
    let (mut writer, reader) = tokio::io::duplex(16 * 1024);
    tokio::spawn(async move {
        for chunk in bytes.chunks(1024) {
            if writer.write_all(chunk).await.is_err() {
                return;
            }
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
        }
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, total)
        .body(Body::from_stream(ReaderStream::new(reader)))
        .expect("artifact response")
}

#[test]
#[ignore = "requires Node/Puppeteer and starts headless Chromium"]
fn downloads_and_installs_the_offline_pack_from_the_page() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    std::env::remove_var(pinpoint_desktop::local_route::GRAPH_ENV);

    let pack = std::fs::read(repo.join("web/map/fixtures/monaco.pmtiles"))
        .expect("committed Monaco PMTiles fixture");
    let graph_dir = tempfile::tempdir().expect("graph temp dir");
    let graph_path = graph_dir.path().join("synthetic.graph.bin");
    synthetic_moldova_graph()
        .save(&graph_path)
        .expect("save synthetic graph");
    let graph = std::fs::read(&graph_path).expect("read synthetic graph");
    let graph_gz = gzip(&graph);

    let host = artifact_host(pack.clone(), graph_gz.clone());
    let plan = DownloadPlan::fixture_gzip(&host.base_url, &pack, &graph_gz, &graph);

    let packs = tempfile::tempdir().expect("packs temp dir");
    let server = pinpoint_desktop::map_server::MapServer::start_with_plan(
        repo.join("web"),
        Some(packs.path().to_path_buf()),
        plan,
    )
    .expect("desktop map server starts");

    let script = repo.join("web/map/test/offline-e2e.mjs");
    let node = std::env::var_os("NODE").unwrap_or_else(|| "node".into());
    let output = Command::new(node)
        .arg(&script)
        .arg(server.index_url())
        .output()
        .expect("run the Node offline E2E");

    println!("{}", String::from_utf8_lossy(&output.stdout));
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));

    assert!(
        output.status.success(),
        "offline install browser E2E failed (packs {})",
        packs.path().display()
    );
    assert!(
        packs.path().join(PACK_FILE).is_file(),
        "the PMTiles pack was installed"
    );
    assert!(
        packs.path().join(GRAPH_FILE).is_file(),
        "the routing graph was installed"
    );
    assert_eq!(
        std::fs::read(packs.path().join(GRAPH_FILE)).expect("installed graph"),
        graph,
        "the installed graph is the decompressed transfer"
    );
    assert!(
        !packs.path().join(GRAPH_GZ_FILE).exists(),
        "the gzip transfer is not installed"
    );
    drop(host);
}

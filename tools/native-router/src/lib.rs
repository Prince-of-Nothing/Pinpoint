//! Local, on-device car routing for Pinpoint 2.0.
//!
//! This crate is a routing-engine spike. It reads an OpenStreetMap PBF extract,
//! builds a directed car road graph with interned road names, persists that
//! graph in a compact binary cache and answers routes with A*. Routes carry
//! turn-by-turn maneuvers generated from the routed geometry and edge names.
//! It makes no network calls: the PBF or the graph cache is the only input.
//!
//! See `README.md` for the graph and JSON formats and `ANDROID.md` for the
//! Android build path.

pub mod graph;
pub mod maneuver;
pub mod pbf;
pub mod profile;
pub mod router;

pub use graph::{
    haversine_m, Edge, Graph, GraphError, Node, RoadClass, EDGE_FLAG_ROUNDABOUT,
    GRAPH_FORMAT_VERSION, GRAPH_MAGIC, NO_NAME,
};
pub use maneuver::Maneuver;
pub use pbf::{load_pbf, PbfError};
pub use profile::Profile;
pub use router::{route, Route, RouteError};

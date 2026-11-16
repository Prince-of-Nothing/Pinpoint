//! Local, on-device car routing for Pinpoint 2.0.
//!
//! This crate is a routing-engine spike. It reads an OpenStreetMap PBF extract,
//! builds a directed car road graph in memory and answers routes with A*. It
//! makes no network calls: the PBF is the only input and the graph never leaves
//! the process.
//!
//! See `README.md` for the graph format and `ANDROID.md` for the Android build
//! path.

pub mod graph;
pub mod pbf;
pub mod profile;
pub mod router;

pub use graph::{haversine_m, Edge, Graph, Node, RoadClass};
pub use pbf::{load_pbf, PbfError};
pub use profile::Profile;
pub use router::{route, Route, RouteError};

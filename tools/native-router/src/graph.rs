//! Graph data structures and the documented on-disk/in-memory format.
//!
//! Node coordinates are WGS84 degrees. Edges are directed; the PBF builder
//! emits one edge per travel direction. The graph is stored as:
//!
//! - `nodes`: a flat `Vec<Node>`, indexed by `u32` graph node id.
//! - `edges`: a flat `Vec<Edge>` grouped by source node.
//! - `offsets`: `offsets[i]..offsets[i + 1]` is the slice of `edges` leaving
//!   node `i`.
//!
//! There are no turn restrictions, turn costs, lane data or traffic data in
//! this spike.

use std::cmp::Ordering;

/// Mean Earth radius used for all distance computations.
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Great-circle distance in metres between two `(lat, lon)` pairs in degrees.
pub fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (lat1, lon1) = (a.0.to_radians(), a.1.to_radians());
    let (lat2, lon2) = (b.0.to_radians(), b.1.to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().asin()
}

/// Road classes retained by the car profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoadClass {
    Motorway,
    MotorwayLink,
    Trunk,
    TrunkLink,
    Primary,
    PrimaryLink,
    Secondary,
    SecondaryLink,
    Tertiary,
    TertiaryLink,
    Unclassified,
    Residential,
    LivingStreet,
    Service,
    Road,
}

impl RoadClass {
    /// Default travel speed when the way has no usable `maxspeed` tag.
    ///
    /// These are routing estimates, not legal limits.
    pub fn default_speed_kmh(self) -> f32 {
        match self {
            RoadClass::Motorway => 110.0,
            RoadClass::MotorwayLink => 70.0,
            RoadClass::Trunk => 100.0,
            RoadClass::TrunkLink => 60.0,
            RoadClass::Primary => 90.0,
            RoadClass::PrimaryLink => 60.0,
            RoadClass::Secondary => 80.0,
            RoadClass::SecondaryLink => 50.0,
            RoadClass::Tertiary => 70.0,
            RoadClass::TertiaryLink => 50.0,
            RoadClass::Unclassified => 60.0,
            RoadClass::Residential => 50.0,
            RoadClass::LivingStreet => 20.0,
            RoadClass::Service => 20.0,
            RoadClass::Road => 50.0,
        }
    }

    /// Stable lowercase name used by docs and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            RoadClass::Motorway => "motorway",
            RoadClass::MotorwayLink => "motorway_link",
            RoadClass::Trunk => "trunk",
            RoadClass::TrunkLink => "trunk_link",
            RoadClass::Primary => "primary",
            RoadClass::PrimaryLink => "primary_link",
            RoadClass::Secondary => "secondary",
            RoadClass::SecondaryLink => "secondary_link",
            RoadClass::Tertiary => "tertiary",
            RoadClass::TertiaryLink => "tertiary_link",
            RoadClass::Unclassified => "unclassified",
            RoadClass::Residential => "residential",
            RoadClass::LivingStreet => "living_street",
            RoadClass::Service => "service",
            RoadClass::Road => "road",
        }
    }
}

/// One OSM node kept in the routing graph.
#[derive(Debug, Clone, Copy)]
pub struct Node {
    /// Original OSM node id, kept so routes can be inspected and tests can
    /// assert on real-world data.
    pub osm_id: i64,
    pub lat: f64,
    pub lon: f64,
}

/// One directed graph edge.
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    /// Great-circle length in metres.
    pub distance_m: f32,
    /// Effective car speed for this edge, already clamped to the profile.
    pub speed_kmh: f32,
    pub class: RoadClass,
}

/// Directed car road graph in compressed-sparse-row form.
#[derive(Debug, Default)]
pub struct Graph {
    nodes: Vec<Node>,
    offsets: Vec<u32>,
    edges: Vec<Edge>,
    /// Maximum edge speed, used for the A* heuristic.
    max_speed_kmh: f32,
}

impl Graph {
    /// Build a graph from nodes and directed edges.
    ///
    /// Invalid endpoints, non-positive distances or non-positive speeds panic:
    /// they indicate a bug in the PBF builder or in a test fixture, never user
    /// input.
    pub fn from_edges(nodes: Vec<Node>, edges: Vec<Edge>) -> Self {
        let node_count = nodes.len();
        assert!(
            node_count <= u32::MAX as usize,
            "graph has more nodes than u32 allows"
        );

        let mut offsets = vec![0u32; node_count + 1];
        for edge in &edges {
            assert!(
                (edge.from as usize) < node_count && (edge.to as usize) < node_count,
                "edge endpoint outside the node table"
            );
            assert!(
                edge.distance_m.is_finite() && edge.distance_m >= 0.0,
                "edge distance must be finite and non-negative"
            );
            assert!(
                edge.speed_kmh.is_finite() && edge.speed_kmh > 0.0,
                "edge speed must be finite and positive"
            );
            offsets[edge.from as usize + 1] += 1;
        }
        for i in 0..node_count {
            offsets[i + 1] += offsets[i];
        }

        let mut cursor = offsets.clone();
        let mut sorted = vec![
            Edge {
                from: 0,
                to: 0,
                distance_m: 0.0,
                speed_kmh: 1.0,
                class: RoadClass::Road,
            };
            edges.len()
        ];
        for edge in edges {
            let slot = &mut cursor[edge.from as usize];
            sorted[*slot as usize] = edge;
            *slot += 1;
        }

        let max_speed_kmh = sorted
            .iter()
            .map(|edge| edge.speed_kmh)
            .fold(0.0_f32, f32::max)
            .max(1.0);

        Graph {
            nodes,
            offsets,
            edges: sorted,
            max_speed_kmh,
        }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn max_speed_kmh(&self) -> f32 {
        self.max_speed_kmh
    }

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.get(id as usize)
    }

    /// Directed edges leaving `node`.
    pub fn edges_from(&self, node: u32) -> &[Edge] {
        let i = node as usize;
        if i + 1 >= self.offsets.len() {
            return &[];
        }
        &self.edges[self.offsets[i] as usize..self.offsets[i + 1] as usize]
    }

    /// Nearest graph node to a `(lat, lon)` pair, by great-circle distance.
    ///
    /// Linear scan: for a Moldova-sized graph this is a few milliseconds and
    /// keeps the spike free of a spatial index.
    pub fn nearest_node(&self, latlon: (f64, f64)) -> Option<(u32, f64)> {
        let mut best: Option<(u32, f64)> = None;
        for (idx, node) in self.nodes.iter().enumerate() {
            let distance = haversine_m(latlon, (node.lat, node.lon));
            if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((idx as u32, distance));
            }
        }
        best
    }
}

/// Total order for heap entries whose `f64` scores are never NaN.
pub(crate) fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.total_cmp(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(osm_id: i64, lat: f64, lon: f64) -> Node {
        Node { osm_id, lat, lon }
    }

    #[test]
    fn haversine_matches_known_degree_length() {
        // One degree of latitude is about 111.2 km on the mean sphere.
        let d = haversine_m((47.0, 28.0), (48.0, 28.0));
        assert!((d - 111_195.0).abs() < 200.0, "got {d}");
    }

    #[test]
    fn builds_csr_offsets_and_reports_degrees() {
        let nodes = vec![
            node(1, 47.0, 28.0),
            node(2, 47.0, 28.001),
            node(3, 47.0, 28.002),
        ];
        let edges = vec![
            Edge {
                from: 0,
                to: 1,
                distance_m: 75.0,
                speed_kmh: 50.0,
                class: RoadClass::Residential,
            },
            Edge {
                from: 0,
                to: 2,
                distance_m: 150.0,
                speed_kmh: 50.0,
                class: RoadClass::Residential,
            },
            Edge {
                from: 1,
                to: 2,
                distance_m: 75.0,
                speed_kmh: 50.0,
                class: RoadClass::Residential,
            },
        ];
        let graph = Graph::from_edges(nodes, edges);
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 3);
        assert_eq!(graph.edges_from(0).len(), 2);
        assert_eq!(graph.edges_from(1).len(), 1);
        assert_eq!(graph.edges_from(2).len(), 0);
        assert_eq!(graph.max_speed_kmh(), 50.0);
    }

    #[test]
    fn nearest_node_finds_closest_and_reports_distance() {
        let nodes = vec![node(1, 47.0, 28.0), node(2, 47.1, 28.1)];
        let graph = Graph::from_edges(nodes, vec![]);
        let (id, distance) = graph.nearest_node((47.001, 28.001)).unwrap();
        assert_eq!(id, 0);
        assert!(distance < 200.0, "got {distance}");
    }

    #[test]
    fn nearest_node_on_empty_graph_is_none() {
        let graph = Graph::from_edges(vec![], vec![]);
        assert!(graph.nearest_node((47.0, 28.0)).is_none());
    }
}

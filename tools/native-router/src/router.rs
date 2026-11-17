//! A* car routing over [`Graph`].

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt;

use serde::Serialize;

use crate::graph::{cmp_f64, haversine_m, Graph, Node};
use crate::maneuver::{build_maneuvers, Maneuver};
use crate::profile::Profile;

/// Queries further than this from the nearest graph node are reported as
/// outside coverage instead of silently snapping to a distant road.
pub const MAX_SNAP_M: f64 = 25_000.0;

/// A computed route. `points` are `(lat, lon)` WGS84 pairs in travel order;
/// `nodes` are the matching OSM node ids. `maneuvers` is the turn-by-turn
/// instruction list for the same path, starting with `depart` at the snapped
/// origin and ending with `arrive`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Route {
    pub distance_m: f64,
    pub duration_s: f64,
    pub points: Vec<(f64, f64)>,
    pub nodes: Vec<i64>,
    pub maneuvers: Vec<Maneuver>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RouteError {
    EmptyGraph,
    OutsideCoverage { snap_m: f64 },
    NoPath,
}

impl fmt::Display for RouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteError::EmptyGraph => write!(f, "routing graph is empty"),
            RouteError::OutsideCoverage { snap_m } => write!(
                f,
                "origin or destination is outside the offline area (nearest road {snap_m:.0} m away)"
            ),
            RouteError::NoPath => write!(f, "no car route found between the snapped nodes"),
        }
    }
}

impl std::error::Error for RouteError {}

/// Route between two `(lat, lon)` pairs using `profile`.
///
/// The origin and destination are snapped to the nearest graph node. The
/// returned distance and duration include the graph path only, not the
/// connector from the raw coordinate to the snapped node.
pub fn route(
    graph: &Graph,
    from_latlon: (f64, f64),
    to_latlon: (f64, f64),
    profile: Profile,
) -> Result<Route, RouteError> {
    let (start, start_snap_m) = graph
        .nearest_node(from_latlon)
        .ok_or(RouteError::EmptyGraph)?;
    let (goal, goal_snap_m) = graph
        .nearest_node(to_latlon)
        .ok_or(RouteError::EmptyGraph)?;
    let worst_snap_m = start_snap_m.max(goal_snap_m);
    if worst_snap_m > MAX_SNAP_M {
        return Err(RouteError::OutsideCoverage {
            snap_m: worst_snap_m,
        });
    }

    if start == goal {
        let node = graph.node(start).expect("nearest_node returns a valid id");
        let points = vec![node_latlon(node)];
        return Ok(Route {
            distance_m: 0.0,
            duration_s: 0.0,
            maneuvers: build_maneuvers(graph, &points, &[], profile),
            points,
            nodes: vec![node.osm_id],
        });
    }

    let node_count = graph.node_count();
    let mut best_time = vec![f64::INFINITY; node_count];
    let mut best_distance = vec![0.0_f64; node_count];
    let mut came_from = vec![u32::MAX; node_count];
    let mut came_from_edge = vec![u32::MAX; node_count];
    let mut closed = vec![false; node_count];
    let mut queue = BinaryHeap::new();

    let heuristic_speed_mps = f64::from(profile.max_speed_kmh().max(1.0)) / 3.6;
    let goal_latlon = node_latlon(graph.node(goal).expect("goal id is valid"));
    let heuristic = |node: u32| {
        let node = graph.node(node).expect("queue holds valid ids");
        haversine_m(node_latlon(node), goal_latlon) / heuristic_speed_mps
    };

    best_time[start as usize] = 0.0;
    queue.push(QueueEntry {
        f: heuristic(start),
        g: 0.0,
        node: start,
    });

    while let Some(entry) = queue.pop() {
        let current = entry.node as usize;
        if closed[current] {
            continue;
        }
        // Skip stale heap entries left behind by a cheaper relaxation.
        if entry.g > best_time[current] {
            continue;
        }
        closed[current] = true;

        if entry.node == goal {
            let mut path = Vec::new();
            let mut path_edges = Vec::new();
            let mut cursor = goal;
            loop {
                path.push(cursor);
                if cursor == start {
                    break;
                }
                let via = came_from_edge[cursor as usize];
                if via == u32::MAX {
                    return Err(RouteError::NoPath);
                }
                path_edges.push(via);
                cursor = came_from[cursor as usize];
                if cursor == u32::MAX {
                    return Err(RouteError::NoPath);
                }
            }
            path.reverse();
            path_edges.reverse();

            let mut points = Vec::with_capacity(path.len());
            let mut nodes = Vec::with_capacity(path.len());
            for id in path {
                let node = graph.node(id).expect("reconstructed path holds valid ids");
                points.push(node_latlon(node));
                nodes.push(node.osm_id);
            }
            let maneuvers = build_maneuvers(graph, &points, &path_edges, profile);
            return Ok(Route {
                distance_m: best_distance[goal as usize],
                duration_s: best_time[goal as usize],
                points,
                nodes,
                maneuvers,
            });
        }

        let speed_cap_kmh = f64::from(profile.max_speed_kmh().max(1.0));
        let edge_base = graph.edge_index_base(entry.node);
        for (offset, edge) in graph.edges_from(entry.node).iter().enumerate() {
            let next = edge.to as usize;
            if closed[next] {
                continue;
            }
            let speed_kmh = f64::from(edge.speed_kmh).min(speed_cap_kmh);
            let edge_time_s = f64::from(edge.distance_m) / (speed_kmh / 3.6);
            let next_time = entry.g + edge_time_s;
            if next_time < best_time[next] {
                best_time[next] = next_time;
                best_distance[next] = best_distance[current] + f64::from(edge.distance_m);
                came_from[next] = entry.node;
                came_from_edge[next] = edge_base + offset as u32;
                queue.push(QueueEntry {
                    f: next_time + heuristic(edge.to),
                    g: next_time,
                    node: edge.to,
                });
            }
        }
    }

    Err(RouteError::NoPath)
}

#[derive(Debug, Clone, Copy)]
struct QueueEntry {
    f: f64,
    g: f64,
    node: u32,
}

impl PartialEq for QueueEntry {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
            && cmp_f64(self.f, other.f) == Ordering::Equal
            && cmp_f64(self.g, other.g) == Ordering::Equal
    }
}

impl Eq for QueueEntry {}

impl PartialOrd for QueueEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // `BinaryHeap` is a max-heap: reverse `f` so the smallest score pops
        // first, and prefer larger `g` on ties (closer to the goal).
        cmp_f64(other.f, self.f)
            .then_with(|| cmp_f64(self.g, other.g))
            .then_with(|| other.node.cmp(&self.node))
    }
}

fn node_latlon(node: &Node) -> (f64, f64) {
    (node.lat, node.lon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, RoadClass, EDGE_FLAG_ROUNDABOUT};
    use crate::maneuver::{
        KIND_ARRIVE, KIND_DEPART, KIND_LEFT, KIND_RIGHT, KIND_ROUNDABOUT, KIND_STRAIGHT,
    };

    fn node(osm_id: i64, lat: f64, lon: f64) -> Node {
        Node { osm_id, lat, lon }
    }

    fn edge(from: u32, to: u32, distance_m: f32, speed_kmh: f32) -> Edge {
        Edge::new(from, to, distance_m, speed_kmh, RoadClass::Residential)
    }

    fn named(from: u32, to: u32, distance_m: f32, name: u32) -> Edge {
        Edge {
            name,
            ..edge(from, to, distance_m, 50.0)
        }
    }

    fn roundabout(from: u32, to: u32, distance_m: f32) -> Edge {
        Edge {
            flags: EDGE_FLAG_ROUNDABOUT,
            ..edge(from, to, distance_m, 30.0)
        }
    }

    fn straight_line_graph() -> Graph {
        Graph::from_edges(
            vec![
                node(101, 47.0, 28.0),
                node(102, 47.0, 28.001),
                node(103, 47.0, 28.002),
            ],
            vec![
                edge(0, 1, 75.0, 50.0),
                edge(1, 0, 75.0, 50.0),
                edge(1, 2, 75.0, 50.0),
                edge(2, 1, 75.0, 50.0),
            ],
        )
    }

    #[test]
    fn routes_straight_line_with_distance_and_duration() {
        let graph = straight_line_graph();
        let route = route(&graph, (47.0, 28.0), (47.0, 28.002), Profile::Car).unwrap();
        assert!(
            (route.distance_m - 150.0).abs() < 0.01,
            "got {}",
            route.distance_m
        );
        // 150 m at 50 km/h = 10.8 s.
        assert!(
            (route.duration_s - 10.8).abs() < 0.01,
            "got {}",
            route.duration_s
        );
        assert_eq!(route.nodes, vec![101, 102, 103]);
        assert_eq!(route.points.len(), 3);
        assert_eq!(route.points[0], (47.0, 28.0));
        assert_eq!(route.points[2], (47.0, 28.002));
    }

    #[test]
    fn respects_oneway_and_reports_no_path() {
        let graph = Graph::from_edges(
            vec![node(1, 47.0, 28.0), node(2, 47.0, 28.001)],
            vec![edge(0, 1, 75.0, 50.0)],
        );
        assert!(route(&graph, (47.0, 28.0), (47.0, 28.001), Profile::Car).is_ok());
        assert_eq!(
            route(&graph, (47.0, 28.001), (47.0, 28.0), Profile::Car),
            Err(RouteError::NoPath)
        );
    }

    #[test]
    fn picks_faster_route_over_shorter_slow_route() {
        // A direct 500 m / 20 km/h edge versus a 1800 m / 100 km/h detour.
        let graph = Graph::from_edges(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.0, 28.002),
                node(4, 47.0, 28.003),
            ],
            vec![
                edge(0, 3, 500.0, 20.0),
                edge(0, 1, 900.0, 100.0),
                edge(1, 3, 900.0, 100.0),
            ],
        );
        let route = route(&graph, (47.0, 28.0), (47.0, 28.003), Profile::Car).unwrap();
        assert!(
            (route.distance_m - 1800.0).abs() < 0.01,
            "got {}",
            route.distance_m
        );
        // 1800 m at 100 km/h = 64.8 s.
        assert!(
            (route.duration_s - 64.8).abs() < 0.01,
            "got {}",
            route.duration_s
        );
        assert_eq!(route.nodes, vec![1, 2, 4]);
    }

    #[test]
    fn snaps_off_graph_coordinates_to_nearest_node() {
        let graph = straight_line_graph();
        let route = route(&graph, (47.0007, 28.0006), (47.0, 28.002), Profile::Car).unwrap();
        assert_eq!(route.nodes.first(), Some(&102));
    }

    #[test]
    fn same_node_route_is_zero_length() {
        let graph = straight_line_graph();
        let route = route(&graph, (47.0, 28.0), (47.0, 28.0), Profile::Car).unwrap();
        assert_eq!(route.distance_m, 0.0);
        assert_eq!(route.duration_s, 0.0);
        assert_eq!(route.nodes, vec![101]);
        assert_eq!(route.points, vec![(47.0, 28.0)]);
        assert_eq!(route.maneuvers.len(), 2);
        assert_eq!(route.maneuvers[0].kind, KIND_DEPART);
        assert_eq!(route.maneuvers[1].kind, KIND_ARRIVE);
        assert_eq!(route.maneuvers[0].distance_m, 0.0);
    }

    #[test]
    fn disconnected_components_report_no_path() {
        let graph = Graph::from_edges(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.1, 28.1),
                node(4, 47.1, 28.101),
            ],
            vec![edge(0, 1, 75.0, 50.0), edge(2, 3, 75.0, 50.0)],
        );
        assert_eq!(
            route(&graph, (47.0, 28.0), (47.1, 28.1), Profile::Car),
            Err(RouteError::NoPath)
        );
    }

    #[test]
    fn far_queries_report_outside_coverage() {
        let graph = straight_line_graph();
        let err = route(&graph, (48.5, 29.5), (47.0, 28.002), Profile::Car).unwrap_err();
        assert!(
            matches!(err, RouteError::OutsideCoverage { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_graph_is_reported() {
        let graph = Graph::from_edges(vec![], vec![]);
        assert_eq!(
            route(&graph, (47.0, 28.0), (47.0, 28.001), Profile::Car),
            Err(RouteError::EmptyGraph)
        );
    }

    #[test]
    fn queue_orders_by_lowest_f_then_highest_g() {
        let mut heap = BinaryHeap::new();
        heap.push(QueueEntry {
            f: 10.0,
            g: 5.0,
            node: 1,
        });
        heap.push(QueueEntry {
            f: 5.0,
            g: 1.0,
            node: 2,
        });
        heap.push(QueueEntry {
            f: 5.0,
            g: 3.0,
            node: 3,
        });
        assert_eq!(heap.pop().unwrap().node, 3);
        assert_eq!(heap.pop().unwrap().node, 2);
        assert_eq!(heap.pop().unwrap().node, 1);
    }

    // -- maneuvers ---------------------------------------------------------

    #[test]
    fn straight_route_departs_then_arrives_without_turn_spam() {
        let graph = straight_line_graph();
        let route = route(&graph, (47.0, 28.0), (47.0, 28.002), Profile::Car).unwrap();
        assert_eq!(route.maneuvers.len(), 2, "{:#?}", route.maneuvers);
        assert_eq!(route.maneuvers[0].kind, KIND_DEPART);
        assert_eq!(route.maneuvers[0].instruction, "Head east");
        assert_eq!(route.maneuvers[0].point_index, 0);
        assert!((route.maneuvers[0].distance_m - 150.0).abs() < 0.01);
        assert_eq!(route.maneuvers[1].kind, KIND_ARRIVE);
        assert_eq!(route.maneuvers[1].distance_m, 0.0);
        assert_eq!(route.maneuvers[1].duration_s, 0.0);
        assert_eq!(route.maneuvers[1].point_index, 2);
    }

    #[test]
    fn left_turn_uses_outgoing_road_name() {
        // East along Strada A, then north onto Strada B: a left turn.
        let graph = Graph::from_edges_with_names(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.001, 28.001),
            ],
            vec![named(0, 1, 75.0, 0), named(1, 2, 75.0, 1)],
            vec!["Strada A".into(), "Strada B".into()],
        );
        let route = route(&graph, (47.0, 28.0), (47.001, 28.001), Profile::Car).unwrap();
        let turn = route
            .maneuvers
            .iter()
            .find(|maneuver| maneuver.kind == KIND_LEFT)
            .expect("expected a left maneuver");
        assert_eq!(turn.instruction, "Turn left onto Strada B");
        assert_eq!(turn.point, (47.0, 28.001));
        assert!(
            (turn.distance_m - 75.0).abs() < 0.5,
            "got {}",
            turn.distance_m
        );
        assert!(
            (turn.duration_s - 5.4).abs() < 0.1,
            "got {}",
            turn.duration_s
        );
    }

    #[test]
    fn right_turn_uses_outgoing_road_name() {
        // East, then south: a right turn.
        let graph = Graph::from_edges_with_names(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 46.999, 28.001),
            ],
            vec![named(0, 1, 75.0, 0), named(1, 2, 75.0, 1)],
            vec!["Strada A".into(), "Strada B".into()],
        );
        let route = route(&graph, (47.0, 28.0), (46.999, 28.001), Profile::Car).unwrap();
        let turn = route
            .maneuvers
            .iter()
            .find(|maneuver| maneuver.kind == KIND_RIGHT)
            .expect("expected a right maneuver");
        assert_eq!(turn.instruction, "Turn right onto Strada B");
    }

    #[test]
    fn turn_without_a_name_falls_back_to_the_bare_instruction() {
        let graph = Graph::from_edges(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.001, 28.001),
            ],
            vec![edge(0, 1, 75.0, 50.0), edge(1, 2, 75.0, 50.0)],
        );
        let route = route(&graph, (47.0, 28.0), (47.001, 28.001), Profile::Car).unwrap();
        let turn = route
            .maneuvers
            .iter()
            .find(|maneuver| maneuver.kind == KIND_LEFT)
            .expect("expected a left maneuver");
        assert_eq!(turn.instruction, "Turn left");
    }

    #[test]
    fn name_change_inside_the_dead_band_emits_straight() {
        let graph = Graph::from_edges_with_names(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.0, 28.002),
            ],
            vec![named(0, 1, 75.0, 0), named(1, 2, 75.0, 1)],
            vec!["Strada A".into(), "Strada B".into()],
        );
        let route = route(&graph, (47.0, 28.0), (47.0, 28.002), Profile::Car).unwrap();
        let straight = route
            .maneuvers
            .iter()
            .find(|maneuver| maneuver.kind == KIND_STRAIGHT)
            .expect("expected a straight maneuver");
        assert_eq!(straight.instruction, "Continue onto Strada B");
        assert_eq!(route.maneuvers.len(), 3);
    }

    #[test]
    fn noisy_geometry_inside_the_dead_band_does_not_add_turns() {
        // A named road with ±4° zig-zags (≈8° per turn): below the 15° dead
        // band, same name.
        let graph = Graph::from_edges_with_names(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.00005, 28.001),
                node(3, 47.0, 28.002),
                node(4, 47.00005, 28.003),
                node(5, 47.0, 28.004),
            ],
            vec![
                named(0, 1, 75.0, 0),
                named(1, 2, 75.0, 0),
                named(2, 3, 75.0, 0),
                named(3, 4, 75.0, 0),
            ],
            vec!["Strada Sinuousă".into()],
        );
        let route = route(&graph, (47.0, 28.0), (47.0, 28.004), Profile::Car).unwrap();
        assert_eq!(route.maneuvers.len(), 2, "{:#?}", route.maneuvers);
        assert_eq!(route.maneuvers[0].kind, KIND_DEPART);
        assert_eq!(route.maneuvers[1].kind, KIND_ARRIVE);
    }

    #[test]
    fn maneuver_distances_and_durations_add_up_to_the_route() {
        let graph = Graph::from_edges_with_names(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.001, 28.001),
                node(4, 47.001, 28.002),
            ],
            vec![
                named(0, 1, 75.0, 0),
                named(1, 2, 75.0, 1),
                named(2, 3, 75.0, 1),
            ],
            vec!["Strada A".into(), "Strada B".into()],
        );
        let route = route(&graph, (47.0, 28.0), (47.001, 28.002), Profile::Car).unwrap();
        let distance: f64 = route.maneuvers.iter().map(|m| m.distance_m).sum();
        let duration: f64 = route.maneuvers.iter().map(|m| m.duration_s).sum();
        assert!((distance - route.distance_m).abs() < 1e-6);
        assert!((duration - route.duration_s).abs() < 1e-6);
        let last = route.maneuvers.last().unwrap();
        assert_eq!(last.kind, KIND_ARRIVE);
        assert_eq!(last.distance_m, 0.0);
        assert_eq!(last.duration_s, 0.0);
    }

    #[test]
    fn roundabout_entry_reports_the_exit_number() {
        // Entry at node 3, a first exit branch at node 4, and the route leaves
        // at node 5, so the instruction must say "2nd exit".
        let graph = Graph::from_edges_with_names(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.0, 28.002),
                node(4, 47.0, 28.003),
                node(5, 47.0, 28.004),
                node(6, 47.001, 28.002),
            ],
            vec![
                edge(0, 1, 75.0, 50.0),
                roundabout(1, 2, 30.0),
                roundabout(2, 3, 30.0),
                edge(2, 5, 75.0, 50.0),
                // Two-way side road: both directions leave ring node 2, but
                // this is one physical exit and must be counted once.
                edge(5, 2, 75.0, 50.0),
                named(3, 4, 75.0, 0),
            ],
            vec!["Strada Ieșirii".into()],
        );
        let route = route(&graph, (47.0, 28.0), (47.0, 28.004), Profile::Car).unwrap();
        let roundabout = route
            .maneuvers
            .iter()
            .find(|maneuver| maneuver.kind == KIND_ROUNDABOUT)
            .expect("expected a roundabout maneuver");
        assert_eq!(
            roundabout.instruction,
            "At the roundabout, take the 2nd exit onto Strada Ieșirii"
        );
        assert_eq!(roundabout.point_index, 1);
    }

    #[test]
    fn roundabout_entry_without_a_name_still_reports_the_exit() {
        let graph = Graph::from_edges(
            vec![
                node(1, 47.0, 28.0),
                node(2, 47.0, 28.001),
                node(3, 47.0, 28.002),
                node(4, 47.0, 28.003),
            ],
            vec![
                edge(0, 1, 75.0, 50.0),
                roundabout(1, 2, 30.0),
                edge(2, 3, 75.0, 50.0),
            ],
        );
        let route = route(&graph, (47.0, 28.0), (47.0, 28.003), Profile::Car).unwrap();
        let roundabout = route
            .maneuvers
            .iter()
            .find(|maneuver| maneuver.kind == KIND_ROUNDABOUT)
            .expect("expected a roundabout maneuver");
        assert_eq!(
            roundabout.instruction,
            "At the roundabout, take the 1st exit"
        );
    }
}

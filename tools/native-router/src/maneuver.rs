//! Turn-by-turn maneuver generation from routed geometry and edge metadata.
//!
//! A maneuver is emitted when the travel direction changes by more than
//! [`TURN_DEAD_BAND_DEG`] between two consecutive path segments. The dead band
//! exists so that normal shape-point noise on a curved OSM way does not turn
//! into a stream of "keep left"/"keep right" instructions. A `straight`
//! maneuver is emitted only when the road name changes while the geometry
//! stays inside the dead band ("Continue onto …"), which is how a fork or a
//! way split is announced without guessing at every vertex.
//!
//! Roundabout handling is tag-based: the PBF builder flags edges of
//! `junction=roundabout|circular` ways, and this module counts the exits
//! between the entry and the exit edge on the routed path. It is a heuristic
//! (the graph has no turn restrictions), documented as such.

use serde::Serialize;

use crate::graph::{haversine_m, Graph};
use crate::profile::Profile;

/// Turns smaller than this are treated as straight geometry noise.
pub const TURN_DEAD_BAND_DEG: f64 = 15.0;

/// Upper bound of a "slight" turn, in degrees.
const SLIGHT_TURN_DEG: f64 = 35.0;
/// Upper bound of a normal turn, in degrees.
const NORMAL_TURN_DEG: f64 = 110.0;
/// Upper bound of a sharp turn; anything above is a U-turn.
const SHARP_TURN_DEG: f64 = 170.0;

pub const KIND_DEPART: &str = "depart";
pub const KIND_STRAIGHT: &str = "straight";
pub const KIND_SLIGHT_LEFT: &str = "slight_left";
pub const KIND_SLIGHT_RIGHT: &str = "slight_right";
pub const KIND_LEFT: &str = "left";
pub const KIND_RIGHT: &str = "right";
pub const KIND_SHARP_LEFT: &str = "sharp_left";
pub const KIND_SHARP_RIGHT: &str = "sharp_right";
pub const KIND_UTURN: &str = "uturn";
pub const KIND_ROUNDABOUT: &str = "roundabout";
pub const KIND_ARRIVE: &str = "arrive";

/// One turn-by-turn instruction.
///
/// `distance_m` / `duration_s` cover the travel from this maneuver to the next
/// one; the final `arrive` maneuver has both at zero. Coordinates serialize as
/// `[lat, lon]`, matching the route UI contract.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Maneuver {
    pub instruction: String,
    pub kind: String,
    pub distance_m: f64,
    pub duration_s: f64,
    pub point: (f64, f64),
    pub point_index: usize,
}

/// Generate the maneuver list for a routed path.
///
/// `edges` holds the global graph edge ids in travel order, with
/// `edges.len() == points.len() - 1`.
pub(crate) fn build_maneuvers(
    graph: &Graph,
    points: &[(f64, f64)],
    edges: &[u32],
    profile: Profile,
) -> Vec<Maneuver> {
    debug_assert_eq!(edges.len() + 1, points.len());
    if points.is_empty() {
        return Vec::new();
    }

    let mut distances = Vec::with_capacity(edges.len());
    let mut durations = Vec::with_capacity(edges.len());
    for (index, &edge_id) in edges.iter().enumerate() {
        let distance = match graph.edge(edge_id) {
            Some(edge) => f64::from(edge.distance_m),
            None => haversine_m(points[index], points[index + 1]),
        };
        let speed_kmh = graph
            .edge(edge_id)
            .map(|edge| f64::from(edge.speed_kmh))
            .unwrap_or(50.0)
            .min(f64::from(profile.max_speed_kmh().max(1.0)))
            .max(1.0);
        distances.push(distance);
        durations.push(distance / (speed_kmh / 3.6));
    }

    let mut indices = Vec::new();
    indices.push(0usize);
    if points.len() > 1 {
        for index in 1..points.len() - 1 {
            let incoming = edges[index - 1];
            let outgoing = edges[index];
            let incoming_roundabout = graph
                .edge(incoming)
                .is_some_and(|edge| edge.is_roundabout());
            let outgoing_roundabout = graph
                .edge(outgoing)
                .is_some_and(|edge| edge.is_roundabout());

            if !incoming_roundabout && outgoing_roundabout {
                // Entry into a roundabout: announce it once, then stay quiet
                // while the path follows the ring.
                indices.push(index);
                continue;
            }
            if incoming_roundabout || outgoing_roundabout {
                continue;
            }

            let turn = turn_degrees(points[index - 1], points[index], points[index + 1]);
            if turn.abs() < TURN_DEAD_BAND_DEG {
                let incoming_name = graph.edge_name_by_index(incoming);
                let outgoing_name = graph.edge_name_by_index(outgoing);
                if outgoing_name.is_some() && outgoing_name != incoming_name {
                    indices.push(index);
                }
            } else {
                indices.push(index);
            }
        }
        indices.push(points.len() - 1);
    } else {
        // Zero-length route: depart and arrive at the same snapped node.
        indices.push(0);
    }

    let mut maneuvers = Vec::with_capacity(indices.len());
    for (position, &point_index) in indices.iter().enumerate() {
        let next_index = indices.get(position + 1).copied();
        let (distance_m, duration_s) = match next_index {
            Some(next) => (
                distances[point_index..next].iter().sum(),
                durations[point_index..next].iter().sum(),
            ),
            None => (0.0, 0.0),
        };

        let is_single_point = points.len() == 1;
        let (kind, instruction) = if position == 0 {
            let first_edge = edges.first().copied();
            (
                KIND_DEPART,
                depart_instruction(points, first_edge, is_single_point, graph),
            )
        } else if position == indices.len() - 1 {
            (KIND_ARRIVE, "Arrive at your destination".to_string())
        } else {
            let incoming = edges[point_index - 1];
            let outgoing = edges[point_index];
            let incoming_roundabout = graph
                .edge(incoming)
                .is_some_and(|edge| edge.is_roundabout());
            let outgoing_roundabout = graph
                .edge(outgoing)
                .is_some_and(|edge| edge.is_roundabout());

            if !incoming_roundabout && outgoing_roundabout {
                (
                    KIND_ROUNDABOUT,
                    roundabout_instruction(graph, edges, point_index),
                )
            } else {
                let turn = turn_degrees(
                    points[point_index - 1],
                    points[point_index],
                    points[point_index + 1],
                );
                let kind = classify_turn(turn);
                let outgoing_name = graph.edge_name_by_index(outgoing);
                (kind, turn_instruction(kind, outgoing_name))
            }
        };

        maneuvers.push(Maneuver {
            instruction,
            kind: kind.to_string(),
            distance_m,
            duration_s,
            point: points[point_index],
            point_index,
        });
    }

    maneuvers
}

fn depart_instruction(
    points: &[(f64, f64)],
    first_edge: Option<u32>,
    is_single_point: bool,
    graph: &Graph,
) -> String {
    if is_single_point || points.len() < 2 {
        return "Start driving".to_string();
    }
    let direction = compass(bearing(points[0], points[1]));
    match first_edge.and_then(|edge| graph.edge_name_by_index(edge)) {
        Some(name) => format!("Head {direction} on {name}"),
        None => format!("Head {direction}"),
    }
}

/// Roundabouts with more distinct exit arms than this are reported without an
/// ordinal: the graph has no turn restrictions, so a very complex junction can
/// make the count unreliable.
const MAX_REPORTED_EXITS: usize = 8;

fn roundabout_instruction(graph: &Graph, edges: &[u32], entry_index: usize) -> String {
    let exit_edge = {
        let mut index = entry_index;
        while index < edges.len()
            && graph
                .edge(edges[index])
                .is_some_and(|edge| edge.is_roundabout())
        {
            index += 1;
        }
        index
    };
    let exit_name = edges
        .get(exit_edge)
        .and_then(|edge| graph.edge_name_by_index(*edge));
    match roundabout_exit_number(graph, edges, entry_index) {
        Some(number) if number <= MAX_REPORTED_EXITS => {
            let exit = format!("take the {} exit", ordinal(number));
            match exit_name {
                Some(name) => format!("At the roundabout, {exit} onto {name}"),
                None => format!("At the roundabout, {exit}"),
            }
        }
        Some(_) => match exit_name {
            Some(name) => format!("At the roundabout, take the exit onto {name}"),
            None => "At the roundabout, take the exit".to_string(),
        },
        None => "Enter the roundabout".to_string(),
    }
}

/// Number of distinct exit arms passed before leaving the roundabout, counting
/// the one the route takes. Returns `None` when the route never leaves the
/// roundabout.
///
/// Exit arms are deduplicated by neighbour node: a two-way side road appears
/// as two directed edges from the ring node, but it is one physical exit.
fn roundabout_exit_number(graph: &Graph, edges: &[u32], entry_index: usize) -> Option<usize> {
    let mut exit_edge = entry_index;
    while exit_edge < edges.len()
        && graph
            .edge(edges[exit_edge])
            .is_some_and(|edge| edge.is_roundabout())
    {
        exit_edge += 1;
    }
    if exit_edge >= edges.len() {
        return None;
    }

    let mut count = 0usize;
    let mut targets: Vec<u32> = Vec::new();
    for node_index in (entry_index + 1)..=exit_edge {
        targets.clear();
        for edge in graph.edges_from(node_index as u32) {
            if !edge.is_roundabout() && !targets.contains(&edge.to) {
                targets.push(edge.to);
            }
        }
        count += targets.len();
    }
    Some(count.max(1))
}

fn turn_instruction(kind: &str, name: Option<&str>) -> String {
    let onto = match name {
        Some(name) => format!(" onto {name}"),
        None => String::new(),
    };
    match kind {
        KIND_STRAIGHT => match name {
            Some(name) => format!("Continue onto {name}"),
            None => "Continue".to_string(),
        },
        KIND_SLIGHT_LEFT => format!("Keep left{onto}"),
        KIND_SLIGHT_RIGHT => format!("Keep right{onto}"),
        KIND_LEFT => format!("Turn left{onto}"),
        KIND_RIGHT => format!("Turn right{onto}"),
        KIND_SHARP_LEFT => format!("Turn sharp left{onto}"),
        KIND_SHARP_RIGHT => format!("Turn sharp right{onto}"),
        KIND_UTURN => format!("Make a U-turn{onto}"),
        _ => "Continue".to_string(),
    }
}

fn classify_turn(turn: f64) -> &'static str {
    let magnitude = turn.abs();
    if magnitude < TURN_DEAD_BAND_DEG {
        KIND_STRAIGHT
    } else if magnitude < SLIGHT_TURN_DEG {
        if turn > 0.0 {
            KIND_SLIGHT_RIGHT
        } else {
            KIND_SLIGHT_LEFT
        }
    } else if magnitude < NORMAL_TURN_DEG {
        if turn > 0.0 {
            KIND_RIGHT
        } else {
            KIND_LEFT
        }
    } else if magnitude < SHARP_TURN_DEG {
        if turn > 0.0 {
            KIND_SHARP_RIGHT
        } else {
            KIND_SHARP_LEFT
        }
    } else {
        KIND_UTURN
    }
}

/// Signed turn from the incoming to the outgoing segment, in degrees.
///
/// Positive values turn clockwise (right); the result is normalised to
/// `(-180, 180]`.
fn turn_degrees(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    let incoming = bearing(a, b);
    let outgoing = bearing(b, c);
    if !incoming.is_finite() || !outgoing.is_finite() {
        return 0.0;
    }
    let mut turn = (outgoing - incoming) % 360.0;
    if turn > 180.0 {
        turn -= 360.0;
    } else if turn <= -180.0 {
        turn += 360.0;
    }
    turn
}

/// Initial great-circle bearing from `a` to `b`, in degrees clockwise from
/// north.
fn bearing(a: (f64, f64), b: (f64, f64)) -> f64 {
    let lat1 = a.0.to_radians();
    let lat2 = b.0.to_radians();
    let dlon = (b.1 - a.1).to_radians();
    let y = dlon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
    y.atan2(x).to_degrees()
}

fn compass(bearing_deg: f64) -> &'static str {
    const DIRECTIONS: [&str; 8] = [
        "north",
        "northeast",
        "east",
        "southeast",
        "south",
        "southwest",
        "west",
        "northwest",
    ];
    let normalized = (bearing_deg % 360.0 + 360.0) % 360.0;
    let index = ((normalized + 22.5) / 45.0).floor() as usize % 8;
    DIRECTIONS[index]
}

fn ordinal(number: usize) -> String {
    let suffix = match (number % 10, number % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{number}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_band_bounds_and_turn_signs() {
        assert!(turn_degrees((47.0, 28.0), (47.0, 28.001), (47.0, 28.002)).abs() < 1e-9);
        // East then north is a left turn.
        let left = turn_degrees((47.0, 28.0), (47.0, 28.001), (47.001, 28.001));
        assert!((left + 90.0).abs() < 1e-3, "got {left}");
        // East then south is a right turn.
        let right = turn_degrees((47.0, 28.0), (47.0, 28.001), (46.999, 28.001));
        assert!((right - 90.0).abs() < 1e-3, "got {right}");
    }

    #[test]
    fn classifies_turn_bands() {
        assert_eq!(classify_turn(0.0), KIND_STRAIGHT);
        assert_eq!(classify_turn(20.0), KIND_SLIGHT_RIGHT);
        assert_eq!(classify_turn(-20.0), KIND_SLIGHT_LEFT);
        assert_eq!(classify_turn(90.0), KIND_RIGHT);
        assert_eq!(classify_turn(-90.0), KIND_LEFT);
        assert_eq!(classify_turn(150.0), KIND_SHARP_RIGHT);
        assert_eq!(classify_turn(-150.0), KIND_SHARP_LEFT);
        assert_eq!(classify_turn(179.0), KIND_UTURN);
    }

    #[test]
    fn ordinals_read_naturally() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(4), "4th");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(22), "22nd");
    }
}

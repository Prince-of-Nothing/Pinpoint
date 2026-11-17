//! OpenStreetMap PBF to car graph conversion.
//!
//! Two streaming passes keep memory bounded:
//!
//! 1. Read ways only; keep the node ids of car-routable highways and the
//!    parsed road attributes.
//! 2. Read nodes only; keep coordinates for the ids collected in pass 1.
//!
//! The graph is then built in one more in-memory pass. Nothing is downloaded:
//! the PBF path is the sole input.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[cfg(test)]
use std::collections::BTreeMap;

use osmpbfreader::OsmPbfReader;

use crate::graph::{haversine_m, Edge, Graph, Node, RoadClass, EDGE_FLAG_ROUNDABOUT, NO_NAME};

/// Highest speed the car profile will accept from a `maxspeed` tag.
const MAX_SPEED_KMH: f32 = 130.0;
/// Floor for tagged or class speeds, so free-flow time never divides by zero.
const MIN_SPEED_KMH: f32 = 5.0;

#[derive(Debug)]
pub enum PbfError {
    Io(std::io::Error),
    Read(String),
    Empty(String),
}

impl fmt::Display for PbfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PbfError::Io(err) => write!(f, "I/O error: {err}"),
            PbfError::Read(err) => write!(f, "PBF read error: {err}"),
            PbfError::Empty(path) => {
                write!(
                    f,
                    "no car-routable highways with coordinates found in {path}"
                )
            }
        }
    }
}

impl std::error::Error for PbfError {}

impl From<std::io::Error> for PbfError {
    fn from(err: std::io::Error) -> Self {
        PbfError::Io(err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OneWay {
    Both,
    Forward,
    Backward,
}

#[derive(Debug, Clone)]
struct WayRule {
    class: RoadClass,
    speed_kmh: f32,
    oneway: OneWay,
    /// `junction=roundabout|circular`; used for maneuver generation.
    roundabout: bool,
    /// `name`, or `ref` as the fallback, trimmed. `None` when both are absent.
    name: Option<Box<str>>,
}

struct WayRecord {
    nodes: Vec<i64>,
    rule: WayRule,
}

/// The subset of OSM way tags used by [`parse_way_rule`]. Keeping this
/// explicit avoids allocating a map per way while streaming the PBF.
struct RawWayTags<'a> {
    highway: Option<&'a str>,
    access: Option<&'a str>,
    motorcar: Option<&'a str>,
    motor_vehicle: Option<&'a str>,
    vehicle: Option<&'a str>,
    oneway: Option<&'a str>,
    junction: Option<&'a str>,
    area: Option<&'a str>,
    maxspeed: Option<&'a str>,
    name: Option<&'a str>,
    reference: Option<&'a str>,
}

impl<'a> RawWayTags<'a> {
    #[cfg(test)]
    fn from_map(tags: &'a BTreeMap<String, String>) -> Self {
        let get = |key: &str| tags.get(key).map(String::as_str);
        Self {
            highway: get("highway"),
            access: get("access"),
            motorcar: get("motorcar"),
            motor_vehicle: get("motor_vehicle"),
            vehicle: get("vehicle"),
            oneway: get("oneway"),
            junction: get("junction"),
            area: get("area"),
            maxspeed: get("maxspeed"),
            name: get("name"),
            reference: get("ref"),
        }
    }

    fn from_osm(tags: &'a osmpbfreader::Tags) -> Self {
        let get = |key: &str| tags.get(key).map(|value| value.as_str());
        Self {
            highway: get("highway"),
            access: get("access"),
            motorcar: get("motorcar"),
            motor_vehicle: get("motor_vehicle"),
            vehicle: get("vehicle"),
            oneway: get("oneway"),
            junction: get("junction"),
            area: get("area"),
            maxspeed: get("maxspeed"),
            name: get("name"),
            reference: get("ref"),
        }
    }
}

/// Parse the subset of OSM way tags this router understands.
///
/// Returns `None` for ways that are not car-routable. Access precedence is
/// `motorcar` > `motor_vehicle` > `vehicle` > `access`; the first present tag
/// decides.
fn parse_way_rule(tags: RawWayTags<'_>) -> Option<WayRule> {
    if tags
        .area
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("yes"))
    {
        return None;
    }
    let highway = tags.highway?.trim();
    let class = match highway {
        "motorway" => RoadClass::Motorway,
        "motorway_link" => RoadClass::MotorwayLink,
        "trunk" => RoadClass::Trunk,
        "trunk_link" => RoadClass::TrunkLink,
        "primary" => RoadClass::Primary,
        "primary_link" => RoadClass::PrimaryLink,
        "secondary" => RoadClass::Secondary,
        "secondary_link" => RoadClass::SecondaryLink,
        "tertiary" => RoadClass::Tertiary,
        "tertiary_link" => RoadClass::TertiaryLink,
        "unclassified" => RoadClass::Unclassified,
        "residential" => RoadClass::Residential,
        "living_street" => RoadClass::LivingStreet,
        "service" => RoadClass::Service,
        "road" => RoadClass::Road,
        _ => return None,
    };

    let access = tags
        .motorcar
        .or(tags.motor_vehicle)
        .or(tags.vehicle)
        .or(tags.access);
    if access.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "no" | "private" | "agricultural" | "forestry"
        )
    }) {
        return None;
    }

    let oneway = match tags
        .oneway
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("yes" | "true" | "1") => OneWay::Forward,
        Some("-1" | "reverse") => OneWay::Backward,
        Some("no" | "false" | "0") => OneWay::Both,
        _ if tags
            .junction
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("roundabout")) =>
        {
            OneWay::Forward
        }
        _ => OneWay::Both,
    };

    let roundabout = tags.junction.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "roundabout" | "circular"
        )
    });

    let speed_kmh = tags
        .maxspeed
        .and_then(parse_maxspeed_kmh)
        .unwrap_or_else(|| class.default_speed_kmh())
        .clamp(MIN_SPEED_KMH, MAX_SPEED_KMH);

    let name = tags
        .name
        .and_then(clean_tag)
        .or_else(|| tags.reference.and_then(clean_tag))
        .map(Into::into);

    Some(WayRule {
        class,
        speed_kmh,
        oneway,
        roundabout,
        name,
    })
}

/// Trim a tag value and treat an empty result as absent.
fn clean_tag(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// Parse an OSM `maxspeed` value into km/h. Returns `None` for values that are
/// not a plain number or `walk` (for example `RO:urban` or `signals`), so the
/// caller can fall back to the road-class default.
fn parse_maxspeed_kmh(raw: &str) -> Option<f32> {
    let first = raw.split(';').next()?.trim().to_ascii_lowercase();
    if first.is_empty() || first == "none" {
        return None;
    }
    if first == "walk" {
        return Some(MIN_SPEED_KMH);
    }
    let (number, is_mph) = match first.strip_suffix("mph") {
        Some(number) => (number, true),
        None => (first.as_str(), false),
    };
    let value: f32 = number.split_whitespace().next()?.parse().ok()?;
    let kmh = if is_mph { value * 1.609_344 } else { value };
    kmh.is_finite().then_some(kmh)
}

/// Build a car graph from an OSM PBF file.
pub fn load_pbf<P: AsRef<Path>>(path: P) -> Result<Graph, PbfError> {
    let path = path.as_ref();

    // Pass 1: ways only. Keeping just car-routable ways and their node ids
    // avoids holding every Moldova node in memory.
    let mut ways = Vec::new();
    let mut needed_nodes = HashSet::new();
    {
        let file = File::open(path)?;
        let mut reader = OsmPbfReader::new(BufReader::new(file));
        for way in reader.iter_ways() {
            let way = way.map_err(|err| PbfError::Read(err.to_string()))?;
            let Some(rule) = parse_way_rule(RawWayTags::from_osm(&way.tags)) else {
                continue;
            };
            if way.nodes.len() < 2 {
                continue;
            }
            for node in &way.nodes {
                needed_nodes.insert(node.0);
            }
            ways.push(WayRecord {
                nodes: way.nodes.into_iter().map(|node| node.0).collect(),
                rule,
            });
        }
    }
    if ways.is_empty() {
        return Err(PbfError::Empty(path.display().to_string()));
    }

    // Pass 2: nodes only, filtered to the ids referenced by car ways.
    let mut coordinates = HashMap::with_capacity(needed_nodes.len());
    {
        let file = File::open(path)?;
        let mut reader = OsmPbfReader::new(BufReader::new(file));
        for node in reader.iter_nodes() {
            let node = node.map_err(|err| PbfError::Read(err.to_string()))?;
            if needed_nodes.contains(&node.id.0) {
                coordinates.insert(node.id.0, (node.lat(), node.lon()));
            }
        }
    }
    drop(needed_nodes);

    // Build graph nodes on first use and add one directed edge per allowed
    // travel direction. A way node missing from the extract breaks the chain:
    // the two sides are not connected across the gap.
    let mut index: HashMap<i64, u32> = HashMap::with_capacity(coordinates.len());
    let mut nodes: Vec<Node> = Vec::with_capacity(coordinates.len());
    let mut edges: Vec<Edge> = Vec::new();
    let mut names: Vec<Box<str>> = Vec::new();
    let mut name_ids: HashMap<Box<str>, u32> = HashMap::new();
    for way in ways {
        let name_id = match way.rule.name.as_deref() {
            Some(name) => intern_name(name, &mut names, &mut name_ids),
            None => NO_NAME,
        };
        let flags = if way.rule.roundabout {
            EDGE_FLAG_ROUNDABOUT
        } else {
            0
        };
        let mut previous: Option<u32> = None;
        for osm_id in way.nodes {
            let Some(&(lat, lon)) = coordinates.get(&osm_id) else {
                previous = None;
                continue;
            };
            let current = match index.get(&osm_id) {
                Some(&id) => id,
                None => {
                    let id = nodes.len() as u32;
                    nodes.push(Node { osm_id, lat, lon });
                    index.insert(osm_id, id);
                    id
                }
            };
            if let Some(from) = previous {
                if from != current {
                    let a = &nodes[from as usize];
                    let b = &nodes[current as usize];
                    let distance_m = haversine_m((a.lat, a.lon), (b.lat, b.lon)) as f32;
                    if distance_m > 0.0 {
                        let forward = Edge {
                            from,
                            to: current,
                            distance_m,
                            speed_kmh: way.rule.speed_kmh,
                            class: way.rule.class,
                            flags,
                            name: name_id,
                        };
                        let backward = Edge {
                            from: current,
                            to: from,
                            ..forward
                        };
                        match way.rule.oneway {
                            OneWay::Both => {
                                edges.push(forward);
                                edges.push(backward);
                            }
                            OneWay::Forward => edges.push(forward),
                            OneWay::Backward => edges.push(backward),
                        }
                    }
                }
            }
            previous = Some(current);
        }
    }

    if nodes.is_empty() {
        return Err(PbfError::Empty(path.display().to_string()));
    }
    Ok(Graph::from_edges_with_names(nodes, edges, names))
}

/// Intern a road name, returning its table index.
///
/// The table owns one `Box<str>` per unique name; every edge only stores the
/// 4-byte id. Ways that share a name (or a `ref`) share the id.
fn intern_name(name: &str, names: &mut Vec<Box<str>>, ids: &mut HashMap<Box<str>, u32>) -> u32 {
    if let Some(&id) = ids.get(name) {
        return id;
    }
    let id = names.len() as u32;
    let boxed: Box<str> = name.into();
    ids.insert(boxed.clone(), id);
    names.push(boxed);
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::RoadClass;

    fn tags(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    // Test-only convenience so the cases below read as tag lists.
    fn parse_way_rule(tags: &BTreeMap<String, String>) -> Option<WayRule> {
        super::parse_way_rule(RawWayTags::from_map(tags))
    }

    #[test]
    fn keeps_car_highways_with_class_defaults() {
        let rule = parse_way_rule(&tags(&[("highway", "residential")])).unwrap();
        assert_eq!(rule.class, RoadClass::Residential);
        assert_eq!(rule.speed_kmh, 50.0);
        assert_eq!(rule.oneway, OneWay::Both);

        let rule = parse_way_rule(&tags(&[("highway", "motorway")])).unwrap();
        assert_eq!(rule.class, RoadClass::Motorway);
        assert_eq!(rule.speed_kmh, 110.0);
    }

    #[test]
    fn drops_non_car_ways() {
        for highway in [
            "footway",
            "path",
            "cycleway",
            "steps",
            "construction",
            "proposed",
        ] {
            assert!(
                parse_way_rule(&tags(&[("highway", highway)])).is_none(),
                "{highway} must not be routable by car"
            );
        }
        assert!(parse_way_rule(&tags(&[("building", "yes")])).is_none());
        assert!(parse_way_rule(&tags(&[("highway", "residential"), ("area", "yes")])).is_none());
    }

    #[test]
    fn reads_oneway_directions_and_roundabouts() {
        assert_eq!(
            parse_way_rule(&tags(&[("highway", "primary"), ("oneway", "yes")]))
                .unwrap()
                .oneway,
            OneWay::Forward
        );
        assert_eq!(
            parse_way_rule(&tags(&[("highway", "primary"), ("oneway", "-1")]))
                .unwrap()
                .oneway,
            OneWay::Backward
        );
        assert_eq!(
            parse_way_rule(&tags(&[("highway", "primary"), ("oneway", "no")]))
                .unwrap()
                .oneway,
            OneWay::Both
        );
        assert_eq!(
            parse_way_rule(&tags(&[("highway", "primary"), ("junction", "roundabout")]))
                .unwrap()
                .oneway,
            OneWay::Forward
        );
        assert_eq!(
            parse_way_rule(&tags(&[
                ("highway", "primary"),
                ("junction", "roundabout"),
                ("oneway", "no"),
            ]))
            .unwrap()
            .oneway,
            OneWay::Both
        );
    }

    #[test]
    fn captures_name_with_ref_fallback() {
        let rule = parse_way_rule(&tags(&[
            ("highway", "secondary"),
            ("name", "  Strada Ștefan cel Mare  "),
            ("ref", "M14"),
        ]))
        .unwrap();
        assert_eq!(rule.name.as_deref(), Some("Strada Ștefan cel Mare"));

        let rule = parse_way_rule(&tags(&[
            ("highway", "trunk"),
            ("ref", " M14 "),
            ("name", "   "),
        ]))
        .unwrap();
        assert_eq!(rule.name.as_deref(), Some("M14"), "ref is the fallback");

        let rule = parse_way_rule(&tags(&[("highway", "residential")])).unwrap();
        assert_eq!(rule.name, None);

        let rule = parse_way_rule(&tags(&[
            ("highway", "residential"),
            ("name", ""),
            ("ref", ""),
        ]))
        .unwrap();
        assert_eq!(rule.name, None, "empty tags are treated as absent");
    }

    #[test]
    fn flags_roundabouts_including_circular_junctions() {
        let rule =
            parse_way_rule(&tags(&[("highway", "primary"), ("junction", "roundabout")])).unwrap();
        assert!(rule.roundabout);
        assert_eq!(rule.oneway, OneWay::Forward);

        let rule =
            parse_way_rule(&tags(&[("highway", "primary"), ("junction", "circular")])).unwrap();
        assert!(rule.roundabout);

        let rule = parse_way_rule(&tags(&[("highway", "primary")])).unwrap();
        assert!(!rule.roundabout, "plain ways must not be flagged");
    }

    #[test]
    fn oneway_values_map_to_travel_directions() {
        for value in ["yes", "true", "1", "YES"] {
            assert_eq!(
                parse_way_rule(&tags(&[("highway", "primary"), ("oneway", value)]))
                    .unwrap()
                    .oneway,
                OneWay::Forward,
                "oneway={value}"
            );
        }
        for value in ["-1", "reverse"] {
            assert_eq!(
                parse_way_rule(&tags(&[("highway", "primary"), ("oneway", value)]))
                    .unwrap()
                    .oneway,
                OneWay::Backward,
                "oneway={value}"
            );
        }
        for value in ["no", "false", "0", "unknown"] {
            assert_eq!(
                parse_way_rule(&tags(&[("highway", "primary"), ("oneway", value)]))
                    .unwrap()
                    .oneway,
                OneWay::Both,
                "oneway={value}"
            );
        }
    }

    #[test]
    fn honours_access_restrictions_with_specific_override() {
        assert!(parse_way_rule(&tags(&[("highway", "service"), ("access", "private")])).is_none());
        assert!(parse_way_rule(&tags(&[("highway", "service"), ("access", "no")])).is_none());
        assert!(
            parse_way_rule(&tags(&[("highway", "service"), ("access", "agricultural")])).is_none()
        );
        // A more specific motorcar=yes overrides a general access=private.
        assert!(parse_way_rule(&tags(&[
            ("highway", "service"),
            ("access", "private"),
            ("motorcar", "yes"),
        ]))
        .is_some());
        // destination access is legal to enter.
        assert!(parse_way_rule(&tags(&[
            ("highway", "residential"),
            ("access", "destination")
        ]))
        .is_some());
    }

    #[test]
    fn parses_maxspeed_and_units() {
        let rule = parse_way_rule(&tags(&[("highway", "primary"), ("maxspeed", "70")])).unwrap();
        assert_eq!(rule.speed_kmh, 70.0);

        let rule =
            parse_way_rule(&tags(&[("highway", "primary"), ("maxspeed", "30 mph")])).unwrap();
        assert!(
            (rule.speed_kmh - 48.28).abs() < 0.01,
            "got {}",
            rule.speed_kmh
        );

        // Unparseable country defaults fall back to the class speed.
        let rule =
            parse_way_rule(&tags(&[("highway", "primary"), ("maxspeed", "RO:urban")])).unwrap();
        assert_eq!(rule.speed_kmh, 90.0);
        // Absurd tags are clamped to the profile ceiling.
        let rule = parse_way_rule(&tags(&[("highway", "motorway"), ("maxspeed", "300")])).unwrap();
        assert_eq!(rule.speed_kmh, MAX_SPEED_KMH);
    }

    #[test]
    fn known_classes_cover_every_retained_highway_value() {
        for (value, class) in [
            ("motorway", RoadClass::Motorway),
            ("motorway_link", RoadClass::MotorwayLink),
            ("trunk", RoadClass::Trunk),
            ("trunk_link", RoadClass::TrunkLink),
            ("primary", RoadClass::Primary),
            ("primary_link", RoadClass::PrimaryLink),
            ("secondary", RoadClass::Secondary),
            ("secondary_link", RoadClass::SecondaryLink),
            ("tertiary", RoadClass::Tertiary),
            ("tertiary_link", RoadClass::TertiaryLink),
            ("unclassified", RoadClass::Unclassified),
            ("residential", RoadClass::Residential),
            ("living_street", RoadClass::LivingStreet),
            ("service", RoadClass::Service),
            ("road", RoadClass::Road),
        ] {
            assert_eq!(
                parse_way_rule(&tags(&[("highway", value)])).unwrap().class,
                class
            );
        }
    }
}

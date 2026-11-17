//! Graph data structures and the documented on-disk/in-memory format.
//!
//! Node coordinates are WGS84 degrees. Edges are directed; the PBF builder
//! emits one edge per travel direction. The graph is stored as:
//!
//! - `nodes`: a flat `Vec<Node>`, indexed by `u32` graph node id.
//! - `edges`: a flat `Vec<Edge>` grouped by source node.
//! - `offsets`: `offsets[i]..offsets[i + 1]` is the slice of `edges` leaving
//!   node `i`.
//! - `names`: an interned road-name table. `Edge::name` is a `u32` index into
//!   it, or [`NO_NAME`] when the way carried neither `name` nor `ref`. Keeping
//!   the string out of the edge means 3.1M directed Moldova edges cost 4 bytes
//!   of name data each instead of a `String` (24 bytes + heap allocation).
//! - `edge.flags`: small bit set; bit 0 marks `junction=roundabout` ways so
//!   the maneuver generator can recognise roundabouts without a second lookup.
//!
//! There are no turn restrictions, turn costs or lane data yet.
//!
//! # Persisted format (`Graph::save` / `Graph::load`)
//!
//! All integers are little-endian. The header is 56 bytes:
//!
//! ```text
//! 0   8   magic "PNPGRPH1"
//! 8   4   format version (currently 1)
//! 12  4   reserved, zero
//! 16  8   node count
//! 24  8   edge count
//! 32  8   interned name count
//! 40  8   payload length in bytes (everything after the header)
//! 48  4   CRC-32 (IEEE) of the payload
//! 52  4   reserved, zero
//! ```
//!
//! The payload is the name table followed by the node array and the edge array:
//!
//! ```text
//! names:   name_count × (u32 byte length, UTF-8 bytes)
//! nodes:   node_count × (i64 osm_id, f64 lat, f64 lon)                  = 24 B
//! edges:   edge_count × (u32 from, u32 to, f32 distance_m, f32 speed_kmh,
//!                        u8 class, u8 flags, u16 reserved, u32 name)    = 24 B
//! ```
//!
//! `offsets` and `max_speed_kmh` are derived on load; the edge array must be
//! grouped by `from` exactly as in memory. Loading streams the arrays straight
//! into their final `Vec`s and never allocates a string per edge. The CRC and
//! every structural invariant are validated before a [`Graph`] is returned, so
//! a truncated or tampered file is rejected, not routed on.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

/// Mean Earth radius used for all distance computations.
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Sentinel `Edge::name` value for "this road has no name or ref".
pub const NO_NAME: u32 = u32::MAX;

/// `Edge::flags` bit: the edge belongs to a `junction=roundabout|circular` way.
pub const EDGE_FLAG_ROUNDABOUT: u8 = 0b0000_0001;

/// Magic bytes at the start of a persisted graph file.
pub const GRAPH_MAGIC: [u8; 8] = *b"PNPGRPH1";

/// Current persisted graph format version.
pub const GRAPH_FORMAT_VERSION: u32 = 1;

/// Persisted header size in bytes.
pub const GRAPH_HEADER_LEN: usize = 56;

const NODE_RECORD_LEN: usize = 24;
const EDGE_RECORD_LEN: usize = 24;
const LOAD_CHUNK_RECORDS: usize = 4096;
/// Names longer than this are treated as corrupt; OSM names are far shorter.
const MAX_NAME_BYTES: u32 = 4096;

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
    Motorway = 0,
    MotorwayLink = 1,
    Trunk = 2,
    TrunkLink = 3,
    Primary = 4,
    PrimaryLink = 5,
    Secondary = 6,
    SecondaryLink = 7,
    Tertiary = 8,
    TertiaryLink = 9,
    Unclassified = 10,
    Residential = 11,
    LivingStreet = 12,
    Service = 13,
    Road = 14,
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

    /// Decode the on-disk discriminant. Returns `None` for unknown values.
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => RoadClass::Motorway,
            1 => RoadClass::MotorwayLink,
            2 => RoadClass::Trunk,
            3 => RoadClass::TrunkLink,
            4 => RoadClass::Primary,
            5 => RoadClass::PrimaryLink,
            6 => RoadClass::Secondary,
            7 => RoadClass::SecondaryLink,
            8 => RoadClass::Tertiary,
            9 => RoadClass::TertiaryLink,
            10 => RoadClass::Unclassified,
            11 => RoadClass::Residential,
            12 => RoadClass::LivingStreet,
            13 => RoadClass::Service,
            14 => RoadClass::Road,
            _ => return None,
        })
    }
}

/// One OSM node kept in the routing graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    /// Original OSM node id, kept so routes can be inspected and tests can
    /// assert on real-world data.
    pub osm_id: i64,
    pub lat: f64,
    pub lon: f64,
}

/// One directed graph edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    /// Great-circle length in metres.
    pub distance_m: f32,
    /// Effective car speed for this edge, already clamped to the profile.
    pub speed_kmh: f32,
    pub class: RoadClass,
    /// Bit set, see [`EDGE_FLAG_ROUNDABOUT`].
    pub flags: u8,
    /// Index into [`Graph::names`], or [`NO_NAME`].
    pub name: u32,
}

impl Edge {
    /// Convenience constructor for an unnamed, non-roundabout edge.
    pub fn new(from: u32, to: u32, distance_m: f32, speed_kmh: f32, class: RoadClass) -> Self {
        Self {
            from,
            to,
            distance_m,
            speed_kmh,
            class,
            flags: 0,
            name: NO_NAME,
        }
    }

    pub fn is_roundabout(&self) -> bool {
        self.flags & EDGE_FLAG_ROUNDABOUT != 0
    }
}

/// Directed car road graph in compressed-sparse-row form.
#[derive(Debug, Default)]
pub struct Graph {
    nodes: Vec<Node>,
    offsets: Vec<u32>,
    edges: Vec<Edge>,
    names: Vec<Box<str>>,
    /// Maximum edge speed, used for the A* heuristic.
    max_speed_kmh: f32,
}

impl Graph {
    /// Build an unnamed graph from nodes and directed edges.
    ///
    /// Invalid endpoints, non-positive distances or non-positive speeds panic:
    /// they indicate a bug in the PBF builder or in a test fixture, never user
    /// input.
    pub fn from_edges(nodes: Vec<Node>, edges: Vec<Edge>) -> Self {
        assert!(
            edges.iter().all(|edge| edge.name == NO_NAME),
            "from_edges cannot carry names; use from_edges_with_names"
        );
        Self::from_edges_with_names(nodes, edges, Vec::new())
    }

    /// Build a graph with an interned name table.
    ///
    /// `edge.name` must be [`NO_NAME`] or a valid index into `names`.
    pub fn from_edges_with_names(nodes: Vec<Node>, edges: Vec<Edge>, names: Vec<Box<str>>) -> Self {
        let node_count = nodes.len();
        assert!(
            node_count <= u32::MAX as usize,
            "graph has more nodes than u32 allows"
        );
        assert!(
            names.len() <= NO_NAME as usize,
            "name table is too large for u32 ids"
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
            assert!(
                edge.name == NO_NAME || (edge.name as usize) < names.len(),
                "edge name id outside the name table"
            );
            offsets[edge.from as usize + 1] += 1;
        }
        for i in 0..node_count {
            offsets[i + 1] += offsets[i];
        }

        let mut cursor = offsets.clone();
        let mut sorted = vec![Edge::new(0, 0, 0.0, 1.0, RoadClass::Road); edges.len()];
        for edge in edges {
            let slot = &mut cursor[edge.from as usize];
            sorted[*slot as usize] = edge;
            *slot += 1;
        }

        Self::finish(nodes, offsets, sorted, names)
    }

    /// Assemble a graph whose CSR arrays are already in their final form.
    fn finish(nodes: Vec<Node>, offsets: Vec<u32>, edges: Vec<Edge>, names: Vec<Box<str>>) -> Self {
        let max_speed_kmh = edges
            .iter()
            .map(|edge| edge.speed_kmh)
            .fold(0.0_f32, f32::max)
            .max(1.0);
        Graph {
            nodes,
            offsets,
            edges,
            names,
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

    /// Interned road names, parallel to `Edge::name` ids.
    pub fn names(&self) -> &[Box<str>] {
        &self.names
    }

    /// Resolve a name id; `None` for [`NO_NAME`] and out-of-range ids.
    pub fn name(&self, id: u32) -> Option<&str> {
        if id == NO_NAME {
            return None;
        }
        self.names.get(id as usize).map(|name| &**name)
    }

    /// Road name or ref for an edge, when the way carried one.
    pub fn edge_name(&self, edge: &Edge) -> Option<&str> {
        self.name(edge.name)
    }

    /// Global edge index for an edge returned by a `Graph` accessor.
    pub fn edge(&self, index: u32) -> Option<&Edge> {
        self.edges.get(index as usize)
    }

    /// Road name or ref for a global edge index.
    pub fn edge_name_by_index(&self, index: u32) -> Option<&str> {
        self.edge(index).and_then(|edge| self.edge_name(edge))
    }

    /// Index of the first edge leaving `node`; the caller must use it only
    /// with a valid node id.
    pub fn edge_index_base(&self, node: u32) -> u32 {
        self.offsets.get(node as usize).copied().unwrap_or_default()
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

    /// Persist this graph in the documented binary format.
    ///
    /// The payload is written to a sibling temp file, flushed and synced, then
    /// atomically renamed over `path`. A failed save leaves `path` untouched
    /// and removes the temp file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), GraphError> {
        let path = path.as_ref();
        let temp = temp_path(path);
        match self.write_file(&temp) {
            Ok(()) => match fs::rename(&temp, path) {
                Ok(()) => Ok(()),
                Err(err) => {
                    let _ = fs::remove_file(&temp);
                    Err(GraphError::Io(err))
                }
            },
            Err(err) => {
                let _ = fs::remove_file(&temp);
                Err(err)
            }
        }
    }

    /// Load a graph written by [`Graph::save`].
    ///
    /// Rejects a bad magic/version, truncation, a CRC mismatch and any
    /// structural violation (endpoint out of range, non-positive speed,
    /// unknown road class, unknown name id, edges not grouped by source).
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Graph, GraphError> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        if file_len < GRAPH_HEADER_LEN as u64 {
            return Err(GraphError::Truncated {
                what: "header",
                expected: GRAPH_HEADER_LEN as u64,
                actual: file_len,
            });
        }

        let mut reader = BufReader::with_capacity(1 << 20, file);
        let mut header = [0u8; GRAPH_HEADER_LEN];
        read_raw(&mut reader, &mut header)?;
        if header[0..8] != GRAPH_MAGIC {
            let mut magic = [0u8; 8];
            magic.copy_from_slice(&header[0..8]);
            return Err(GraphError::BadMagic(magic));
        }
        let version = u32::from_le_bytes(header[8..12].try_into().expect("fixed slice"));
        if version != GRAPH_FORMAT_VERSION {
            return Err(GraphError::UnsupportedVersion(version));
        }
        let node_count = u64::from_le_bytes(header[16..24].try_into().expect("fixed slice"));
        let edge_count = u64::from_le_bytes(header[24..32].try_into().expect("fixed slice"));
        let name_count = u64::from_le_bytes(header[32..40].try_into().expect("fixed slice"));
        let payload_len = u64::from_le_bytes(header[40..48].try_into().expect("fixed slice"));
        let expected_crc = u32::from_le_bytes(header[48..52].try_into().expect("fixed slice"));

        let expected_file_len = GRAPH_HEADER_LEN as u64 + payload_len;
        if file_len < expected_file_len {
            return Err(GraphError::Truncated {
                what: "payload",
                expected: payload_len,
                actual: file_len.saturating_sub(GRAPH_HEADER_LEN as u64),
            });
        }
        if file_len > expected_file_len {
            return Err(GraphError::Corrupt(format!(
                "{} trailing bytes after the payload",
                file_len - expected_file_len
            )));
        }

        let node_bytes = node_count
            .checked_mul(NODE_RECORD_LEN as u64)
            .ok_or_else(|| GraphError::Corrupt("node count overflows".into()))?;
        let edge_bytes = edge_count
            .checked_mul(EDGE_RECORD_LEN as u64)
            .ok_or_else(|| GraphError::Corrupt("edge count overflows".into()))?;
        let fixed_bytes = node_bytes
            .checked_add(edge_bytes)
            .and_then(|bytes| bytes.checked_add(name_count.checked_mul(4)?))
            .ok_or_else(|| GraphError::Corrupt("graph size overflows".into()))?;
        if fixed_bytes > payload_len {
            return Err(GraphError::Corrupt(
                "counts do not fit in the declared payload length".into(),
            ));
        }
        if node_count > u32::MAX as u64 || name_count >= NO_NAME as u64 {
            return Err(GraphError::Corrupt(
                "node or name count exceeds the u32 id space".into(),
            ));
        }

        let mut hashing = CrcReader::new(&mut reader);

        let mut names: Vec<Box<str>> = Vec::with_capacity(name_count as usize);
        for _ in 0..name_count {
            let mut length_buf = [0u8; 4];
            hashing.read_exact(&mut length_buf)?;
            let length = u32::from_le_bytes(length_buf);
            if length > MAX_NAME_BYTES {
                return Err(GraphError::Corrupt(format!(
                    "name length {length} exceeds the {MAX_NAME_BYTES}-byte limit"
                )));
            }
            let mut bytes = vec![0u8; length as usize];
            hashing.read_exact(&mut bytes)?;
            let name = String::from_utf8(bytes)
                .map_err(|_| GraphError::Corrupt("name is not valid UTF-8".into()))?;
            names.push(name.into_boxed_str());
        }

        let mut nodes: Vec<Node> = Vec::with_capacity(node_count as usize);
        let mut chunk = vec![0u8; NODE_RECORD_LEN * LOAD_CHUNK_RECORDS];
        let mut remaining = node_count;
        while remaining > 0 {
            let take = remaining.min(LOAD_CHUNK_RECORDS as u64) as usize;
            let buffer = &mut chunk[..take * NODE_RECORD_LEN];
            hashing.read_exact(buffer)?;
            let (records, remainder) = buffer.as_chunks::<NODE_RECORD_LEN>();
            debug_assert!(remainder.is_empty());
            for record in records {
                let osm_id = i64::from_le_bytes(record[0..8].try_into().expect("fixed slice"));
                let lat = f64::from_le_bytes(record[8..16].try_into().expect("fixed slice"));
                let lon = f64::from_le_bytes(record[16..24].try_into().expect("fixed slice"));
                if !lat.is_finite() || !lon.is_finite() {
                    return Err(GraphError::Corrupt("node coordinate is not finite".into()));
                }
                if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                    return Err(GraphError::Corrupt(format!(
                        "node {osm_id} coordinate out of range: {lat},{lon}"
                    )));
                }
                nodes.push(Node { osm_id, lat, lon });
            }
            remaining -= take as u64;
        }

        let mut edges: Vec<Edge> = Vec::with_capacity(edge_count as usize);
        let mut offsets = vec![0u32; nodes.len() + 1];
        let mut chunk = vec![0u8; EDGE_RECORD_LEN * LOAD_CHUNK_RECORDS];
        let mut remaining = edge_count;
        let mut previous_from = 0u32;
        let mut first = true;
        while remaining > 0 {
            let take = remaining.min(LOAD_CHUNK_RECORDS as u64) as usize;
            let buffer = &mut chunk[..take * EDGE_RECORD_LEN];
            hashing.read_exact(buffer)?;
            let (records, remainder) = buffer.as_chunks::<EDGE_RECORD_LEN>();
            debug_assert!(remainder.is_empty());
            for record in records {
                let from = u32::from_le_bytes(record[0..4].try_into().expect("fixed slice"));
                let to = u32::from_le_bytes(record[4..8].try_into().expect("fixed slice"));
                let distance_m = f32::from_le_bytes(record[8..12].try_into().expect("fixed slice"));
                let speed_kmh = f32::from_le_bytes(record[12..16].try_into().expect("fixed slice"));
                let class = RoadClass::from_u8(record[16]).ok_or_else(|| {
                    GraphError::Corrupt(format!("unknown road class {}", record[16]))
                })?;
                let flags = record[17];
                let name = u32::from_le_bytes(record[20..24].try_into().expect("fixed slice"));

                if from as usize >= nodes.len() || to as usize >= nodes.len() {
                    return Err(GraphError::Corrupt(
                        "edge endpoint outside the node table".into(),
                    ));
                }
                if !distance_m.is_finite() || distance_m < 0.0 {
                    return Err(GraphError::Corrupt(
                        "edge distance is not finite and non-negative".into(),
                    ));
                }
                if !speed_kmh.is_finite() || speed_kmh <= 0.0 {
                    return Err(GraphError::Corrupt(
                        "edge speed is not finite and positive".into(),
                    ));
                }
                if name != NO_NAME && name as usize >= names.len() {
                    return Err(GraphError::Corrupt(
                        "edge name id outside the name table".into(),
                    ));
                }
                if !first && from < previous_from {
                    return Err(GraphError::Corrupt(
                        "edges are not grouped by source node".into(),
                    ));
                }
                first = false;
                previous_from = from;
                offsets[from as usize + 1] += 1;
                edges.push(Edge {
                    from,
                    to,
                    distance_m,
                    speed_kmh,
                    class,
                    flags,
                    name,
                });
            }
            remaining -= take as u64;
        }

        if hashing.bytes != payload_len {
            return Err(GraphError::Truncated {
                what: "payload",
                expected: payload_len,
                actual: hashing.bytes,
            });
        }
        let actual_crc = hashing.crc.finish();
        if actual_crc != expected_crc {
            return Err(GraphError::ChecksumMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        for i in 0..nodes.len() {
            offsets[i + 1] += offsets[i];
        }

        Ok(Self::finish(nodes, offsets, edges, names))
    }

    fn write_file(&self, path: &Path) -> Result<(), GraphError> {
        if self.names.len() >= NO_NAME as usize {
            return Err(GraphError::Corrupt(
                "name table is too large for u32 ids".into(),
            ));
        }
        let file = File::create(path)?;
        let mut file = BufWriter::with_capacity(1 << 20, file);

        // Reserve the header; it is rewritten once the payload length and CRC
        // are known. It is deliberately outside the CRC: the checksum covers
        // the payload only.
        file.write_all(&[0u8; GRAPH_HEADER_LEN])?;
        let mut writer = CrcWriter::new(file);

        for name in &self.names {
            let bytes = name.as_bytes();
            if bytes.len() > MAX_NAME_BYTES as usize {
                return Err(GraphError::Corrupt(format!(
                    "name `{name}` exceeds the {MAX_NAME_BYTES}-byte limit"
                )));
            }
            writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
            writer.write_all(bytes)?;
        }

        let mut record = [0u8; NODE_RECORD_LEN];
        for node in &self.nodes {
            record[0..8].copy_from_slice(&node.osm_id.to_le_bytes());
            record[8..16].copy_from_slice(&node.lat.to_le_bytes());
            record[16..24].copy_from_slice(&node.lon.to_le_bytes());
            writer.write_all(&record)?;
        }

        for edge in &self.edges {
            encode_edge(edge, &mut record);
            writer.write_all(&record)?;
        }

        writer.flush()?;
        let payload_len = writer.bytes;
        let crc = writer.crc.finish();
        let mut file = writer
            .inner
            .into_inner()
            .map_err(|err| GraphError::Io(err.into_error()))?;

        let header = encode_header(
            self.nodes.len() as u64,
            self.edges.len() as u64,
            self.names.len() as u64,
            payload_len,
            crc,
        );
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&header)?;
        file.sync_all()?;
        Ok(())
    }
}

fn encode_edge(edge: &Edge, record: &mut [u8; EDGE_RECORD_LEN]) {
    record[0..4].copy_from_slice(&edge.from.to_le_bytes());
    record[4..8].copy_from_slice(&edge.to.to_le_bytes());
    record[8..12].copy_from_slice(&edge.distance_m.to_le_bytes());
    record[12..16].copy_from_slice(&edge.speed_kmh.to_le_bytes());
    record[16] = edge.class as u8;
    record[17] = edge.flags;
    record[18..20].copy_from_slice(&0u16.to_le_bytes());
    record[20..24].copy_from_slice(&edge.name.to_le_bytes());
}

fn encode_header(
    node_count: u64,
    edge_count: u64,
    name_count: u64,
    payload_len: u64,
    crc: u32,
) -> [u8; GRAPH_HEADER_LEN] {
    let mut header = [0u8; GRAPH_HEADER_LEN];
    header[0..8].copy_from_slice(&GRAPH_MAGIC);
    header[8..12].copy_from_slice(&GRAPH_FORMAT_VERSION.to_le_bytes());
    header[16..24].copy_from_slice(&node_count.to_le_bytes());
    header[24..32].copy_from_slice(&edge_count.to_le_bytes());
    header[32..40].copy_from_slice(&name_count.to_le_bytes());
    header[40..48].copy_from_slice(&payload_len.to_le_bytes());
    header[48..52].copy_from_slice(&crc.to_le_bytes());
    header
}

fn read_raw<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<(), GraphError> {
    reader.read_exact(buffer).map_err(|err| {
        if err.kind() == io::ErrorKind::UnexpectedEof {
            GraphError::Truncated {
                what: "header",
                expected: buffer.len() as u64,
                actual: 0,
            }
        } else {
            GraphError::Io(err)
        }
    })
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(path: &Path) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "graph.bin".into());
    path.with_file_name(format!(".{file_name}.tmp-{}-{counter}", std::process::id()))
}

/// Persisted-graph errors.
#[derive(Debug)]
pub enum GraphError {
    Io(io::Error),
    BadMagic([u8; 8]),
    UnsupportedVersion(u32),
    Truncated {
        what: &'static str,
        expected: u64,
        actual: u64,
    },
    ChecksumMismatch {
        expected: u32,
        actual: u32,
    },
    Corrupt(String),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::Io(err) => write!(f, "I/O error: {err}"),
            GraphError::BadMagic(magic) => write!(
                f,
                "bad graph magic {:?}: not a native-router graph file",
                String::from_utf8_lossy(magic)
            ),
            GraphError::UnsupportedVersion(version) => write!(
                f,
                "unsupported graph format version {version} (this build reads {GRAPH_FORMAT_VERSION})"
            ),
            GraphError::Truncated {
                what,
                expected,
                actual,
            } => write!(
                f,
                "truncated graph file: {what} needs {expected} bytes, found {actual}"
            ),
            GraphError::ChecksumMismatch { expected, actual } => write!(
                f,
                "graph checksum mismatch: header CRC {expected:#010x}, payload CRC {actual:#010x}"
            ),
            GraphError::Corrupt(message) => write!(f, "corrupt graph file: {message}"),
        }
    }
}

impl std::error::Error for GraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GraphError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for GraphError {
    fn from(err: io::Error) -> Self {
        GraphError::Io(err)
    }
}

/// Total order for heap entries whose `f64` scores are never NaN.
pub(crate) fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.total_cmp(&b)
}

// ---------------------------------------------------------------------------
// CRC-32 (IEEE 802.3, reflected polynomial 0xEDB88320).
//
// A 256-entry table keeps hashing ~100 MB in the tens of milliseconds without
// pulling a dependency into the mobile build.
// ---------------------------------------------------------------------------

const CRC32_TABLE: [u32; 256] = build_crc32_table();

const fn build_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0;
    while index < 256 {
        let mut crc = index as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
}

#[derive(Debug, Clone, Copy)]
struct Crc32 {
    value: u32,
}

impl Crc32 {
    fn new() -> Self {
        Self { value: u32::MAX }
    }

    fn update(&mut self, bytes: &[u8]) {
        let mut crc = self.value;
        for &byte in bytes {
            let index = ((crc ^ u32::from(byte)) & 0xff) as usize;
            crc = CRC32_TABLE[index] ^ (crc >> 8);
        }
        self.value = crc;
    }

    fn finish(self) -> u32 {
        !self.value
    }
}

struct CrcWriter<W: Write> {
    inner: W,
    crc: Crc32,
    bytes: u64,
}

impl<W: Write> CrcWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            crc: Crc32::new(),
            bytes: 0,
        }
    }
}

impl<W: Write> Write for CrcWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.crc.update(&buffer[..written]);
        self.bytes += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct CrcReader<R: Read> {
    inner: R,
    crc: Crc32,
    bytes: u64,
}

impl<R: Read> CrcReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            crc: Crc32::new(),
            bytes: 0,
        }
    }

    fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), GraphError> {
        self.inner.read_exact(buffer).map_err(|err| {
            if err.kind() == io::ErrorKind::UnexpectedEof {
                GraphError::Truncated {
                    what: "payload",
                    expected: self.bytes + buffer.len() as u64,
                    actual: self.bytes,
                }
            } else {
                GraphError::Io(err)
            }
        })?;
        self.crc.update(buffer);
        self.bytes += buffer.len() as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(osm_id: i64, lat: f64, lon: f64) -> Node {
        Node { osm_id, lat, lon }
    }

    fn edge(from: u32, to: u32, distance_m: f32, speed_kmh: f32) -> Edge {
        Edge::new(from, to, distance_m, speed_kmh, RoadClass::Residential)
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
            edge(0, 1, 75.0, 50.0),
            edge(0, 2, 150.0, 50.0),
            edge(1, 2, 75.0, 50.0),
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

    #[test]
    fn resolves_interned_names_and_the_no_name_sentinel() {
        let nodes = vec![node(1, 47.0, 28.0), node(2, 47.0, 28.001)];
        let names = vec!["Strada Ștefan cel Mare".into(), "M14".into()];
        let named = Edge {
            name: 0,
            ..edge(0, 1, 75.0, 50.0)
        };
        let unnamed = edge(1, 0, 75.0, 50.0);
        let graph = Graph::from_edges_with_names(nodes, vec![named, unnamed], names);
        assert_eq!(graph.name(0), Some("Strada Ștefan cel Mare"));
        assert_eq!(graph.name(1), Some("M14"));
        assert_eq!(graph.name(NO_NAME), None);
        assert_eq!(graph.name(99), None);
        assert_eq!(
            graph.edge_name(&graph.edges_from(0)[0]),
            Some("Strada Ștefan cel Mare")
        );
        assert_eq!(graph.edge_name(&graph.edges_from(1)[0]), None);
        assert_eq!(graph.edge_index_base(1), 1);
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        // CRC-32/ISO-HDLC of "123456789" is 0xCBF43926.
        let mut crc = Crc32::new();
        crc.update(b"123456789");
        assert_eq!(crc.finish(), 0xCBF4_3926);
    }
}

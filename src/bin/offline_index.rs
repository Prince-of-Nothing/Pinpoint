//! Build a SQLite FTS5 place index from an OSM PBF extract.
//!
//! This binary is behind the `offline-build` Cargo feature so the normal app
//! build does not link SQLite or the PBF reader.
//!
//! Usage:
//!   pinpoint-offline-index --osm <file.osm.pbf> --out <file.sqlite>
//!   pinpoint-offline-index --verify <file.sqlite>
//!   pinpoint-offline-index --fixture <file.sqlite>

use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use osmpbfreader::{OsmObj, OsmPbfReader};
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub id: i64,
    pub name: String,
    pub normalized: String,
    pub aliases: String,
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
    pub address: Option<String>,
    pub importance: f64,
}

const NAME_KEYS: &[&str] = &[
    "name",
    "name:ro",
    "name:ru",
    "name:uk",
    "name:bg",
    "name:gag",
    "name:en",
    "alt_name",
    "official_name",
    "short_name",
];

const DISPLAY_KEYS: &[&str] = &[
    "name:ro", "name:ru", "name:uk", "name:bg", "name:gag", "name:en", "name",
];

pub fn normalize_name(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let mapped = match ch {
            'ă' | 'Ă' | 'â' | 'Â' => 'a',
            'î' | 'Î' => 'i',
            'ș' | 'Ș' | 'ş' | 'Ş' => 's',
            'ț' | 'Ț' | 'ţ' | 'Ţ' => 't',
            'ё' | 'Ё' => 'е',
            other => other,
        };
        for lower in mapped.to_lowercase() {
            out.push(lower);
        }
    }
    out
}

/// Turns a free-form query into safe FTS5 prefix terms. Punctuation is a word
/// separator, never query syntax, so `"str. Ștefan"`, `"balti-oras"` and
/// `"balti'"` cannot produce an FTS syntax error.
fn search_terms(query: &str) -> Vec<String> {
    normalize_name(query)
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| format!("{token}*"))
        .collect()
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS places (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            normalized TEXT NOT NULL,
            aliases TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL,
            lat REAL NOT NULL,
            lon REAL NOT NULL,
            address TEXT,
            importance REAL NOT NULL DEFAULT 0
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS places_fts USING fts5(
            name, normalized, aliases, address,
            content='places', content_rowid='id',
            tokenize='unicode61 remove_diacritics 2'
        );
        CREATE TRIGGER IF NOT EXISTS places_ai AFTER INSERT ON places BEGIN
            INSERT INTO places_fts(rowid, name, normalized, aliases, address)
            VALUES (new.id, new.name, new.normalized, new.aliases, new.address);
        END;
        CREATE TRIGGER IF NOT EXISTS places_ad AFTER DELETE ON places BEGIN
            INSERT INTO places_fts(places_fts, rowid, name, normalized, aliases, address)
            VALUES ('delete', old.id, old.name, old.normalized, old.aliases, old.address);
        END;",
    )
}

pub fn insert_places(conn: &mut Connection, places: &[Place]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO places
             (id, name, normalized, aliases, kind, lat, lon, address, importance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for place in places {
            stmt.execute(params![
                place.id,
                place.name,
                place.normalized,
                place.aliases,
                place.kind,
                place.lat,
                place.lon,
                place.address,
                place.importance
            ])?;
        }
    }
    tx.commit()
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> rusqlite::Result<Vec<Place>> {
    let match_query = search_terms(query).join(" ");
    if match_query.is_empty() {
        return Ok(Vec::new());
    }

    // BM25's `rank` is negative (lower is a better match). Importance shifts a
    // place by up to 10 rank points so a known city such as Bălți outranks a
    // bar or payment terminal that happens to share the same search term.
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.normalized, p.aliases, p.kind, p.lat, p.lon, p.address, p.importance
         FROM places_fts JOIN places p ON p.id = places_fts.rowid
         WHERE places_fts MATCH ?1
         ORDER BY (places_fts.rank - p.importance * 10.0)
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![match_query, limit as i64], |row| {
        Ok(Place {
            id: row.get(0)?,
            name: row.get(1)?,
            normalized: row.get(2)?,
            aliases: row.get(3)?,
            kind: row.get(4)?,
            lat: row.get(5)?,
            lon: row.get(6)?,
            address: row.get(7)?,
            importance: row.get(8)?,
        })
    })?;
    rows.collect()
}

fn tag<'a>(node: &'a osmpbfreader::Node, key: &str) -> Option<&'a str> {
    node.tags.get(key).map(|value| value.as_str())
}

fn place_from_node(node: &osmpbfreader::Node) -> Option<Place> {
    let mut names = Vec::new();
    for key in NAME_KEYS {
        if let Some(value) = tag(node, key) {
            if !names.iter().any(|existing| existing == value) {
                names.push(value.to_string());
            }
        }
    }
    let address = match (tag(node, "addr:street"), tag(node, "addr:housenumber")) {
        (Some(street), Some(number)) => Some(format!("{street} {number}")),
        (Some(street), None) => Some(street.to_string()),
        _ => None,
    };

    let (display, kind) = if names.is_empty() {
        // Address-only nodes are still useful search targets, so use the
        // postal address as the display name.
        let address = address.as_deref()?;
        (address.to_string(), "address".to_string())
    } else {
        let display = DISPLAY_KEYS
            .iter()
            .find_map(|key| tag(node, key))
            .map(str::to_string)
            .unwrap_or_else(|| names[0].clone());
        let kind = ["place", "amenity", "shop", "tourism", "leisure"]
            .iter()
            .find_map(|key| tag(node, key))
            .unwrap_or("place")
            .to_string();
        (display, kind)
    };

    let place_tag = tag(node, "place").unwrap_or("");
    let base = match place_tag {
        "city" => 0.9,
        "town" => 0.7,
        "village" => 0.5,
        "hamlet" => 0.3,
        _ => 0.1,
    };
    let population = tag(node, "population")
        .and_then(|value| value.replace([' ', ','], "").parse::<f64>().ok())
        .unwrap_or(0.0);
    let importance = (base + (population / 1_000_000.0).min(0.5)).min(1.0);

    Some(Place {
        id: node.id.0,
        name: display.clone(),
        normalized: normalize_name(&display),
        aliases: names.join(" "),
        kind,
        lat: node.lat(),
        lon: node.lon(),
        address,
        importance,
    })
}

/// OSM PBF starts with a 4-byte big-endian `BlobHeader` length; the reader
/// rejects anything above 64 KiB. Checking it here turns obvious garbage and
/// short files into a clear error before the parser runs.
const PBF_MAX_HEADER_BYTES: u64 = 64 * 1024;

fn validate_pbf(path: &Path) -> Result<()> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("stat OSM extract {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("OSM extract is not a file: {}", path.display());
    }
    let size = metadata.len();
    if size == 0 {
        anyhow::bail!("OSM extract is empty: {}", path.display());
    }
    if size < 8 {
        anyhow::bail!("OSM extract is too short to be a PBF: {}", path.display());
    }

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("open OSM extract {}", path.display()))?;
    let mut header_length = [0u8; 4];
    file.read_exact(&mut header_length)
        .with_context(|| format!("read OSM PBF header {}", path.display()))?;
    let header_length = u32::from_be_bytes(header_length) as u64;
    if header_length == 0 || header_length > PBF_MAX_HEADER_BYTES || header_length + 4 > size {
        anyhow::bail!(
            "not a valid OSM PBF (bad first blob header length {header_length}): {}",
            path.display()
        );
    }
    Ok(())
}

fn parse_places(path: &Path) -> Result<Vec<Place>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("open OSM extract {}", path.display()))?;
    let mut pbf = OsmPbfReader::new(file);
    let mut places = Vec::new();
    for object in pbf.iter() {
        if let OsmObj::Node(node) = object.context("read OSM object")? {
            if let Some(place) = place_from_node(&node) {
                places.push(place);
            }
        }
    }
    Ok(places)
}

fn read_places(path: &Path) -> Result<Vec<Place>> {
    validate_pbf(path)?;
    // osmpbfreader asserts on a short blob instead of returning an error for
    // truncated files; convert that panic into a normal `anyhow` failure.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_places(path))) {
        Ok(places) => places,
        Err(_) => Err(anyhow::anyhow!(
            "PBF is truncated or corrupt: {}",
            path.display()
        )),
    }
}

fn fixture_places() -> Vec<Place> {
    vec![
        Place {
            id: 1,
            name: "Bălți".into(),
            normalized: normalize_name("Bălți"),
            aliases: "Bălți Bălți".into(),
            kind: "city".into(),
            lat: 47.76,
            lon: 27.93,
            address: None,
            importance: 0.9,
        },
        Place {
            id: 2,
            name: "Кишинёв".into(),
            normalized: normalize_name("Кишинёв"),
            aliases: "Кишинёв Кишинэу".into(),
            kind: "city".into(),
            lat: 47.01,
            lon: 28.86,
            address: None,
            importance: 0.9,
        },
    ]
}

fn verify_index(path: &Path) -> Result<()> {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open index {}", path.display()))?;
    let places: i64 = conn.query_row("SELECT count(*) FROM places", [], |row| row.get(0))?;
    let fts: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'places_fts'",
        [],
        |row| row.get(0),
    )?;
    println!("{{\"places\":{places},\"fts\":{fts}}}");
    if places == 0 || fts == 0 {
        anyhow::bail!("index is empty or missing the places_fts table");
    }
    Ok(())
}

fn build_index(osm: &Path, out: &Path) -> Result<usize> {
    let places = read_places(osm)?;
    if places.is_empty() {
        anyhow::bail!("no named places found; PBF empty, truncated or not an OSM extract");
    }

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create output directory {}", parent.display()))?;
        }
    }

    let temporary = out.with_extension("sqlite.part");
    let _ = std::fs::remove_file(&temporary);
    if let Err(error) = write_index(&temporary, &places) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, out) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).with_context(|| format!("publish index {}", out.display()));
    }
    Ok(places.len())
}

fn write_index(path: &Path, places: &[Place]) -> Result<()> {
    let mut conn =
        Connection::open(path).with_context(|| format!("create index {}", path.display()))?;
    init_schema(&conn)?;
    insert_places(&mut conn, places)?;
    drop(conn);
    Ok(())
}

fn fixture_index(path: &Path) -> Result<()> {
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    init_schema(&conn)?;
    insert_places(&mut conn, &fixture_places())?;
    println!("{{\"places\":2}}");
    Ok(())
}

fn print_help() {
    println!(
        "pinpoint-offline-index\n\
         \n\
         --osm <file.osm.pbf> --out <file.sqlite>   build the search index\n\
         --verify <file.sqlite>                     check an existing index\n\
         --fixture <file.sqlite>                    write the two-place test fixture"
    );
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut osm: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut verify: Option<PathBuf> = None;
    let mut fixture: Option<PathBuf> = None;

    let mut index = 1;
    while index < args.len() {
        let argument = args[index].as_str();
        let value = |index: usize| -> Result<String> {
            args.get(index + 1)
                .cloned()
                .with_context(|| format!("{argument} needs a path"))
        };
        match argument {
            "--osm" => {
                osm = Some(PathBuf::from(value(index)?));
                index += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(value(index)?));
                index += 2;
            }
            "--verify" => {
                verify = Some(PathBuf::from(value(index)?));
                index += 2;
            }
            "--fixture" => {
                fixture = Some(PathBuf::from(value(index)?));
                index += 2;
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }

    if let Some(path) = verify {
        return verify_index(&path);
    }
    if let Some(path) = fixture {
        return fixture_index(&path);
    }

    let osm = osm.context("--osm is required")?;
    let out = out.context("--out is required")?;
    let count = build_index(&osm, &out)?;
    println!("{{\"places\":{count}}}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_romanian_diacritics() {
        assert_eq!(normalize_name("Bălți"), "balti");
        assert_eq!(normalize_name("Chișinău"), "chisinau");
    }

    #[test]
    fn folds_cedilla_diacritics() {
        assert_eq!(normalize_name("Ştefan Ţara"), "stefan tara");
        assert_eq!(normalize_name("ŞTEFAN"), "stefan");
    }

    #[test]
    fn keeps_cyrillic_searchable() {
        assert_eq!(normalize_name("Кишинёв"), "кишинев");
    }

    #[test]
    fn fts_finds_by_normalized_and_cyrillic() {
        let conn = memory_db(&[
            place(1, "Bălți", "balti", 47.76, 27.93),
            place(2, "Кишинёв", "кишинев", 47.01, 28.86),
        ]);
        let ro = search(&conn, "balti", 10).unwrap();
        assert_eq!(ro[0].name, "Bălți");
        let ru = search(&conn, "кишин", 10).unwrap();
        assert_eq!(ru[0].name, "Кишинёв");
    }

    #[test]
    fn tokenizes_punctuation_in_queries() {
        let conn = memory_db(&[
            place(1, "Bălți", "balti", 47.76, 27.93),
            place(2, "Café-Bar", "café-bar", 47.75, 27.94),
            place(
                3,
                "Str. Ștefan cel Mare 1",
                "str. stefan cel mare 1",
                47.74,
                27.95,
            ),
            place(4, "Кишинёв", "кишинев", 47.01, 28.86),
        ]);

        assert_eq!(search(&conn, "balti-", 10).unwrap()[0].name, "Bălți");
        assert_eq!(search(&conn, "balti'", 10).unwrap()[0].name, "Bălți");
        assert_eq!(search(&conn, "(balti)", 10).unwrap()[0].name, "Bălți");
        assert_eq!(search(&conn, "Café-Bar", 10).unwrap()[0].name, "Café-Bar");
        assert_eq!(
            search(&conn, "str. Ștefan cel Mare 1", 10).unwrap()[0].name,
            "Str. Ștefan cel Mare 1"
        );
        assert_eq!(
            search(&conn, "str. Ștefan, cel Mare", 10).unwrap()[0].name,
            "Str. Ștefan cel Mare 1"
        );
        assert_eq!(search(&conn, "кишинёв,", 10).unwrap()[0].name, "Кишинёв");
        assert!(search(&conn, "balti-oras", 10).unwrap().is_empty());
        assert!(search(&conn, "!!! ... --- ???", 10).unwrap().is_empty());
    }

    #[test]
    fn importance_outranks_noisy_match() {
        let noisy = Place {
            id: 1,
            name: "BALTI".into(),
            normalized: "balti".into(),
            aliases: "BALTI".into(),
            kind: "bar".into(),
            lat: 47.0,
            lon: 28.0,
            address: None,
            importance: 0.1,
        };
        let city = Place {
            id: 2,
            name: "Bălți".into(),
            normalized: "balti".into(),
            aliases: "Bălți".into(),
            kind: "city".into(),
            lat: 47.76,
            lon: 27.93,
            address: None,
            importance: 1.0,
        };
        let conn = memory_db(&[noisy, city]);

        let results = search(&conn, "balti", 10).unwrap();
        assert_eq!(results[0].name, "Bălți");
    }

    #[test]
    fn indexes_address_only_nodes() {
        let with_number = node(
            42,
            &[
                ("addr:street", "Strada Ștefan cel Mare"),
                ("addr:housenumber", "1"),
            ],
        );
        let place = place_from_node(&with_number).expect("address-only node should be indexed");
        assert_eq!(place.name, "Strada Ștefan cel Mare 1");
        assert_eq!(place.normalized, "strada stefan cel mare 1");
        assert_eq!(place.kind, "address");
        assert_eq!(place.address.as_deref(), Some("Strada Ștefan cel Mare 1"));

        let street_only = node(43, &[("addr:street", "Strada Mare")]);
        let place = place_from_node(&street_only).expect("street-only node should be indexed");
        assert_eq!(place.name, "Strada Mare");
        assert_eq!(place.kind, "address");

        let conn = memory_db(&[place]);
        let results = search(&conn, "strada mare", 10).unwrap();
        assert_eq!(results[0].kind, "address");
    }

    #[test]
    fn parses_population_with_commas() {
        let with_comma = node(
            7,
            &[
                ("place", "hamlet"),
                ("name", "Test"),
                ("population", "10,000"),
            ],
        );
        let place = place_from_node(&with_comma).unwrap();
        assert!((place.importance - 0.31).abs() < 1e-9);

        let with_space = node(
            8,
            &[
                ("place", "hamlet"),
                ("name", "Test"),
                ("population", "10 000"),
            ],
        );
        let place = place_from_node(&with_space).unwrap();
        assert!((place.importance - 0.31).abs() < 1e-9);
    }

    #[test]
    fn empty_pbf_is_rejected_without_output() {
        let dir = temp_dir("empty");
        let osm = dir.join("empty.osm.pbf");
        std::fs::write(&osm, []).unwrap();
        let out = dir.join("index.sqlite");

        let error = build_index(&osm, &out).unwrap_err();
        assert!(
            format!("{error:#}").contains("empty"),
            "unexpected error: {error:#}"
        );
        assert!(!out.exists());
        assert!(!part_path(&out).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn garbage_pbf_is_rejected_without_output() {
        let dir = temp_dir("garbage");
        let osm = dir.join("garbage.osm.pbf");
        std::fs::write(&osm, [0u8; 64]).unwrap();
        let out = dir.join("index.sqlite");

        let error = build_index(&osm, &out).unwrap_err();
        assert!(
            format!("{error:#}").contains("PBF"),
            "unexpected error: {error:#}"
        );
        assert!(!out.exists());
        assert!(!part_path(&out).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncated_pbf_is_rejected_without_panic_or_output() {
        let source = monaco_fixture();
        let bytes = std::fs::read(&source).unwrap();
        assert!(bytes.len() > 1024, "fixture should not be tiny");

        let dir = temp_dir("truncated");
        let osm = dir.join("truncated.osm.pbf");
        std::fs::write(&osm, &bytes[..bytes.len() / 2]).unwrap();
        let out = dir.join("index.sqlite");

        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build_index(&osm, &out)));
        let error = result
            .expect("build_index must not panic on a truncated PBF")
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("truncated or corrupt"),
            "unexpected error: {error:#}"
        );
        assert!(!out.exists());
        assert!(!part_path(&out).exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn monaco_fixture_has_searchable_places() {
        let places = read_places(&monaco_fixture()).expect("read committed Monaco fixture");
        assert!(
            !places.is_empty(),
            "Monaco fixture should contain named places"
        );

        let conn = memory_db(&places);
        let results = search(&conn, "monaco", 10).unwrap();
        assert!(
            !results.is_empty(),
            "search should find a known Monaco place"
        );
    }

    #[test]
    fn builds_index_into_missing_parent_directory() {
        let dir = temp_dir("nested");
        let out = dir.join("nested").join("deeper").join("index.sqlite");

        let count = build_index(&monaco_fixture(), &out).unwrap();
        assert!(count > 0);
        assert!(out.exists());

        let conn =
            Connection::open_with_flags(&out, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let places: i64 = conn
            .query_row("SELECT count(*) FROM places", [], |row| row.get(0))
            .unwrap();
        assert_eq!(places, count as i64);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn memory_db(places: &[Place]) -> Connection {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        insert_places(&mut conn, places).unwrap();
        conn
    }

    fn monaco_fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/offline/tests/fixtures/monaco.osm.pbf")
    }

    fn part_path(out: &Path) -> PathBuf {
        out.with_extension("sqlite.part")
    }

    fn temp_dir(label: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);

        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-offline-index-{label}-{}-{nanos}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn node(id: i64, tags: &[(&str, &str)]) -> osmpbfreader::Node {
        osmpbfreader::Node {
            id: osmpbfreader::NodeId(id),
            tags: osmpbfreader::Tags::from_iter(
                tags.iter()
                    .map(|(key, value)| ((*key).into(), (*value).into())),
            ),
            decimicro_lat: 477_600_000,
            decimicro_lon: 279_300_000,
        }
    }

    fn place(id: i64, name: &str, normalized: &str, lat: f64, lon: f64) -> Place {
        Place {
            id,
            name: name.into(),
            normalized: normalized.into(),
            aliases: name.into(),
            kind: "place".into(),
            lat,
            lon,
            address: None,
            importance: 0.5,
        }
    }
}

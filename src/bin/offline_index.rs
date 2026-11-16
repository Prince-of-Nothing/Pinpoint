//! Build a SQLite FTS5 place index from an OSM PBF extract.
//!
//! This binary is behind the `offline-build` Cargo feature so the normal app
//! build does not link SQLite or the PBF reader.
//!
//! Usage:
//!   pinpoint-offline-index --osm <file.osm.pbf> --out <file.sqlite>
//!   pinpoint-offline-index --verify <file.sqlite>
//!   pinpoint-offline-index --fixture <file.sqlite>

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
            'ș' | 'Ș' => 's',
            'ț' | 'Ț' => 't',
            'ё' | 'Ё' => 'е',
            other => other,
        };
        for lower in mapped.to_lowercase() {
            out.push(lower);
        }
    }
    out
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
    let normalized = normalize_name(query);
    let match_query = normalized
        .split_whitespace()
        .map(|token| format!("{token}*"))
        .collect::<Vec<_>>()
        .join(" ");
    if match_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.normalized, p.aliases, p.kind, p.lat, p.lon, p.address, p.importance
         FROM places_fts JOIN places p ON p.id = places_fts.rowid
         WHERE places_fts MATCH ?1
         ORDER BY rank
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
    if names.is_empty() {
        return None;
    }

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

    let place_tag = tag(node, "place").unwrap_or("");
    let base = match place_tag {
        "city" => 0.9,
        "town" => 0.7,
        "village" => 0.5,
        "hamlet" => 0.3,
        _ => 0.1,
    };
    let population = tag(node, "population")
        .and_then(|value| value.replace(' ', "").parse::<f64>().ok())
        .unwrap_or(0.0);
    let importance = (base + (population / 1_000_000.0).min(0.5)).min(1.0);

    let address = match (tag(node, "addr:street"), tag(node, "addr:housenumber")) {
        (Some(street), Some(number)) => Some(format!("{street} {number}")),
        (Some(street), None) => Some(street.to_string()),
        _ => None,
    };

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

fn read_places(path: &Path) -> Result<Vec<Place>> {
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
    let temporary = out.with_extension("sqlite.part");
    let _ = std::fs::remove_file(&temporary);
    let mut conn = Connection::open(&temporary)
        .with_context(|| format!("create index {}", temporary.display()))?;
    init_schema(&conn)?;
    insert_places(&mut conn, &places)?;
    drop(conn);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&temporary, out).with_context(|| format!("publish index {}", out.display()))?;
    Ok(places.len())
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
    fn keeps_cyrillic_searchable() {
        assert_eq!(normalize_name("Кишинёв"), "кишинев");
    }

    #[test]
    fn fts_finds_by_normalized_and_cyrillic() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        insert_places(
            &mut conn,
            &[
                place(1, "Bălți", "balti", 47.76, 27.93),
                place(2, "Кишинёв", "кишинев", 47.01, 28.86),
            ],
        )
        .unwrap();
        let ro = search(&conn, "balti", 10).unwrap();
        assert_eq!(ro[0].name, "Bălți");
        let ru = search(&conn, "кишин", 10).unwrap();
        assert_eq!(ru[0].name, "Кишинёв");
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

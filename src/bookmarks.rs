//! On-device bookmarks in pure Rust — the multiplatform replacement for the
//! archived web (`js/bookmarks.js`, localStorage) and Kotlin
//! (`BookmarkRepositoryImpl`, DataStore). Same semantics everywhere:
//! manually-starred places only, stored as JSON next to the app data dir,
//! never synced, never containing friend/shared locations.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Maximum number of new places accepted from one GeoJSON import.
pub const MAX_IMPORT: usize = 500;

/// Result of a GeoJSON import.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub imported: usize,
    pub duplicates: usize,
    pub invalid: usize,
    pub over_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub created_at: String,
}

pub struct BookmarkStore {
    path: Option<std::path::PathBuf>,
}

impl BookmarkStore {
    /// `None` on targets without a writable app dir (wasm/iOS sandbox) —
    /// the store then keeps everything in memory.
    pub fn new(dir: Option<std::path::PathBuf>) -> Self {
        let path = dir.map(|d| d.join("bookmarks.json"));
        Self { path }
    }

    /// Default store under [`crate::platform::app_data_dir`].
    pub fn default_location() -> Self {
        Self::new(crate::platform::app_data_dir())
    }

    pub fn list(&self) -> Vec<Bookmark> {
        let Some(path) = &self.path else {
            return Vec::new();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    /// Saved places as map pins: `(name, latitude, longitude)`.
    pub fn map_pins(&self) -> Vec<(String, f64, f64)> {
        self.list()
            .into_iter()
            .map(|bookmark| (bookmark.name, bookmark.latitude, bookmark.longitude))
            .collect()
    }

    pub fn add(&self, name: &str, latitude: f64, longitude: f64) -> Bookmark {
        let mut list = self.list();
        let bm = Bookmark {
            id: format!("bm_{}", list.len() + 1),
            name: name.to_string(),
            latitude,
            longitude,
            note: String::new(),
            category: String::new(),
            created_at: String::new(),
        };
        list.insert(0, bm.clone());
        self.persist(&list);
        bm
    }

    pub fn remove(&self, id: &str) {
        let next: Vec<Bookmark> = self.list().into_iter().filter(|b| b.id != id).collect();
        self.persist(&next);
    }

    /// Export all saved bookmarks as an RFC 7946 GeoJSON `FeatureCollection`
    /// (`coordinates: [longitude, latitude]`).
    pub fn to_geojson(&self) -> String {
        let features: Vec<serde_json::Value> = self
            .list()
            .into_iter()
            .map(|b| {
                serde_json::json!({
                    "type": "Feature",
                    "geometry": {
                        "type": "Point",
                        "coordinates": [b.longitude, b.latitude],
                    },
                    "properties": {
                        "id": b.id,
                        "name": b.name,
                        "note": b.note,
                        "category": b.category,
                        "created_at": b.created_at,
                    }
                })
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::json!({
            "type": "FeatureCollection",
            "features": features,
        }))
        .unwrap_or_else(|_| r#"{"type":"FeatureCollection","features":[]}"#.to_string())
    }

    /// Import bookmarks from an RFC 7946 GeoJSON `FeatureCollection`.
    /// Validates every Point coordinate pair fail-closed, rejects non-Point
    /// geometries, deduplicates against existing places and within the batch,
    /// preserves names/notes/categories, and writes once at the end.
    pub fn import_geojson(&self, raw: &str) -> Result<ImportSummary, String> {
        if self.path.is_none() {
            return Err("Import needs a writable app data directory.".to_string());
        }
        let doc: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| format!("Invalid GeoJSON: {e}"))?;
        if doc.get("type").and_then(|t| t.as_str()) != Some("FeatureCollection") {
            return Err("Expected a GeoJSON FeatureCollection.".to_string());
        }
        let features = doc
            .get("features")
            .and_then(|f| f.as_array())
            .ok_or_else(|| "GeoJSON missing 'features' array".to_string())?;
        if features.is_empty() {
            return Err("GeoJSON has no features.".to_string());
        }

        let existing = self.list();
        let mut seen: HashSet<String> = existing.iter().map(dedupe_key).collect();
        let mut imported = Vec::new();
        let mut summary = ImportSummary::default();
        let stamp = chrono::Utc::now();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        for (index, feature) in features.iter().enumerate() {
            if summary.imported >= MAX_IMPORT {
                summary.over_limit = features.len() - index;
                break;
            }
            let geometry = feature.get("geometry");
            let geometry_type = geometry
                .and_then(|g| g.get("type"))
                .and_then(|t| t.as_str());
            if geometry_type != Some("Point") {
                summary.invalid += 1;
                continue;
            }
            let Some(coords) = geometry
                .and_then(|g| g.get("coordinates"))
                .and_then(|c| c.as_array())
            else {
                summary.invalid += 1;
                continue;
            };
            if coords.len() < 2 {
                summary.invalid += 1;
                continue;
            }
            let (Some(lon), Some(lat)) = (coords[0].as_f64(), coords[1].as_f64()) else {
                summary.invalid += 1;
                continue;
            };
            if !lat.is_finite()
                || !lon.is_finite()
                || !crate::security::validation::validate_location(lat, lon, None).is_valid()
            {
                summary.invalid += 1;
                continue;
            }
            let properties = feature.get("properties");
            let name = properties
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or("Imported place");
            let note = properties
                .and_then(|p| p.get("note"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let category = properties
                .and_then(|p| p.get("category"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            let bookmark = Bookmark {
                id: format!("bm_import_{nanos}_{index}"),
                name: name.to_string(),
                latitude: lat,
                longitude: lon,
                note,
                category,
                created_at: stamp.to_rfc3339(),
            };
            if !seen.insert(dedupe_key(&bookmark)) {
                summary.duplicates += 1;
                continue;
            }
            summary.imported += 1;
            imported.push(bookmark);
        }

        if summary.imported > 0 {
            imported.extend(existing);
            if !self.persist(&imported) {
                return Err("Could not write the imported places.".to_string());
            }
        }
        Ok(summary)
    }

    fn persist(&self, list: &[Bookmark]) -> bool {
        let Some(path) = &self.path else { return false };
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
        match serde_json::to_string_pretty(list) {
            Ok(text) => write_private(path, text.as_bytes()).is_ok(),
            Err(_) => false,
        }
    }
}

/// Saved places are private, so the file is owner-only on Unix like the
/// diagnostics logs.
#[cfg(unix)]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

fn dedupe_key(bookmark: &Bookmark) -> String {
    format!(
        "{}|{}|{}",
        bookmark.name.trim().to_lowercase(),
        (bookmark.latitude * 100_000.0).round() as i64,
        (bookmark.longitude * 100_000.0).round() as i64
    )
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self::default_location()
    }
}

#[cfg(test)]
mod tests {
    use super::{BookmarkStore, ImportSummary};

    #[test]
    fn in_memory_store_add_remove() {
        let store = BookmarkStore::new(None);
        assert!(store.list().is_empty());
        let bm = store.add("Home", 47.01, 28.86);
        assert_eq!(bm.name, "Home");
        // In-memory mode persists nothing — list stays empty by design.
        assert!(store.list().is_empty());
    }

    #[test]
    fn map_pins_mirror_saved_places() {
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-bm-pins-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = BookmarkStore::new(Some(dir.clone()));
        store.add("Home", 47.01, 28.86);
        store.add("Work", 47.02, 28.87);
        let pins = store.map_pins();
        assert_eq!(pins.len(), 2);
        assert!(pins.iter().any(|(name, lat, lon)| name == "Home"
            && (*lat - 47.01).abs() < 1e-9
            && (*lon - 28.86).abs() < 1e-9));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn file_store_round_trip() {
        let dir = std::env::temp_dir().join(format!("pinpoint-bm-{}", std::process::id()));
        let store = BookmarkStore::new(Some(dir.clone()));
        store.add("Cafe", 47.0, 28.8);
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Cafe");
        store.remove(&list[0].id.clone());
        assert!(store.list().is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn geojson_export_import_round_trip() {
        let dir1 = std::env::temp_dir().join(format!("pinpoint-bm-gj1-{}", std::process::id()));
        let dir2 = std::env::temp_dir().join(format!("pinpoint-bm-gj2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir1);
        let _ = std::fs::remove_dir_all(&dir2);
        let store1 = BookmarkStore::new(Some(dir1.clone()));
        store1.add("Valea Morilor", 47.0022, 28.8180);
        let gj = store1.to_geojson();
        assert!(gj.contains("FeatureCollection"));
        assert!(gj.contains("Valea Morilor"));

        let store2 = BookmarkStore::new(Some(dir2.clone()));
        let imported = store2.import_geojson(&gj).unwrap();
        assert_eq!(imported.imported, 1);
        let list2 = store2.list();
        assert_eq!(list2.len(), 1);
        assert_eq!(list2[0].name, "Valea Morilor");
        assert!((list2[0].latitude - 47.0022).abs() < 1e-6);
        assert!((list2[0].longitude - 28.8180).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(dir1);
        let _ = std::fs::remove_dir_all(dir2);
    }

    #[test]
    fn import_dedupes_validates_and_preserves_properties() {
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-bm-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = BookmarkStore::new(Some(dir.clone()));
        let doc = serde_json::json!({
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [28.86, 47.01]},
                    "properties": {"name": " Home ", "note": "gate code", "category": "home"}
                },
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [28.86, 47.01]},
                    "properties": {"name": "home"}
                },
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [200.0, 47.0]},
                    "properties": {"name": "out of range"}
                },
                {
                    "type": "Feature",
                    "geometry": {"type": "LineString", "coordinates": [[0.0, 0.0], [1.0, 1.0]]},
                    "properties": {"name": "line"}
                },
                {
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [28.87, 47.02]},
                    "properties": {}
                }
            ]
        });
        let summary = store.import_geojson(&doc.to_string()).unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                imported: 2,
                duplicates: 1,
                invalid: 2,
                over_limit: 0
            }
        );
        let list = store.list();
        assert_eq!(list.len(), 2);
        let home = list.iter().find(|b| b.name == "Home").unwrap();
        assert_eq!(home.note, "gate code");
        assert_eq!(home.category, "home");
        assert!(list.iter().any(|b| b.name == "Imported place"));

        // Re-importing the same document adds nothing.
        let again = store.import_geojson(&doc.to_string()).unwrap();
        assert_eq!(again.imported, 0);
        assert_eq!(again.duplicates, 3);
        assert_eq!(again.invalid, 2);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn import_rejects_malformed_and_empty_documents() {
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-bm-bad-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = BookmarkStore::new(Some(dir.clone()));
        assert!(store.import_geojson("not json").is_err());
        assert!(store
            .import_geojson(r#"{"type":"Feature","geometry":null}"#)
            .is_err());
        assert!(store
            .import_geojson(r#"{"type":"FeatureCollection","features":[]}"#)
            .is_err());
        assert!(BookmarkStore::new(None).import_geojson("{}").is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn import_is_capped_at_maximum() {
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-bm-cap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let features: Vec<serde_json::Value> = (0..=super::MAX_IMPORT)
            .map(|i| {
                serde_json::json!({
                    "type": "Feature",
                    "geometry": {"type": "Point", "coordinates": [28.0 + i as f64 / 1000.0, 47.0]},
                    "properties": {"name": format!("Place {i}")}
                })
            })
            .collect();
        let doc = serde_json::json!({"type": "FeatureCollection", "features": features});
        let store = BookmarkStore::new(Some(dir.clone()));
        let summary = store.import_geojson(&doc.to_string()).unwrap();
        assert_eq!(summary.imported, super::MAX_IMPORT);
        assert_eq!(summary.over_limit, 1);
        assert_eq!(store.list().len(), super::MAX_IMPORT);
        std::fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn bookmark_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "pinpoint-bm-mode-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = BookmarkStore::new(Some(dir.clone()));
        store.add("Home", 47.01, 28.86);
        let file_mode = std::fs::metadata(dir.join("bookmarks.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

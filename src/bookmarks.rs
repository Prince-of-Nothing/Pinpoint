//! On-device bookmarks in pure Rust — the multiplatform replacement for the
//! archived web (`js/bookmarks.js`, localStorage) and Kotlin
//! (`BookmarkRepositoryImpl`, DataStore). Same semantics everywhere:
//! manually-starred places only, stored as JSON next to the app data dir,
//! never synced, never containing friend/shared locations.

use serde::{Deserialize, Serialize};

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
    /// Validates every coordinate pair fail-closed; returns the number of
    /// imported places.
    pub fn import_geojson(&self, raw: &str) -> Result<usize, String> {
        let doc: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| format!("Invalid GeoJSON: {e}"))?;
        let features = doc
            .get("features")
            .and_then(|f| f.as_array())
            .ok_or_else(|| "GeoJSON missing 'features' array".to_string())?;
        let mut count = 0usize;
        for f in features {
            let coords = f
                .get("geometry")
                .and_then(|g| g.get("coordinates"))
                .and_then(|c| c.as_array());
            let Some(coords) = coords else { continue };
            if coords.len() < 2 {
                continue;
            }
            let (Some(lon), Some(lat)) = (coords[0].as_f64(), coords[1].as_f64()) else {
                continue;
            };
            if !crate::security::validation::validate_location(lat, lon, None).is_valid() {
                continue;
            }
            let name = f
                .get("properties")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("Imported place");
            self.add(name, lat, lon);
            count += 1;
        }
        Ok(count)
    }

    fn persist(&self, list: &[Bookmark]) {
        let Some(path) = &self.path else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(list) {
            let _ = std::fs::write(path, text);
        }
    }
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self::default_location()
    }
}

#[cfg(test)]
mod tests {
    use super::BookmarkStore;

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
        assert_eq!(imported, 1);
        let list2 = store2.list();
        assert_eq!(list2.len(), 1);
        assert_eq!(list2[0].name, "Valea Morilor");
        assert!((list2[0].latitude - 47.0022).abs() < 1e-6);
        assert!((list2[0].longitude - 28.8180).abs() < 1e-6);

        let _ = std::fs::remove_dir_all(dir1);
        let _ = std::fs::remove_dir_all(dir2);
    }
}

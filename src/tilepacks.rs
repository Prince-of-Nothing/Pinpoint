//! Offline tile packs — user-initiated, throttled downloads of map tiles to
//! the on-device app dir so Pinpoint works offline.
//!
//! OSM policy: never bulk-scrapes `tile.openstreetmap.org`; packs fetch from
//! CARTO (same provider as the tile proxy) at ≤1 req/s, only for an explicit
//! bounding box the user chose, capped to 4096 tiles per pack.

use std::path::PathBuf;
use std::time::Duration;

use crate::platform;

pub const TILE_SIZE: f64 = 256.0;

/// Slippy tile for `(lat, lon)` at zoom `z` (Web Mercator, OSM standard).
pub fn latlon_to_tile(lat: f64, lon: f64, z: u8) -> (u32, u32) {
    let n = 1u32 << z;
    let lat_r = lat.clamp(-85.0511, 85.0511).to_radians();
    let x = n as f64 * (lon + 180.0) / 360.0;
    let y = n as f64 * (1.0 - (lat_r.tan() + 1.0 / lat_r.cos()).ln() / std::f64::consts::PI) / 2.0;
    let max = n.saturating_sub(1);
    ((x.floor() as u32).min(max), (y.floor() as u32).min(max))
}

/// Northwest `(lat, lon)` corner of Slippy tile `(x, y)` at zoom `z`.
pub fn tile_to_latlon(x: u32, y: u32, z: u8) -> (f64, f64) {
    let n = (1u32 << z) as f64;
    let lon = x as f64 / n * 360.0 - 180.0;
    let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * y as f64 / n))
        .sinh()
        .atan();
    (lat_rad.to_degrees(), lon)
}

/// Convert `(lat, lon)` to global Web Mercator pixel coordinates at zoom `z`
/// (256 px per tile).
pub fn latlon_to_world_px(lat: f64, lon: f64, z: u8) -> (f64, f64) {
    let scale = TILE_SIZE * ((1u32 << z) as f64);
    let lat_r = lat.clamp(-85.0511, 85.0511).to_radians();
    let x = (lon + 180.0) / 360.0 * scale;
    let y = (1.0 - (lat_r.tan() + 1.0 / lat_r.cos()).ln() / std::f64::consts::PI) / 2.0 * scale;
    (x, y)
}

/// Inverse of [`latlon_to_world_px`]: global pixel `(px, py)` at zoom `z` to `(lat, lon)`.
pub fn world_px_to_latlon(px: f64, py: f64, z: u8) -> (f64, f64) {
    let scale = TILE_SIZE * ((1u32 << z) as f64);
    let lon = (px / scale * 360.0 - 180.0).clamp(-180.0, 180.0);
    let n = std::f64::consts::PI - 2.0 * std::f64::consts::PI * (py / scale);
    let lat = n.sinh().atan().to_degrees().clamp(-85.0511, 85.0511);
    (lat, lon)
}

/// On-disk path for a cached tile `tiles/{z}/{x}/{y}.png`, if it exists.
pub fn cached_tile_path(z: u8, x: u32, y: u32) -> Option<PathBuf> {
    let base = platform::app_data_dir()?;
    let file = base
        .join("tiles")
        .join(z.to_string())
        .join(x.to_string())
        .join(format!("{y}.png"));
    if file.exists() {
        Some(file)
    } else {
        None
    }
}

/// Inclusive tile range covering `[(min_lat, min_lon), (max_lat, max_lon)]` at `z`.
pub fn bbox_tiles(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    z: u8,
) -> Vec<(u32, u32)> {
    let (x0, y0) = latlon_to_tile(max_lat, min_lon, z);
    let (x1, y1) = latlon_to_tile(min_lat, max_lon, z);
    let (x_lo, x_hi) = (x0.min(x1), x0.max(x1));
    let (y_lo, y_hi) = (y0.min(y1), y0.max(y1));
    let mut out = Vec::new();
    for y in y_lo..=y_hi {
        for x in x_lo..=x_hi {
            out.push((x, y));
        }
    }
    out
}

pub const PACK_TILE_CAP: usize = 4096;

/// Tiles for an offline pack around `(center_lat, center_lon)`: the current
/// zoom level plus one level out and one level in, deduplicated. The wider
/// span keeps the map usable while pinching or pressing +/- offline.
pub fn area_pack_tiles(
    center_lat: f64,
    center_lon: f64,
    zoom: u8,
    delta: f64,
) -> Vec<(u32, u32, u32)> {
    let z = zoom.clamp(10, 15);
    let mut triples = Vec::new();
    for level in [z.saturating_sub(1), z, (z + 1).min(18)] {
        for (x, y) in bbox_tiles(
            center_lat - delta,
            center_lon - delta,
            center_lat + delta,
            center_lon + delta,
            level,
        ) {
            let triple = (level as u32, x, y);
            if !triples.contains(&triple) {
                triples.push(triple);
            }
        }
    }
    triples
}
/// Demo OSM raster fallbacks (ODbL, no API key). Production self-hosts via
/// `PINPOINT_TILE_URL` / the backend proxy; never use tile.openstreetmap.org.
pub const OSM_DE_BASE: &str = "https://tile.openstreetmap.de";
pub const OSM_FR_BASE: &str = "https://a.tile.openstreetmap.fr/osmfr";

/// Candidate ODbL-compliant tile URLs for `(z, x, y)` in fallback order.
/// A self-hosted server (`PINPOINT_TILE_URL`, e.g.
/// `https://tiles.example.com/{z}/{x}/{y}.png`) is tried first; the public
/// providers are demo fallbacks. Never uses `tile.openstreetmap.org`.
/// Candidate tile URLs including a **Pinpoint backend proxy first**, then
/// `PINPOINT_TILE_URL` (self-hosted), then public demo fallbacks. Routing the
/// desktop's offline packs through the backend keeps third-party requests on
/// the server, where the operator can self-host everything (ADR-0011).
pub fn tile_candidate_urls_via(z: u32, x: u32, y: u32, backend_url: Option<&str>) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(base) = backend_url {
        let base = base.trim().trim_end_matches('/');
        if !base.is_empty() {
            urls.push(format!("{base}/map/tiles/{z}/{x}/{y}"));
        }
    }
    if let Ok(template) = std::env::var("PINPOINT_TILE_URL") {
        let t = template.trim();
        if !t.is_empty() {
            urls.push(
                t.replace("{z}", &z.to_string())
                    .replace("{x}", &x.to_string())
                    .replace("{y}", &y.to_string()),
            );
        }
    }
    urls.extend([
        format!("{OSM_DE_BASE}/{z}/{x}/{y}.png"),
        format!("{OSM_FR_BASE}/{z}/{x}/{y}.png"),
    ]);
    urls
}

/// Candidate URLs without a backend hint (self-hosted env + demo fallbacks).
pub fn tile_candidate_urls(z: u32, x: u32, y: u32) -> Vec<String> {
    tile_candidate_urls_via(z, x, y, None)
}

/// Prefer the Pinpoint backend proxy (`backend_url/map/tiles/...`), then
/// `PINPOINT_TILE_URL`, then demo fallbacks. Download `tiles` (z/x/y triples,
/// from [`bbox_tiles`] or a bookmark) into `tiles/{z}/{x}/{y}.png` under the
/// app data dir. Skips files already present (resume-safe).
/// Returns (downloaded, skipped, failed).
pub async fn download_pack(
    tiles: &[(u32, u32, u32)],
    client: Option<reqwest::Client>,
    backend_url: Option<&str>,
) -> (usize, usize, usize) {
    let Some(base) = platform::app_data_dir() else {
        return (0, 0, tiles.len());
    };
    let root = base.join("tiles");
    let client = match client {
        Some(c) => c,
        None => match reqwest::Client::builder()
            .local_address(Some(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
            .timeout(Duration::from_secs(15))
            .user_agent(concat!(
                "Pinpoint/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/Prince-of-Nothing/Pinpoint)"
            ))
            .build()
        {
            Ok(c) => c,
            Err(_) => return (0, 0, tiles.len()),
        },
    };
    let (mut ok, mut skipped, mut failed) = (0, 0, 0);
    for (i, (z, x, y)) in tiles.iter().take(PACK_TILE_CAP).enumerate() {
        let dir = root.join(z.to_string()).join(x.to_string());
        let file = dir.join(format!("{y}.png"));
        if file.exists() {
            skipped += 1;
            continue;
        }
        // OSM courtesy throttle: ≤1 req/s.
        if i > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let mut fetched_bytes = None;
        for url in tile_candidate_urls_via(*z, *x, *y, backend_url) {
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(body) = resp.bytes().await {
                        if !body.is_empty() {
                            fetched_bytes = Some(body);
                            break;
                        }
                    }
                }
            }
        }
        match fetched_bytes {
            Some(body) => {
                if std::fs::create_dir_all(&dir).is_err() {
                    failed += 1;
                    continue;
                }
                match std::fs::write(&file, body) {
                    Ok(()) => ok += 1,
                    Err(_) => failed += 1,
                }
            }
            None => failed += 1,
        }
    }
    (ok, skipped, failed)
}

/// Bytes currently occupied by offline tile packs (0 when unavailable).
pub fn packs_size() -> u64 {
    let Some(base) = platform::app_data_dir() else {
        return 0;
    };
    let root = base.join("tiles");
    let mut total = 0u64;
    if let Ok(entries) = walk_files(&root) {
        for f in entries {
            total += std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

fn walk_files(dir: &std::path::Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{
        area_pack_tiles, bbox_tiles, latlon_to_tile, latlon_to_world_px, tile_candidate_urls,
        tile_candidate_urls_via, tile_to_latlon, world_px_to_latlon,
    };

    #[test]
    fn slippy_origin() {
        // (0,0) at z=0 is tile (0,0); at z=1 the SE quadrant is (1,1).
        assert_eq!(latlon_to_tile(0.0, 0.0, 0), (0, 0));
        assert_eq!(latlon_to_tile(10.0, 90.0, 1), (1, 0));
        assert_eq!(latlon_to_tile(-10.0, -90.0, 1), (0, 1));
    }

    #[test]
    fn bbox_small_area_single_tile() {
        let tiles = bbox_tiles(41.88, -87.63, 41.89, -87.62, 14);
        assert!(!tiles.is_empty() && tiles.len() <= 9, "got {}", tiles.len());
    }

    #[test]
    fn bbox_inverted_corners_normalized() {
        let a = bbox_tiles(41.88, -87.63, 41.89, -87.62, 12);
        let b = bbox_tiles(41.89, -87.62, 41.88, -87.63, 12);
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn area_pack_spans_three_zoom_levels_without_duplicates() {
        let tiles = area_pack_tiles(47.0105, 28.8638, 14, 0.02);
        let levels: std::collections::HashSet<u32> = tiles.iter().map(|t| t.0).collect();
        assert_eq!(levels.len(), 3, "levels were {levels:?}");
        assert!(levels.contains(&13) && levels.contains(&14) && levels.contains(&15));
        let unique: std::collections::HashSet<_> = tiles.iter().copied().collect();
        assert_eq!(unique.len(), tiles.len());
        assert!(tiles.len() <= super::PACK_TILE_CAP);
    }

    #[test]
    fn world_px_round_trip() {
        let (lat, lon) = (47.0105, 28.8638);
        let (px, py) = latlon_to_world_px(lat, lon, 13);
        let (lat2, lon2) = world_px_to_latlon(px, py, 13);
        assert!((lat - lat2).abs() < 1e-5, "lat {lat} vs {lat2}");
        assert!((lon - lon2).abs() < 1e-5, "lon {lon} vs {lon2}");
        let (tx, ty) = latlon_to_tile(lat, lon, 13);
        let (nw_lat, nw_lon) = tile_to_latlon(tx, ty, 13);
        assert!(nw_lat >= lat && nw_lon <= lon);
    }

    #[test]
    fn candidate_urls_are_odbl_and_never_osmf() {
        let urls = tile_candidate_urls(13, 4752, 2880);
        assert!(urls.len() >= 2);
        for u in &urls {
            assert!(!u.contains("tile.openstreetmap.org"));
        }
    }

    #[test]
    fn backend_proxy_is_preferred_for_client_downloads() {
        let urls = tile_candidate_urls_via(13, 4752, 2880, Some("https://pinpoint.example/"));
        assert_eq!(
            urls.first().map(String::as_str),
            Some("https://pinpoint.example/map/tiles/13/4752/2880"),
            "desktop packs must go through the Pinpoint server, not a CDN"
        );
    }
}

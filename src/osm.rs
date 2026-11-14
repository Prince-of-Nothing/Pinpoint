//! OpenStreetMap services in pure Rust — the multiplatform replacement for the
//! archived web (`js/geocode.js` / `js/routing.js` / `js/places.js`) and
//! Kotlin (`OsmServices.kt`) clients. Same URLs, same policies, same privacy
//! envelope — now one implementation for every platform:
//!
//! * Nominatim search / reverse (https://nominatim.openstreetmap.org)
//!   — max 1 req/s, user-typed queries only.
//! * OSRM routing driving/foot/bike (https://router.project-osrm.org)
//!   — demo only; production self-hosts OSRM over OSM extracts.
//! * Overpass POI browse (https://overpass-api.de) — one query per tap.
//!
//! Data is © OpenStreetMap contributors (ODbL), reference:
//! https://wiki.openstreetmap.org/wiki/Main_Page
//! Friend/shared locations are NEVER sent to any of these services.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Public demo endpoints (defaults only). Production must point the
/// `PINPOINT_*_URL` env vars at **self-hosted** services — see
/// `docs/maps/self-hosted-osm.md` and ADR-0011.
pub const PUBLIC_NOMINATIM: &str = "https://nominatim.openstreetmap.org";
pub const PUBLIC_OSRM: &str = "https://router.project-osrm.org";
pub const PUBLIC_OVERPASS: &str = "https://overpass-api.de/api/interpreter";

fn env_url(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Active Nominatim base: `PINPOINT_NOMINATIM_URL` or the public demo.
pub fn nominatim_base() -> String {
    env_url("PINPOINT_NOMINATIM_URL", PUBLIC_NOMINATIM)
}

/// Active OSRM base: `PINPOINT_OSRM_URL` or the public demo.
pub fn osrm_base() -> String {
    env_url("PINPOINT_OSRM_URL", PUBLIC_OSRM)
}

/// Mode-specific OSRM base. Driving always falls back to [`osrm_base`];
/// foot/bike require their own self-hosted instance because the public demo
/// only serves a car profile (a `/foot` request silently returns car geometry).
pub fn osrm_base_for(mode: TravelMode) -> Option<String> {
    match mode {
        TravelMode::Driving => Some(osrm_base()),
        TravelMode::Foot => env_optional("PINPOINT_OSRM_FOOT_URL"),
        TravelMode::Bike => env_optional("PINPOINT_OSRM_BIKE_URL"),
    }
}

fn env_optional(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
}

/// OSRM exclusions (`avoid=motorway,toll,ferry`) need a profile that declares
/// those classes. The public demo does not, so the capability is opt-in for
/// self-hosted instances (`PINPOINT_OSRM_URL` or `PINPOINT_OSRM_AVOID=1`).
pub fn osrm_supports_avoid() -> bool {
    env_optional("PINPOINT_OSRM_URL").is_some()
        || std::env::var("PINPOINT_OSRM_AVOID")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
}

/// Active Overpass interpreter URL: `PINPOINT_OVERPASS_URL` or the public demo.
pub fn overpass_url() -> String {
    env_url("PINPOINT_OVERPASS_URL", PUBLIC_OVERPASS)
}

const USER_AGENT: &str = "PinpointRust/0.1.0 (privacy-first OSM client)";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchPlace {
    pub display_name: String,
    pub short_name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub category: String,
    pub place_type: String,
    /// Backend-computed distance from the search map centre, when supplied.
    #[serde(default)]
    pub distance_m: Option<f64>,
}

/// Server-advertised map capabilities. The client uses these to hide
/// walk/bike modes or avoid options the server cannot serve.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MapCapabilities {
    #[serde(default)]
    pub autocomplete: bool,
    #[serde(default)]
    pub modes: Vec<String>,
    #[serde(default)]
    pub avoid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelMode {
    Driving,
    Foot,
    Bike,
}

impl TravelMode {
    pub fn profile(self) -> &'static str {
        match self {
            Self::Driving => "driving",
            Self::Foot => "foot",
            Self::Bike => "bike",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteStep {
    pub text: String,
    pub distance_m: Option<f64>,
    pub duration_s: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteLeg {
    pub distance_m: f64,
    pub duration_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    pub points: Vec<(f64, f64)>,
    pub distance_m: f64,
    pub duration_s: f64,
    pub steps: Vec<RouteStep>,
    pub summary: String,
    pub legs: Vec<RouteLeg>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Poi {
    pub osm_id: i64,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub category: String,
    pub icon: String,
    pub phone: String,
    pub website: String,
    pub opening_hours: String,
    pub cuisine: String,
}

// Nominatim DTOs (only the fields we use; unknown keys ignored).
#[derive(Debug, Deserialize)]
struct NominatimItem {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    lat: String,
    #[serde(default)]
    lon: String,
    #[serde(rename = "class", default)]
    class_: String,
    #[serde(rename = "type", default)]
    type_: String,
}

impl NominatimItem {
    fn into_place(self) -> SearchPlace {
        let short = self
            .display_name
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        SearchPlace {
            display_name: self.display_name,
            short_name: if short.is_empty() {
                "Unnamed place".to_string()
            } else {
                short
            },
            latitude: self.lat.parse().unwrap_or(0.0),
            longitude: self.lon.parse().unwrap_or(0.0),
            category: self.class_,
            place_type: self.type_,
            distance_m: None,
        }
    }
}

// OSRM DTOs.
#[derive(Debug, Deserialize)]
struct OsrmResponse {
    #[serde(default)]
    code: String,
    #[serde(default)]
    routes: Vec<OsrmRoute>,
}

#[derive(Debug, Deserialize)]
struct OsrmRoute {
    #[serde(default)]
    distance: f64,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    geometry: String,
    #[serde(default)]
    legs: Vec<OsrmLeg>,
}

#[derive(Debug, Deserialize)]
struct OsrmLeg {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    distance: f64,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    steps: Vec<OsrmStep>,
}

#[derive(Debug, Deserialize)]
struct OsrmStep {
    #[serde(default)]
    name: String,
    #[serde(default)]
    distance: Option<f64>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    maneuver: Option<OsrmManeuver>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Deserialized from OSRM API; modifier/exit are part of the contract but not read yet.
struct OsrmManeuver {
    #[serde(rename = "type", default)]
    type_: String,
    #[serde(default)]
    modifier: Option<String>,
    #[serde(default)]
    exit: Option<i64>,
}

// Overpass DTOs.
#[derive(Debug, Deserialize)]
struct OverpassResponse {
    #[serde(default)]
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Thin OSM client. `reqwest::Client` is already in `Cargo.toml`, so this
/// adds zero dependencies while matching the archived web/Kotlin behaviour.
#[cfg(not(target_arch = "wasm32"))]
pub struct OsmClient {
    http: reqwest::Client,
    last_search: std::sync::Mutex<Option<Instant>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl OsmClient {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent(USER_AGENT)
                .build()?,
            last_search: std::sync::Mutex::new(None),
        })
    }

    async fn throttle(&self) {
        let wait = {
            let guard = self.last_search.lock().unwrap();
            match *guard {
                Some(last) => {
                    let elapsed = last.elapsed();
                    if elapsed < Duration::from_secs(1) {
                        Some(Duration::from_secs(1) - elapsed)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };
        if let Some(d) = wait {
            tokio::time::sleep(d).await;
        }
        *self.last_search.lock().unwrap() = Some(Instant::now());
    }

    /// Nominatim search (≥3 chars, debounced by the UI).
    pub async fn search(&self, query: &str) -> anyhow::Result<Vec<SearchPlace>> {
        let q = query.trim();
        if q.chars().count() < 3 {
            anyhow::bail!("Type at least 3 characters to search.");
        }
        // Moldova-first bias: append country hint unless the user already
        // named a place, so "Cemetary of Mihai" resolves to the Moldovan
        // result instead of a homonym elsewhere. Diacritics are kept (Nominatim
        // handles them) but we also accept ASCII fallbacks ("Chisinau").
        let lower = q.to_lowercase();
        let biased = if [
            "moldova",
            "chisinau",
            "chișinău",
            "balti",
            "bălți",
            "tiraspol",
            "orhei",
        ]
        .iter()
        .any(|k| lower.contains(k))
        {
            q.to_string()
        } else {
            format!("{q}, Moldova")
        };
        self.throttle().await;
        let url = format!(
            "{}/search?q={}&format=jsonv2&limit=8&addressdetails=1&bounded=0&viewbox=26.5,48.5,30.0,45.5&accept-language=ro,ru,en",
            nominatim_base(),
            urlencoding(&biased)
        );
        let items: Vec<NominatimItem> = self.http.get(&url).send().await?.json().await?;
        Ok(items
            .into_iter()
            .map(|i| i.into_place())
            .filter(|p| p.latitude != 0.0 || p.longitude != 0.0)
            .collect())
    }

    /// Reverse geocode (tap location).
    pub async fn reverse(&self, latitude: f64, longitude: f64) -> anyhow::Result<SearchPlace> {
        self.throttle().await;
        let url = format!(
            "{}/reverse?lat={latitude}&lon={longitude}&format=jsonv2&zoom=16",
            nominatim_base()
        );
        let item: NominatimItem = self.http.get(&url).send().await?.json().await?;
        Ok(item.into_place())
    }

    /// OSRM route (driving/foot/bike) using the single demo base.
    /// Desktop parity: public OSRM ignores the profile, so this remains the
    /// legacy path; the web proxy uses `route_alternatives` with per-mode URLs.
    pub async fn route(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        mode: TravelMode,
    ) -> anyhow::Result<Route> {
        self.route_alternatives_at(&osrm_base(), from, to, mode, &[], &[])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No route found between these points."))
    }

    /// Web routing with up to three alternatives, using the mode-specific
    /// self-hosted URL when configured (see [`osrm_base_for`]).
    pub async fn route_alternatives(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        mode: TravelMode,
    ) -> anyhow::Result<Vec<Route>> {
        self.route_alternatives_via(from, to, mode, &[], &[]).await
    }

    /// Routing with intermediate stops (`via`) and OSRM exclusions
    /// (`motorway`, `toll`, `ferry`). Alternatives stay enabled.
    pub async fn route_alternatives_via(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        mode: TravelMode,
        via: &[(f64, f64)],
        avoid: &[String],
    ) -> anyhow::Result<Vec<Route>> {
        let base = osrm_base_for(mode).ok_or_else(|| {
            anyhow::anyhow!(
                "The {} routing profile is not configured. Set {}.",
                mode.profile(),
                match mode {
                    TravelMode::Driving => "PINPOINT_OSRM_URL",
                    TravelMode::Foot => "PINPOINT_OSRM_FOOT_URL",
                    TravelMode::Bike => "PINPOINT_OSRM_BIKE_URL",
                }
            )
        })?;
        self.route_alternatives_at(&base, from, to, mode, via, avoid)
            .await
    }

    async fn route_alternatives_at(
        &self,
        base: &str,
        from: (f64, f64),
        to: (f64, f64),
        mode: TravelMode,
        via: &[(f64, f64)],
        avoid: &[String],
    ) -> anyhow::Result<Vec<Route>> {
        // Coords are lon,lat;lon,lat (OSRM convention), with optional stops.
        let mut coords = format!("{:.6},{:.6}", from.1, from.0);
        for (lat, lon) in via {
            coords.push_str(&format!(";{lon:.6},{lat:.6}"));
        }
        coords.push_str(&format!(";{:.6},{:.6}", to.1, to.0));
        let exclude = if avoid.is_empty() {
            String::new()
        } else {
            format!("&exclude={}", avoid.join(","))
        };
        let url = format!(
            "{base}/route/v1/{}/{coords}?overview=full&geometries=polyline&steps=true&alternatives=true{exclude}",
            mode.profile(),
        );
        let resp: OsrmResponse = self.http.get(&url).send().await?.json().await?;
        if resp.code != "Ok" || resp.routes.is_empty() {
            anyhow::bail!("No route found between these points.");
        }
        Ok(resp.routes.into_iter().map(route_from_osrm).collect())
    }

    /// Overpass POI around a centre (one query per call).
    pub async fn pois(
        &self,
        latitude: f64,
        longitude: f64,
        category: &str,
    ) -> anyhow::Result<Vec<Poi>> {
        let fragment =
            poi_fragment(category).ok_or_else(|| anyhow::anyhow!("Unknown POI category"))?;
        let (icon, _) = poi_icon(category);
        let query = format!(
            "[out:json][timeout:25];({});out center 60;",
            fragment
                .replace("{r}", "1500")
                .replace("{lat}", &format!("{latitude:.6}"))
                .replace("{lon}", &format!("{longitude:.6}"))
        );
        let resp: OverpassResponse = self
            .http
            .post(overpass_url())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("data={}", urlencoding(&query)))
            .send()
            .await?
            .json()
            .await?;
        Ok(resp
            .elements
            .into_iter()
            .filter_map(|e| match (e.lat, e.lon) {
                (Some(lat), Some(lon)) => {
                    let name = e
                        .tags
                        .get("name")
                        .or_else(|| e.tags.get("brand"))
                        .or_else(|| e.tags.get("operator"))
                        .cloned()
                        .unwrap_or_else(|| "(unnamed)".to_string());
                    Some(Poi {
                        osm_id: e.id,
                        name,
                        latitude: lat,
                        longitude: lon,
                        category: category.to_string(),
                        icon: icon.to_string(),
                        phone: e.tags.get("phone").cloned().unwrap_or_default(),
                        website: e.tags.get("website").cloned().unwrap_or_default(),
                        opening_hours: e.tags.get("opening_hours").cloned().unwrap_or_default(),
                        cuisine: e.tags.get("cuisine").cloned().unwrap_or_default(),
                    })
                }
                _ => None,
            })
            .collect())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for OsmClient {
    fn default() -> Self {
        Self::new().expect("OsmClient default requires a reqwest client")
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn route_from_osrm(r: OsrmRoute) -> Route {
    let points = decode_polyline(&r.geometry);
    let summary = r
        .legs
        .first()
        .map(|l| l.summary.clone())
        .unwrap_or_default();
    let legs: Vec<RouteLeg> = r
        .legs
        .iter()
        .map(|l| RouteLeg {
            distance_m: l.distance,
            duration_s: l.duration,
        })
        .collect();
    let steps: Vec<RouteStep> = r
        .legs
        .into_iter()
        .flat_map(|leg| leg.steps)
        .map(|s| RouteStep {
            text: step_text(&s),
            distance_m: s.distance,
            duration_s: s.duration,
        })
        .collect();
    Route {
        points,
        distance_m: r.distance,
        duration_s: r.duration,
        steps,
        summary,
        legs,
    }
}

fn urlencoding(s: &str) -> String {
    // Minimal percent-encoding (unreserved + space as %20). Keeps us
    // dependency-free; production could pull `url`'s form_urlencoded.
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Google polyline decode (OSRM `overview=full&geometries=polyline`).
pub fn decode_polyline(encoded: &str) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    let mut idx = 0usize;
    let mut lat: i32 = 0;
    let mut lng: i32 = 0;
    let bytes = encoded.as_bytes();
    while idx < bytes.len() {
        let (delta, _) = decode_delta(bytes, &mut idx);
        lat += delta;
        let (delta2, _) = decode_delta(bytes, &mut idx);
        lng += delta2;
        points.push((lat as f64 / 1e5, lng as f64 / 1e5));
    }
    points
}

fn decode_delta(bytes: &[u8], idx: &mut usize) -> (i32, usize) {
    let mut shift = 0;
    let mut result: i32 = 0;
    let mut byte: u8;
    loop {
        byte = bytes[*idx] - 63;
        *idx += 1;
        result |= ((byte & 0x1f) as i32) << shift;
        shift += 5;
        if byte < 0x20 {
            break;
        }
    }
    let delta = if result & 1 != 0 {
        !(result >> 1)
    } else {
        result >> 1
    };
    (delta, shift)
}

fn step_text(step: &OsrmStep) -> String {
    let t = step
        .maneuver
        .as_ref()
        .map(|m| m.type_.as_str())
        .unwrap_or("continue");
    let verb = match t {
        "depart" => "Depart",
        "arrive" => "Arrive",
        "turn" => "Turn",
        "merge" => "Merge",
        "onramp" => "Take the ramp",
        "offramp" => "Exit",
        "fork" => "Keep",
        "roundabout" | "rotary" | "roundaboutturn" => "At the roundabout, take",
        _ => "Continue",
    };
    let name = if step.name.is_empty() {
        String::new()
    } else {
        format!(" onto {}", step.name)
    };
    format!("{verb}{name}")
}

/// Render a route as a GPX 1.1 track (with ODbL attribution embedded).
pub fn route_to_gpx(route: &Route, name: &str) -> String {
    let safe = xml_escape(name);
    let mut trkpts = String::new();
    for (lat, lon) in &route.points {
        trkpts.push_str(&format!(
            "      <trkpt lat=\"{lat:.6}\" lon=\"{lon:.6}\"></trkpt>\n"
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<gpx version=\"1.1\" creator=\"Pinpoint\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n\
  <metadata>\n\
    <name>{safe}</name>\n\
    <desc>Route from Pinpoint — map data © OpenStreetMap contributors (ODbL)</desc>\n\
  </metadata>\n\
  <trk>\n\
    <name>{safe}</name>\n\
    <trkseg>\n\
{trkpts}    </trkseg>\n\
  </trk>\n\
</gpx>\n"
    )
}

/// Write a route GPX file under `<app_data_dir>/exports/` and return its path.
pub fn export_route_gpx(route: &Route, name: &str) -> anyhow::Result<std::path::PathBuf> {
    let base = crate::platform::app_data_dir()
        .ok_or_else(|| anyhow::anyhow!("no writable app data dir on this platform"))?;
    let dir = base.join("exports");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("route_{stamp}.gpx"));
    std::fs::write(&path, route_to_gpx(route, name))?;
    Ok(path)
}

/// KML LineString for a route. Coordinates are `lon,lat,0` per KML.
pub fn route_to_kml(route: &Route, name: &str) -> String {
    let safe = xml_escape(name);
    let coords = route
        .points
        .iter()
        .map(|(lat, lon)| format!("{lon:.6},{lat:.6},0"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <kml xmlns=\"http://www.opengis.net/kml/2.2\">\n\
         <Document><name>{safe}</name><Placemark><name>{safe}</name><LineString>\
         <tessellate>1</tessellate><coordinates>{coords}</coordinates>\
         </LineString></Placemark></Document></kml>\n"
    )
}

/// GeoJSON FeatureCollection with one LineString feature for a route.
pub fn route_to_geojson(route: &Route, name: &str) -> String {
    let coordinates: Vec<[f64; 2]> = route.points.iter().map(|(lat, lon)| [*lon, *lat]).collect();
    serde_json::json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "properties": {
                "name": name,
                "distance_m": route.distance_m,
                "duration_s": route.duration_s,
            },
            "geometry": {
                "type": "LineString",
                "coordinates": coordinates,
            }
        }]
    })
    .to_string()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn poi_fragment(id: &str) -> Option<&'static str> {
    match id {
        "food" => Some(
            r#"node["amenity"="restaurant"](around:{r},{lat},{lon});node["amenity"="cafe"](around:{r},{lat},{lon});"#,
        ),
        "fuel" => Some(r#"node["amenity"="fuel"](around:{r},{lat},{lon});"#),
        "pharmacy" => Some(r#"node["amenity"="pharmacy"](around:{r},{lat},{lon});"#),
        "hospital" => Some(r#"node["amenity"="hospital"](around:{r},{lat},{lon});"#),
        "hotel" => Some(
            r#"node["tourism"="hotel"](around:{r},{lat},{lon});node["tourism"="hostel"](around:{r},{lat},{lon});"#,
        ),
        "tourism" => Some(
            r#"node["tourism"="museum"](around:{r},{lat},{lon});node["tourism"="attraction"](around:{r},{lat},{lon});"#,
        ),
        "water" => Some(
            r#"node["amenity"="drinking_water"](around:{r},{lat},{lon});node["amenity"="toilets"](around:{r},{lat},{lon});"#,
        ),
        "parking" => Some(r#"node["amenity"="parking"](around:{r},{lat},{lon});"#),
        "park" => Some(r#"node["leisure"="park"](around:{r},{lat},{lon});"#),
        _ => None,
    }
}

pub fn poi_icon(id: &str) -> (&'static str, &'static str) {
    match id {
        "food" => ("🍽️", "Food"),
        "fuel" => ("⛽", "Fuel"),
        "pharmacy" => ("💊", "Pharmacy"),
        "hospital" => ("🏥", "Health"),
        "hotel" => ("🏨", "Stay"),
        "tourism" => ("🏛️", "Sights"),
        "water" => ("💧", "Water"),
        "parking" => ("🅿️", "Parking"),
        "park" => ("🌳", "Parks"),
        _ => ("•", "Place"),
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_polyline, poi_fragment, urlencoding};

    #[test]
    fn polyline_decodes_known_vector() {
        // "_p~iF~ps|U_ulLnnqC_mqNvxq@" → (38.5, -120.2), (40.7, -120.95), (43.252, -126.453)
        let pts = decode_polyline("_p~iF~ps|U_ulLnnqC_mqNvxq@");
        assert_eq!(pts.len(), 3);
        assert!((pts[0].0 - 38.5).abs() < 1e-4);
        assert!((pts[0].1 + 120.2).abs() < 1e-4);
    }

    #[test]
    fn empty_polyline_is_empty() {
        assert!(decode_polyline("").is_empty());
    }

    #[test]
    fn poi_fragments_all_known() {
        for id in [
            "food", "fuel", "pharmacy", "hospital", "hotel", "tourism", "water", "parking", "park",
        ] {
            assert!(poi_fragment(id).is_some(), "{id} must have a fragment");
        }
        assert!(poi_fragment("nope").is_none());
    }

    #[test]
    fn urlencoding_handles_spaces() {
        assert_eq!(urlencoding("Chisinau cafe"), "Chisinau%20cafe");
    }

    #[test]
    fn route_gpx_has_trackpoints_and_escaped_name() {
        use super::{route_to_gpx, Route};
        let route = Route {
            points: vec![(47.0105, 28.8638), (47.0205, 28.8738)],
            distance_m: 1200.0,
            duration_s: 900.0,
            steps: vec![],
            summary: "Main".into(),
            legs: vec![],
        };
        let gpx = route_to_gpx(&route, "Cafe & <Bar>");
        assert!(gpx.contains("<gpx version=\"1.1\""));
        assert!(gpx.contains("<trkpt lat=\"47.010500\" lon=\"28.863800\">"));
        assert!(gpx.contains("Cafe &amp; &lt;Bar&gt;"));
        assert!(gpx.contains("OpenStreetMap contributors"));
    }

    #[test]
    fn route_kml_and_geojson_have_lon_lat_coordinates() {
        use super::{route_to_geojson, route_to_kml, Route};
        let route = Route {
            points: vec![(47.0105, 28.8638), (47.0205, 28.8738)],
            distance_m: 1200.0,
            duration_s: 900.0,
            steps: vec![],
            summary: "Main".into(),
            legs: vec![],
        };
        let kml = route_to_kml(&route, "Cafe & <Bar>");
        assert!(kml.contains("28.863800,47.010500,0"));
        assert!(kml.contains("Cafe &amp; &lt;Bar&gt;"));
        let geojson = route_to_geojson(&route, "Cafe");
        assert!(geojson.contains("\"type\":\"LineString\""));
        assert!(geojson.contains("28.8638"));
    }
}

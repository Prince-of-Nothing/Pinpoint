//! Region catalog for offline packs (schema 1).
//!
//! The catalog lists downloadable regions and the exact release assets that
//! make up each pack. Production fetches it from [`DEFAULT_CATALOG_URL`]
//! (override with [`CATALOG_URL_ENV`]) and caches the parsed result for
//! [`CATALOG_CACHE_TTL`]. Any fetch or validation failure falls back to the
//! catalog bundled at compile time from `web/catalog.json`, so the map keeps
//! working offline and in tests.
//!
//! Validation is intentionally strict: every id, name, URL, size and hash in
//! a catalog is used to build a local file path or to fetch a remote
//! artifact, so a malformed catalog must never reach the installer.
//!
//! # Host policy (see `docs/offline/catalog-security.md`)
//!
//! Remote catalog-supplied URLs are untrusted. Only these hosts are accepted,
//! always over HTTPS with no userinfo, fragment or non-443 port:
//!
//! * catalog document: [`CATALOG_HOSTS`] (`raw.githubusercontent.com`);
//! * release assets: [`ASSET_HOSTS`] (`github.com` and the two GitHub release
//!   redirect hosts it uses, `release-assets.githubusercontent.com` and
//!   `objects.githubusercontent.com`).
//!
//! Asset redirects are capped at [`MAX_REDIRECTS`] and every hop must stay on
//! an [`ASSET_HOSTS`] host over HTTPS. Loopback HTTP is accepted only for
//! explicitly configured sources (the `PINPOINT_OFFLINE_BASE_URL` test/mirror
//! override, `DownloadPlan`, or an injected fixed catalog), never for a URL
//! that came from a remote catalog.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Only schema 1 is understood.
pub const CATALOG_SCHEMA: u32 = 1;

/// Remote catalog published by the `pinpoint-maps` data repository.
pub const DEFAULT_CATALOG_URL: &str =
    "https://raw.githubusercontent.com/Prince-of-Nothing/pinpoint-maps/main/catalog.json";

/// Environment override for the catalog URL (tests and mirrors).
pub const CATALOG_URL_ENV: &str = "PINPOINT_MAPS_CATALOG_URL";

/// Bundled fallback, replaced only by a new application release.
pub const BUNDLED_CATALOG: &str = include_str!("../web/catalog.json");

/// Hosts a catalog document may be fetched from.
pub const CATALOG_HOSTS: &[&str] = &["raw.githubusercontent.com"];

/// Hosts a release asset may be fetched or redirected to.
pub const ASSET_HOSTS: &[&str] = &[
    "github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];

/// Redirects followed for a catalog or asset request.
pub const MAX_REDIRECTS: usize = 3;

/// A catalog response larger than this is rejected without parsing.
pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;

/// GitHub Releases caps one asset at 2 GiB; a larger declaration is refused.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Upper bounds that keep a hostile catalog from exhausting memory or disk.
pub const MAX_REGIONS: usize = 64;
pub const MAX_FILES_PER_REGION: usize = 32;

/// A successful remote catalog is reused for one hour.
pub const CATALOG_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// A remote catalog request gives up after five seconds.
const CATALOG_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// One file of a region pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    Basemap,
    Graph,
}

/// How the published transfer is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileCompression {
    Identity,
    Gzip,
}

/// One downloadable artifact of a region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionFile {
    pub role: FileRole,
    /// Published asset name (for logging and fixtures).
    pub transfer_name: String,
    /// File name below `packs/<region>/`. Never contains a path separator.
    pub installed_name: String,
    /// Absolute `https` URL of the published transfer.
    pub url: String,
    pub transfer_bytes: u64,
    pub transfer_sha256: String,
    pub installed_bytes: u64,
    pub installed_sha256: String,
    pub compression: FileCompression,
}

/// One region (usually a country) in the catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub id: String,
    pub name: String,
    /// `[west, south, east, north]` in WGS84 degrees.
    pub bbox: [f64; 4],
    #[serde(default)]
    pub maxzoom: u8,
    pub version: String,
    pub files: Vec<RegionFile>,
}

/// The parsed catalog document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionCatalog {
    pub schema: u32,
    pub updated: String,
    pub regions: Vec<Region>,
}

impl RegionCatalog {
    /// Parse and validate a catalog document. Asset URLs must satisfy the
    /// remote host policy ([`valid_asset_url`]).
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(format!("catalog is larger than {MAX_CATALOG_BYTES} bytes"));
        }
        let catalog: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("catalog is not valid JSON: {error}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// The bundled fallback. It is committed with the app and covered by a
    /// test, so a parse failure is a build error, not a runtime condition.
    pub fn bundled() -> Self {
        Self::parse(BUNDLED_CATALOG.as_bytes()).expect("the bundled catalog is valid")
    }

    /// Structural validation of a parsed catalog.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CATALOG_SCHEMA {
            return Err(format!(
                "unsupported catalog schema {} (expected {CATALOG_SCHEMA})",
                self.schema
            ));
        }
        if !valid_timestamp(&self.updated) {
            return Err(format!(
                "catalog updated {:?} is not an RFC 3339 timestamp",
                self.updated
            ));
        }
        if self.regions.is_empty() {
            return Err("catalog contains no regions".to_string());
        }
        if self.regions.len() > MAX_REGIONS {
            return Err(format!(
                "catalog contains {} regions; at most {MAX_REGIONS} are allowed",
                self.regions.len()
            ));
        }

        let mut ids = HashSet::new();
        for region in &self.regions {
            if !valid_region_id(&region.id) {
                return Err(format!("invalid region id {:?}", region.id));
            }
            if !ids.insert(region.id.as_str()) {
                return Err(format!("duplicate region id {:?}", region.id));
            }
            if !valid_version(&region.version) {
                return Err(format!(
                    "region {:?} version {:?} is not a safe version string",
                    region.id, region.version
                ));
            }
            if region.maxzoom > 24 {
                return Err(format!(
                    "region {:?} maxzoom {} is above the 24 cap",
                    region.id, region.maxzoom
                ));
            }
            if !region.bbox.iter().all(|value| value.is_finite()) {
                return Err(format!("region {:?} has a non-finite bbox", region.id));
            }
            if region.files.is_empty() {
                return Err(format!("region {:?} contains no files", region.id));
            }
            if region.files.len() > MAX_FILES_PER_REGION {
                return Err(format!(
                    "region {:?} contains {} files; at most {MAX_FILES_PER_REGION} are allowed",
                    region.id,
                    region.files.len()
                ));
            }

            let mut names = HashSet::new();
            let mut roles = HashSet::new();
            for file in &region.files {
                if !valid_file_name(&file.installed_name) {
                    return Err(format!(
                        "region {:?} has an unsafe installed name {:?}",
                        region.id, file.installed_name
                    ));
                }
                if !valid_file_name(&file.transfer_name) {
                    return Err(format!(
                        "region {:?} has an unsafe transfer name {:?}",
                        region.id, file.transfer_name
                    ));
                }
                if !names.insert(file.installed_name.as_str()) {
                    return Err(format!(
                        "region {:?} installs {:?} twice",
                        region.id, file.installed_name
                    ));
                }
                if !roles.insert(file.role) {
                    return Err(format!(
                        "region {:?} has more than one {:?} file",
                        region.id, file.role
                    ));
                }
                if !valid_asset_url(&file.url) {
                    return Err(format!(
                        "region {:?} file {:?} has an invalid URL",
                        region.id, file.installed_name
                    ));
                }
                if file.transfer_bytes == 0 || file.installed_bytes == 0 {
                    return Err(format!(
                        "region {:?} file {:?} has a zero byte length",
                        region.id, file.installed_name
                    ));
                }
                if file.transfer_bytes > MAX_FILE_BYTES || file.installed_bytes > MAX_FILE_BYTES {
                    return Err(format!(
                        "region {:?} file {:?} is larger than the 2 GiB cap",
                        region.id, file.installed_name
                    ));
                }
                if !valid_sha256(&file.transfer_sha256) {
                    return Err(format!(
                        "region {:?} file {:?} has an invalid transfer sha256",
                        region.id, file.installed_name
                    ));
                }
                if !valid_sha256(&file.installed_sha256) {
                    return Err(format!(
                        "region {:?} file {:?} has an invalid installed sha256",
                        region.id, file.installed_name
                    ));
                }
            }

            let basemap = region
                .files
                .iter()
                .find(|file| file.role == FileRole::Basemap);
            let graph = region
                .files
                .iter()
                .find(|file| file.role == FileRole::Graph);
            let (Some(basemap), Some(graph)) = (basemap, graph) else {
                return Err(format!(
                    "region {:?} must publish one basemap and one graph file",
                    region.id
                ));
            };
            if basemap.compression != FileCompression::Identity
                || basemap.transfer_bytes != basemap.installed_bytes
                || basemap.transfer_sha256 != basemap.installed_sha256
            {
                return Err(format!(
                    "region {:?} basemap must be an identity transfer with equal bytes and hash",
                    region.id
                ));
            }
            if graph.compression != FileCompression::Gzip {
                return Err(format!("region {:?} graph must be gzip", region.id));
            }
            if graph.transfer_name == graph.installed_name {
                return Err(format!(
                    "region {:?} graph transfer and installed names must differ",
                    region.id
                ));
            }
        }
        Ok(())
    }

    /// Look up one region by id.
    pub fn region(&self, id: &str) -> Option<&Region> {
        self.regions.iter().find(|region| region.id == id)
    }
}

/// Region ids are lowercase ASCII, digits and hyphens, 1-64 characters.
pub fn valid_region_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Installed and transfer names are one lowercase path segment:
/// `^[a-z0-9][a-z0-9.-]{0,127}$` without `..`.
pub fn valid_file_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if !(1..=128).contains(&bytes.len()) || name.contains("..") {
        return false;
    }
    let Some(first) = bytes.first() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes.iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
    })
}

/// Strict `https` URL for a remote catalog document.
pub fn valid_catalog_url(value: &str) -> bool {
    valid_https_url(value, CATALOG_HOSTS)
}

/// Strict `https` URL for a catalog-supplied release asset.
pub fn valid_asset_url(value: &str) -> bool {
    valid_https_url(value, ASSET_HOSTS)
}

/// Explicitly configured test/mirror URL: a strict [`valid_asset_url`], or
/// plain HTTP on a loopback host. Never used for remote catalog entries.
pub fn valid_local_asset_url(value: &str) -> bool {
    valid_asset_url(value) || valid_loopback_url(value)
}

/// A catalog document source may be the release host or a loopback test host.
pub fn valid_catalog_source_url(value: &str) -> bool {
    valid_catalog_url(value) || valid_loopback_url(value)
}

/// A redirect hop for a release asset. HTTPS and [`ASSET_HOSTS`] only.
pub fn asset_redirect_allowed(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| ASSET_HOSTS.contains(&host))
        && url.port().is_none_or(|port| port == 443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

/// A redirect hop for a catalog document. HTTPS and [`CATALOG_HOSTS`] only.
pub fn catalog_redirect_allowed(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| CATALOG_HOSTS.contains(&host))
        && url.port().is_none_or(|port| port == 443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

fn valid_https_url(value: &str, hosts: &[&str]) -> bool {
    let Ok(parsed) = url::Url::parse(value) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.host_str().is_some_and(|host| hosts.contains(&host))
        && parsed.port().is_none_or(|port| port == 443)
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.fragment().is_none()
}

fn valid_loopback_url(value: &str) -> bool {
    let Ok(parsed) = url::Url::parse(value) else {
        return false;
    };
    matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str().is_some_and(is_loopback_host)
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.fragment().is_none()
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "[::1]" | "localhost")
}

fn valid_timestamp(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value).is_ok()
}

/// Versions are opaque (string equality drives updates): accept the compact
/// `YYYYMMDD` used by the release workflow and the `YYYY-MM-DD` used by the
/// bundled catalog, plus future safe identifiers.
fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.contains("..")
}

/// Both hashes are mandatory lowercase SHA-256 hex.
pub fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Catalog source and its one-hour cache.
#[derive(Debug)]
pub struct CatalogCache {
    source: CatalogSource,
    state: Mutex<CacheState>,
}

#[derive(Debug)]
enum CatalogSource {
    Fixed(Arc<RegionCatalog>),
    Remote { url: String },
}

#[derive(Debug)]
struct CacheState {
    catalog: Arc<RegionCatalog>,
    fetched_at: Option<Instant>,
    last_error: Option<String>,
    /// `updated` of the accepted catalog; a remote rollback is refused.
    updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl CacheState {
    fn new(catalog: Arc<RegionCatalog>) -> Self {
        Self {
            updated: catalog_updated(&catalog),
            catalog,
            fetched_at: None,
            last_error: None,
        }
    }
}

fn catalog_updated(catalog: &RegionCatalog) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(&catalog.updated)
        .ok()
        .map(|updated| updated.with_timezone(&chrono::Utc))
}

impl CatalogCache {
    /// A cache fixed to an explicit catalog; it never touches the network.
    pub fn fixed(catalog: RegionCatalog) -> Self {
        let catalog = Arc::new(catalog);
        Self {
            source: CatalogSource::Fixed(Arc::clone(&catalog)),
            state: Mutex::new(CacheState::new(catalog)),
        }
    }

    /// A cache fixed to the bundled fallback.
    pub fn bundled() -> Self {
        Self::fixed(RegionCatalog::bundled())
    }

    /// A remote cache that starts on the bundled catalog and refreshes at
    /// most once per [`CATALOG_CACHE_TTL`] (or after a failure).
    pub fn remote(url: impl Into<String>) -> Self {
        Self {
            source: CatalogSource::Remote { url: url.into() },
            state: Mutex::new(CacheState::new(Arc::new(RegionCatalog::bundled()))),
        }
    }

    /// Production cache: `PINPOINT_MAPS_CATALOG_URL` or the pinned default.
    pub fn remote_default() -> Self {
        let environment = std::env::var(CATALOG_URL_ENV).ok();
        Self::remote(resolve_catalog_url(environment.as_deref()))
    }

    /// Whether the catalog comes from the network. A remote catalog is
    /// untrusted, so its asset URLs are revalidated with the strict policy.
    pub fn is_remote(&self) -> bool {
        matches!(self.source, CatalogSource::Remote { .. })
    }

    /// The current catalog, refreshing a stale remote source first. A failed
    /// refresh keeps the last good catalog; the bundled fallback remains only
    /// until a remote catalog has ever been accepted.
    pub async fn get(&self) -> Arc<RegionCatalog> {
        let stale = {
            let state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            match &self.source {
                CatalogSource::Fixed(_) => false,
                CatalogSource::Remote { .. } => state
                    .fetched_at
                    .is_none_or(|fetched_at| fetched_at.elapsed() >= CATALOG_CACHE_TTL),
            }
        };
        if stale {
            let _ = self.refresh().await;
        }
        self.current()
    }

    /// The catalog last loaded without any I/O.
    pub fn current(&self) -> Arc<RegionCatalog> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .catalog
            .clone()
    }

    /// Force a fetch of the remote source (no-op for a fixed cache). The
    /// accepted catalog must be strictly newer than the current one, so a
    /// replayed or tampered `main` cannot roll a region back.
    pub async fn refresh(&self) -> Result<Arc<RegionCatalog>, String> {
        let url = match &self.source {
            CatalogSource::Fixed(catalog) => return Ok(Arc::clone(catalog)),
            CatalogSource::Remote { url } => url.clone(),
        };
        let result = fetch_catalog(&url).await;
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.fetched_at = Some(Instant::now());
        let catalog = match result {
            Ok(catalog) => catalog,
            Err(error) => {
                // Keep the last good catalog (bundled until a remote one was
                // accepted) instead of downgrading availability.
                state.last_error = Some(error.clone());
                return Err(error);
            }
        };
        let updated = match catalog_updated(&catalog) {
            Some(updated) => updated,
            None => {
                let error = format!("catalog updated {:?} is not valid", catalog.updated);
                state.last_error = Some(error.clone());
                return Err(error);
            }
        };
        if state.updated.is_some_and(|current| updated <= current) {
            let error = format!(
                "catalog updated {} is not newer than the accepted {}",
                catalog.updated, state.catalog.updated
            );
            state.last_error = Some(error.clone());
            return Err(error);
        }
        state.catalog = Arc::new(catalog);
        state.updated = Some(updated);
        state.last_error = None;
        Ok(Arc::clone(&state.catalog))
    }

    /// Last fetch error, if the most recent refresh failed.
    pub fn last_error(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .last_error
            .clone()
    }
}

fn resolve_catalog_url(environment: Option<&str>) -> String {
    match environment
        .map(str::trim)
        .filter(|url| valid_catalog_source_url(url))
    {
        Some(url) => url.to_string(),
        None => DEFAULT_CATALOG_URL.to_string(),
    }
}

fn catalog_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(CATALOG_FETCH_TIMEOUT)
        .connect_timeout(CATALOG_FETCH_TIMEOUT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many catalog redirects")
            } else if catalog_redirect_allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("catalog redirect left the allowlisted host")
            }
        }))
        .user_agent(concat!(
            "Pinpoint/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/Prince-of-Nothing/Pinpoint)"
        ))
        .build()
        .map_err(|error| format!("could not create the catalog client: {error}"))
}

async fn fetch_catalog(url: &str) -> Result<RegionCatalog, String> {
    if !valid_catalog_source_url(url) {
        return Err(format!("{url} is not an allowlisted catalog URL"));
    }
    let client = catalog_client()?;
    let mut response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| format!("could not fetch {url}: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "{url} returned HTTP {} instead of a catalog",
            response.status()
        ));
    }
    if let Some(length) = response.content_length() {
        if length > MAX_CATALOG_BYTES as u64 {
            return Err(format!(
                "{url} declared {length} bytes; the cap is {MAX_CATALOG_BYTES}"
            ));
        }
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("download of {url} failed: {error}"))?
    {
        if body.len().saturating_add(chunk.len()) > MAX_CATALOG_BYTES {
            return Err(format!("{url} sent more than {MAX_CATALOG_BYTES} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    RegionCatalog::parse(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::Router;

    type JsonMutation = (&'static str, Box<dyn Fn(&mut serde_json::Value)>);

    fn bundled_value() -> serde_json::Value {
        serde_json::to_value(RegionCatalog::bundled()).expect("bundled catalog JSON")
    }

    #[test]
    fn bundled_catalog_matches_the_moldova_release() {
        let catalog = RegionCatalog::bundled();
        assert_eq!(catalog.schema, CATALOG_SCHEMA);
        assert_eq!(catalog.updated, "2026-11-17T12:00:00Z");
        let moldova = catalog.region("moldova").expect("moldova region");
        assert_eq!(moldova.bbox, [26.6, 45.4, 30.2, 48.5]);
        assert_eq!(moldova.version, "2026-11-16");
        assert_eq!(moldova.files.len(), 2);
        let basemap = &moldova.files[0];
        assert_eq!(basemap.role, FileRole::Basemap);
        assert_eq!(basemap.installed_bytes, 93_172_399);
        assert_eq!(
            basemap.url,
            "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/data-moldova-20261116/moldova.pmtiles"
        );
        assert_eq!(
            basemap.installed_sha256,
            "7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e"
        );
        assert!(valid_sha256(&basemap.installed_sha256));
        let graph = &moldova.files[1];
        assert_eq!(graph.role, FileRole::Graph);
        assert_eq!(graph.compression, FileCompression::Gzip);
        assert_eq!(graph.installed_bytes, 110_858_815);
        assert_eq!(
            graph.url,
            "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/data-moldova-20261116/moldova.graph.bin.gz"
        );
        assert_eq!(
            graph.installed_sha256,
            "8cdbb0cb1d61c2d5857cb2677ab1343968223e4ae10892f0268c8feb0eb50e75"
        );
        assert_eq!(
            graph.transfer_sha256,
            "276d0ddd3058524038f53a6562c86a12c408693a1e84bbb316c9db16badc9087"
        );
        assert!(valid_sha256(&graph.transfer_sha256));
    }

    #[test]
    fn host_allowlist_accepts_release_hosts_and_rejects_off_list_or_downgrade() {
        for url in [
            "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/t/a.pmtiles",
            "https://release-assets.githubusercontent.com/github-production-release-asset/a",
            "https://objects.githubusercontent.com/github-production-release-asset/a",
        ] {
            assert!(valid_asset_url(url), "{url}");
        }
        for url in [
            "http://github.com/a.pmtiles",
            "https://evil.example/a.pmtiles",
            "https://github.com.evil.example/a.pmtiles",
            "https://127.0.0.1/a.pmtiles",
            "https://10.0.0.1/a.pmtiles",
            "https://169.254.169.254/latest/meta-data",
            "https://user:pass@github.com/a.pmtiles",
            "https://github.com:8443/a.pmtiles",
            "https://github.com/a.pmtiles#fragment",
        ] {
            assert!(!valid_asset_url(url), "{url}");
        }
        assert!(valid_catalog_url(
            "https://raw.githubusercontent.com/Prince-of-Nothing/pinpoint-maps/main/catalog.json"
        ));
        assert!(!valid_catalog_url("https://github.com/catalog.json"));
    }

    #[test]
    fn redirects_stay_on_the_release_allowlist() {
        for url in [
            "https://release-assets.githubusercontent.com/x",
            "https://objects.githubusercontent.com/x",
            "https://github.com/x",
        ] {
            assert!(
                asset_redirect_allowed(&url::Url::parse(url).expect("url")),
                "{url}"
            );
        }
        for url in [
            "http://release-assets.githubusercontent.com/x",
            "https://evil.example/x",
            "http://127.0.0.1/x",
            "https://10.0.0.1/x",
            "https://raw.githubusercontent.com/x",
        ] {
            assert!(
                !asset_redirect_allowed(&url::Url::parse(url).expect("url")),
                "{url}"
            );
        }
        assert!(catalog_redirect_allowed(
            &url::Url::parse("https://raw.githubusercontent.com/x").expect("url")
        ));
        assert!(!catalog_redirect_allowed(
            &url::Url::parse("https://github.com/x").expect("url")
        ));
    }

    #[test]
    fn loopback_is_only_for_configured_sources() {
        assert!(valid_local_asset_url("http://127.0.0.1:9000/a.pmtiles"));
        assert!(valid_local_asset_url("http://localhost:9000/a.pmtiles"));
        assert!(valid_local_asset_url("http://[::1]:9000/a.pmtiles"));
        assert!(valid_catalog_source_url(
            "http://127.0.0.1:9000/catalog.json"
        ));
        assert!(!valid_asset_url("http://127.0.0.1:9000/a.pmtiles"));
        assert!(!valid_asset_url("http://10.0.0.7/a.pmtiles"));
        assert!(!valid_catalog_source_url("http://10.0.0.7/catalog.json"));
    }

    #[test]
    fn rejects_bad_schema_hash_url_bbox_and_id() {
        let mutations: Vec<JsonMutation> = vec![
            ("schema", Box::new(|value| value["schema"] = 2.into())),
            (
                "sha256",
                Box::new(|value| {
                    value["regions"][0]["files"][0]["installed_sha256"] = "not-a-hash".into();
                }),
            ),
            (
                "URL",
                Box::new(|value| {
                    value["regions"][0]["files"][0]["url"] = "ftp://example/x".into();
                }),
            ),
            (
                // serde rejects a 3-element array before our finite check.
                "invalid length 3",
                Box::new(|value| {
                    value["regions"][0]["bbox"] = serde_json::json!([1.0, 2.0, 3.0]);
                }),
            ),
            (
                "id",
                Box::new(|value| {
                    value["regions"][0]["id"] = "../moldova".into();
                }),
            ),
            (
                "version",
                Box::new(|value| {
                    value["regions"][0]["version"] = "../2026-11-17".into();
                }),
            ),
            (
                "maxzoom",
                Box::new(|value| {
                    value["regions"][0]["maxzoom"] = 25.into();
                }),
            ),
        ];
        for (needle, mutate) in mutations {
            let mut value = bundled_value();
            mutate(&mut value);
            let error = RegionCatalog::parse(value.to_string().as_bytes())
                .expect_err("mutation must be rejected");
            assert!(error.contains(needle), "expected {needle:?} in {error:?}");
        }
    }

    #[test]
    fn rejects_a_non_finite_bbox_and_missing_roles() {
        let mut catalog = RegionCatalog::bundled();
        catalog.regions[0].bbox[0] = f64::NAN;
        assert!(catalog
            .validate()
            .expect_err("non-finite bbox")
            .contains("non-finite"));

        let mut catalog = RegionCatalog::bundled();
        catalog.regions[0]
            .files
            .retain(|file| file.role != FileRole::Graph);
        assert!(catalog
            .validate()
            .expect_err("missing graph")
            .contains("one basemap and one graph"));
    }

    #[test]
    fn rejects_basemap_and_graph_publication_violations() {
        let mut catalog = RegionCatalog::bundled();
        catalog.regions[0].files[0].compression = FileCompression::Gzip;
        assert!(catalog
            .validate()
            .expect_err("gzip basemap")
            .contains("basemap must be"));

        let mut catalog = RegionCatalog::bundled();
        catalog.regions[0].files[1].compression = FileCompression::Identity;
        assert!(catalog
            .validate()
            .expect_err("identity graph")
            .contains("graph must be gzip"));

        let mut value = bundled_value();
        value["regions"][0]["source_sha256"] = "00".repeat(32).into();
        assert!(RegionCatalog::parse(value.to_string().as_bytes())
            .expect_err("unknown field")
            .contains("unknown field"));
    }

    #[test]
    fn rejects_missing_hashes_unsafe_names_duplicate_roles_and_oversized_files() {
        let mutations: Vec<JsonMutation> = vec![
            (
                "sha256",
                Box::new(|value| {
                    value["regions"][0]["files"][0]["transfer_sha256"] = "".into();
                }),
            ),
            (
                "unsafe",
                Box::new(|value| {
                    value["regions"][0]["files"][0]["installed_name"] = "../escape.pmtiles".into();
                }),
            ),
            (
                "unsafe",
                Box::new(|value| {
                    value["regions"][0]["files"][0]["installed_name"] = "Moldova.PMTILES".into();
                }),
            ),
            (
                "more than one",
                Box::new(|value| {
                    value["regions"][0]["files"][1]["role"] = "basemap".into();
                }),
            ),
            (
                "2 GiB",
                Box::new(|value| {
                    value["regions"][0]["files"][0]["installed_bytes"] =
                        (MAX_FILE_BYTES + 1).into();
                }),
            ),
            (
                "zero",
                Box::new(|value| {
                    value["regions"][0]["files"][0]["transfer_bytes"] = 0.into();
                }),
            ),
        ];
        for (needle, mutate) in mutations {
            let mut value = bundled_value();
            mutate(&mut value);
            let error = RegionCatalog::parse(value.to_string().as_bytes())
                .expect_err("mutation must be rejected");
            assert!(error.contains(needle), "expected {needle:?} in {error:?}");
        }
    }

    #[test]
    fn rejects_duplicate_ids_and_oversized_bodies() {
        let mut value = bundled_value();
        let duplicate = value["regions"][0].clone();
        value["regions"]
            .as_array_mut()
            .expect("regions")
            .push(duplicate);
        assert!(RegionCatalog::parse(value.to_string().as_bytes())
            .expect_err("duplicate id")
            .contains("duplicate"));

        let oversized = vec![b' '; MAX_CATALOG_BYTES + 1];
        assert!(RegionCatalog::parse(&oversized)
            .expect_err("oversized")
            .contains("larger"));
    }

    #[test]
    fn catalog_url_resolution_prefers_an_allowlisted_override() {
        assert_eq!(resolve_catalog_url(None), DEFAULT_CATALOG_URL);
        assert_eq!(
            resolve_catalog_url(Some(
                " https://raw.githubusercontent.com/Prince-of-Nothing/pinpoint-maps/main/catalog.json "
            )),
            "https://raw.githubusercontent.com/Prince-of-Nothing/pinpoint-maps/main/catalog.json"
        );
        assert_eq!(
            resolve_catalog_url(Some("http://127.0.0.1:9000/catalog.json")),
            "http://127.0.0.1:9000/catalog.json"
        );
        assert_eq!(
            resolve_catalog_url(Some("https://evil.example/catalog.json")),
            DEFAULT_CATALOG_URL
        );
        assert_eq!(
            resolve_catalog_url(Some("file:///etc/passwd")),
            DEFAULT_CATALOG_URL
        );
    }

    fn remote_catalog(updated: &str) -> String {
        serde_json::json!({
            "schema": 1,
            "updated": updated,
            "regions": [{
                "id": "romania",
                "name": "Romania",
                "bbox": [20.0, 43.6, 29.7, 48.3],
                "maxzoom": 14,
                "version": "2026-11-16",
                "files": [{
                    "role": "basemap",
                    "transfer_name": "romania.pmtiles",
                    "installed_name": "romania.pmtiles",
                    "url": "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/romania/romania.pmtiles",
                    "transfer_bytes": 10,
                    "transfer_sha256": "7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e",
                    "installed_bytes": 10,
                    "installed_sha256": "7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e",
                    "compression": "identity"
                }, {
                    "role": "graph",
                    "transfer_name": "romania.graph.bin.gz",
                    "installed_name": "romania.graph.bin",
                    "url": "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/romania/romania.graph.bin.gz",
                    "transfer_bytes": 5,
                    "transfer_sha256": "276d0ddd3058524038f53a6562c86a12c408693a1e84bbb316c9db16badc9087",
                    "installed_bytes": 7,
                    "installed_sha256": "8cdbb0cb1d61c2d5857cb2677ab1343968223e4ae10892f0268c8feb0eb50e75",
                    "compression": "gzip"
                }]
            }]
        })
        .to_string()
    }

    async fn catalog_server(body: impl Into<String>) -> (String, Arc<Mutex<String>>) {
        let state = Arc::new(Mutex::new(body.into()));
        let shared = Arc::clone(&state);
        let app = Router::new().route(
            "/catalog.json",
            get(move || {
                let shared = Arc::clone(&shared);
                async move {
                    let body = shared
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner())
                        .clone();
                    (StatusCode::OK, Body::from(body)).into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("catalog listener");
        let address = listener.local_addr().expect("catalog address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{address}/catalog.json"), state)
    }

    #[tokio::test]
    async fn remote_fetch_uses_the_served_catalog() {
        let (url, _state) = catalog_server(remote_catalog("2026-11-18T12:00:00Z")).await;
        let cache = CatalogCache::remote(url);
        let catalog = cache.get().await;
        assert!(catalog.region("romania").is_some(), "{catalog:?}");
        assert!(cache.last_error().is_none(), "{:?}", cache.last_error());
    }

    #[tokio::test]
    async fn a_failed_or_older_refresh_keeps_the_last_good_catalog() {
        let (url, state) = catalog_server(remote_catalog("2026-11-18T12:00:00Z")).await;
        let cache = CatalogCache::remote(url);
        assert!(cache.refresh().await.is_ok());
        assert!(cache.current().region("romania").is_some());

        // A replayed catalog with an older `updated` is refused.
        *state.lock().expect("state") = remote_catalog("2026-11-17T12:00:00Z");
        let error = cache.refresh().await.expect_err("older catalog");
        assert!(error.contains("not newer"), "{error}");
        assert!(
            cache.current().region("romania").is_some(),
            "a refused rollback must keep the accepted catalog"
        );

        // A malformed body also keeps the last good catalog.
        *state.lock().expect("state") = "not json".to_string();
        assert!(cache.refresh().await.is_err());
        assert!(cache.current().region("romania").is_some());
        assert!(cache.last_error().is_some());
    }

    #[tokio::test]
    async fn remote_fetch_failure_keeps_bundled_when_none_succeeded() {
        let cache = CatalogCache::remote("http://127.0.0.1:1/catalog.json");
        let catalog = cache.get().await;
        assert_eq!(*catalog, RegionCatalog::bundled());
        assert!(cache.last_error().is_some(), "the failure is remembered");
        assert!(catalog.region("moldova").is_some());
    }

    #[tokio::test]
    async fn a_foreign_catalog_url_is_refused_and_keeps_bundled() {
        let cache = CatalogCache::remote("https://evil.example/catalog.json");
        let catalog = cache.get().await;
        assert_eq!(*catalog, RegionCatalog::bundled());
        assert!(
            cache
                .last_error()
                .unwrap_or_default()
                .contains("allowlisted"),
            "{:?}",
            cache.last_error()
        );
    }
}

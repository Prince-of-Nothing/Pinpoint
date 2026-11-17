//! Offline map + routing graph download manager and pack server.
//!
//! Both loopback map servers (desktop [`crate::map_server`] and Android
//! [`crate::mobile::map_server`]) and the web server mount the same routes
//! from this module:
//!
//! * `GET /map/catalog` — the cached region catalog with install flags and no
//!   asset URLs;
//! * `GET /map/config` — which PMTiles archive the page should load and which
//!   region packs are installed;
//! * `GET /map/offline/status` — pollable install state for the selected
//!   region;
//! * `POST /map/offline/download` — start the single background job for one
//!   region id; URLs, sizes and hashes never come from the page;
//! * `GET|HEAD /packs/<region>/<name>` and legacy `GET|HEAD /packs/<name>` —
//!   installed archives with HTTP `Range` support (`206`/`416`), which PMTiles
//!   requires.
//!
//! Two installer modes share the code:
//!
//! * [`OfflineInstaller::new`] (production) and
//!   [`OfflineInstaller::with_catalog`] install catalog regions under
//!   `packs/<region>/<installed_name>`, verify both hashes and lengths, and
//!   write `packs/<region>/installed.json` after every file succeeded. Legacy
//!   v2.3.0 root files (`packs/moldova.pmtiles`, `packs/moldova.graph.bin`) are
//!   migrated into `packs/moldova/` on first use without re-downloading.
//! * [`OfflineInstaller::with_plan`] keeps the original single-region plan
//!   (and root `packs/` layout) for tests and fixtures.
//!
//! Transfers stream into a sibling `.<name>.part`, are length- and SHA-256
//! checked on both the transfer and the installed bytes, and only then are
//! atomically renamed. A failed, truncated or corrupt download therefore never
//! replaces a working pack. Remote catalog URLs must be HTTPS on the GitHub
//! release hosts; redirects are capped and may only stay on those hosts (see
//! [`crate::region_catalog`]). Nothing here calls a CDN, a geocoder or a
//! routing service, and no session or account data is involved.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Extension, Path as UrlPath};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use flate2::{Decompress, FlushDecompress, Status};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;

pub use crate::region_catalog::FileCompression as Compression;
use crate::region_catalog::{
    valid_file_name, valid_region_id, valid_sha256, CatalogCache, FileRole, Region, RegionCatalog,
    RegionFile,
};

/// Immutable data release that holds the published Moldova artifacts.
pub const DEFAULT_BASE_URL: &str =
    "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/data-moldova-20261116";

/// Environment override for the artifact host (tests and mirrors). Only a
/// release host or a loopback URL is accepted.
pub const BASE_URL_ENV: &str = "PINPOINT_OFFLINE_BASE_URL";

/// Installed PMTiles archive name below a region directory.
pub const PACK_FILE: &str = "moldova.pmtiles";

/// Installed routing graph name below a region directory.
pub const GRAPH_FILE: &str = "moldova.graph.bin";

pub const PACK_BYTES: u64 = 93_172_399;
pub const PACK_SHA256: &str = "7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e";

pub const GRAPH_BYTES: u64 = 110_858_815;
pub const GRAPH_SHA256: &str = "8cdbb0cb1d61c2d5857cb2677ab1343968223e4ae10892f0268c8feb0eb50e75";

/// Published gzip transfer of the routing graph.
pub const GRAPH_GZ_FILE: &str = "moldova.graph.bin.gz";
pub const GRAPH_GZ_BYTES: u64 = 45_338_586;
pub const GRAPH_GZ_SHA256: &str =
    "276d0ddd3058524038f53a6562c86a12c408693a1e84bbb316c9db16badc9087";

/// Expected bytes on the wire for both artifacts (139 MB decimal).
pub const DOWNLOAD_BYTES: u64 = PACK_BYTES + GRAPH_GZ_BYTES;

/// Expected bytes on disk after decompression (204 MB decimal).
pub const INSTALLED_BYTES: u64 = PACK_BYTES + GRAPH_BYTES;

/// Region id used by the original single-pack layout and by the bundled
/// fallback catalog.
pub const LEGACY_REGION: &str = "moldova";

/// Marker written after a region installed completely.
pub const INSTALLED_MANIFEST: &str = "installed.json";

/// The Monaco fixture used until a pack is installed. It matches
/// `web/map/fixtures/monaco.pmtiles` and is served by both map servers.
pub const FIXTURE_URL: &str = "./fixtures/monaco.pmtiles";

const MIME_PMTILES: &str = "application/octet-stream";
const MIME_TEXT: &str = "text/plain; charset=utf-8";

/// Decoded output is written to disk in 64 KiB pieces; neither the 45 MB
/// transfer nor the 111 MB graph is buffered in memory.
const DECOMPRESS_BUFFER_BYTES: usize = 64 * 1024;

/// One release artifact and the metadata a successful install must match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSpec {
    /// Published asset name below the release base URL.
    pub transfer_name: &'static str,
    /// Exact transfer (compressed) byte length.
    pub transfer_bytes: u64,
    /// SHA-256 of the transfer bytes.
    pub transfer_sha256: String,
    /// Installed file name below `packs/`.
    pub name: &'static str,
    /// Exact installed (decompressed) byte length.
    pub expected_bytes: u64,
    /// SHA-256 of the installed (decompressed) bytes.
    pub sha256: String,
    pub compression: Compression,
}

impl ArtifactSpec {
    fn pack() -> Self {
        Self {
            transfer_name: PACK_FILE,
            transfer_bytes: PACK_BYTES,
            transfer_sha256: PACK_SHA256.to_string(),
            name: PACK_FILE,
            expected_bytes: PACK_BYTES,
            sha256: PACK_SHA256.to_string(),
            compression: Compression::Identity,
        }
    }

    fn graph() -> Self {
        Self {
            transfer_name: GRAPH_GZ_FILE,
            transfer_bytes: GRAPH_GZ_BYTES,
            transfer_sha256: GRAPH_GZ_SHA256.to_string(),
            name: GRAPH_FILE,
            expected_bytes: GRAPH_BYTES,
            sha256: GRAPH_SHA256.to_string(),
            compression: Compression::Gzip,
        }
    }
}

/// Artifact metadata used by one installer.
///
/// Production catalog installs use [`OfflineInstaller::new`]; tests inject a
/// small fixture plan (bytes + SHA-256) so the real 93/106 MB files are not
/// needed. A plan always installs below the root `packs/` directory, exactly
/// like v2.3.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadPlan {
    /// Optional host override. When `None`, [`BASE_URL_ENV`] or the release
    /// host is used when the job starts.
    pub base_url: Option<String>,
    pub pack: ArtifactSpec,
    pub graph: ArtifactSpec,
}

impl Default for DownloadPlan {
    fn default() -> Self {
        Self {
            base_url: None,
            pack: ArtifactSpec::pack(),
            graph: ArtifactSpec::graph(),
        }
    }
}

impl DownloadPlan {
    /// Total transfer bytes both artifacts are expected to download.
    pub fn expected_download_bytes(&self) -> u64 {
        self.pack
            .transfer_bytes
            .saturating_add(self.graph.transfer_bytes)
    }

    /// Total installed (decompressed) bytes both artifacts are expected to
    /// occupy.
    pub fn expected_installed_bytes(&self) -> u64 {
        self.pack
            .expected_bytes
            .saturating_add(self.graph.expected_bytes)
    }

    /// Build a fixture plan with the production file names and custom
    /// payloads. The graph is served gzip-compressed from `graph_gz` and must
    /// decode to `graph`. Both names stay fixed, so a plan can never escape
    /// `packs/`.
    pub fn fixture_gzip(
        base_url: impl Into<String>,
        pack: &[u8],
        graph_gz: &[u8],
        graph: &[u8],
    ) -> Self {
        let pack_sha256 = hex(&Sha256::digest(pack));
        let graph_sha256 = hex(&Sha256::digest(graph));
        let graph_gz_sha256 = hex(&Sha256::digest(graph_gz));
        Self {
            base_url: Some(base_url.into()),
            pack: ArtifactSpec {
                transfer_name: PACK_FILE,
                transfer_bytes: pack.len() as u64,
                transfer_sha256: pack_sha256.clone(),
                name: PACK_FILE,
                expected_bytes: pack.len() as u64,
                sha256: pack_sha256,
                compression: Compression::Identity,
            },
            graph: ArtifactSpec {
                transfer_name: GRAPH_GZ_FILE,
                transfer_bytes: graph_gz.len() as u64,
                transfer_sha256: graph_gz_sha256,
                name: GRAPH_FILE,
                expected_bytes: graph.len() as u64,
                sha256: graph_sha256,
                compression: Compression::Gzip,
            },
        }
    }

    #[cfg(test)]
    fn for_file(&self, name: &str) -> &ArtifactSpec {
        if name == self.graph.name {
            &self.graph
        } else {
            &self.pack
        }
    }
}

/// A resolved artifact: either a plan fixture or one catalog file.
#[derive(Debug, Clone)]
struct Artifact {
    role: FileRole,
    transfer_name: String,
    installed_name: String,
    url: String,
    transfer_bytes: u64,
    transfer_sha256: String,
    installed_bytes: u64,
    installed_sha256: String,
    compression: Compression,
}

impl Artifact {
    fn from_spec(spec: &ArtifactSpec, base: &str, role: FileRole) -> Self {
        Self {
            role,
            transfer_name: spec.transfer_name.to_string(),
            installed_name: spec.name.to_string(),
            url: format!("{base}/{}", spec.transfer_name),
            transfer_bytes: spec.transfer_bytes,
            transfer_sha256: spec.transfer_sha256.clone(),
            installed_bytes: spec.expected_bytes,
            installed_sha256: spec.sha256.clone(),
            compression: spec.compression,
        }
    }

    fn from_catalog(file: &RegionFile) -> Self {
        Self {
            role: file.role,
            transfer_name: file.transfer_name.clone(),
            installed_name: file.installed_name.clone(),
            url: file.url.clone(),
            transfer_bytes: file.transfer_bytes,
            transfer_sha256: file.transfer_sha256.clone(),
            installed_bytes: file.installed_bytes,
            installed_sha256: file.installed_sha256.clone(),
            compression: file.compression,
        }
    }
}

/// Install phase shown by `/map/offline/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// No job is running and no valid installed file is present.
    Idle,
    /// The response body is being streamed to a temp file.
    Downloading,
    /// The byte length and SHA-256 digest are being checked.
    Verifying,
    /// A verified file is installed at the final path.
    Installed,
    /// The last job failed. An already installed file is left untouched.
    Failed,
}

/// Per-artifact status returned by `/map/offline/status`.
#[derive(Debug, Clone, Serialize)]
pub struct FileStatus {
    /// A file with the expected byte length exists at the final path. The
    /// length is checked on every status call; the SHA-256 was checked when
    /// the file was installed.
    pub installed: bool,
    /// Current installed (decompressed) file length, or `0` when absent.
    pub bytes: u64,
    /// Expected installed (decompressed) file length.
    pub expected_bytes: u64,
    /// Expected transfer length of the published artifact. The graph is gzip
    /// on the wire, so this is smaller than `expected_bytes`.
    pub download_bytes: u64,
    /// Transfer bytes received by the running job, or the full transfer
    /// length when the artifact is installed.
    pub downloaded_bytes: u64,
    /// Decompressed bytes written by the running job, or the installed size
    /// at rest.
    pub progress_bytes: u64,
    pub phase: Phase,
    /// Failure detail. `null` when there is no error.
    pub error: Option<String>,
}

/// Status body for `GET /map/offline/status`.
#[derive(Debug, Clone, Serialize)]
pub struct OfflineStatus {
    pub pack: FileStatus,
    pub graph: FileStatus,
    /// Total transfer bytes expected for the selected region.
    pub expected_download_bytes: u64,
    /// Selected region id, or `null` when the catalog has no matching entry.
    #[serde(default)]
    pub region: Option<String>,
}

/// One installed region in `/map/config`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapRegion {
    pub id: String,
    pub name: String,
    pub bbox: [f64; 4],
    pub version: String,
    pub pmtiles_url: String,
    pub graph_url: Option<String>,
    pub installed: bool,
}

/// Config body for `GET /map/config`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapConfig {
    pub pmtiles_url: String,
    pub graph_installed: bool,
    pub pack_installed: bool,
    /// Regions with an installed basemap, in catalog order.
    #[serde(default)]
    pub regions: Vec<MapRegion>,
}

/// One region in the page-facing catalog (`GET /map/catalog`).
#[derive(Debug, Clone, Serialize)]
pub struct RegionView {
    pub id: String,
    pub name: String,
    pub bbox: [f64; 4],
    pub maxzoom: u8,
    pub version: String,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub files: Vec<RegionFileView>,
}

/// One file in the page-facing catalog; asset URLs are deliberately absent.
#[derive(Debug, Clone, Serialize)]
pub struct RegionFileView {
    pub role: FileRole,
    pub transfer_bytes: u64,
    pub installed_bytes: u64,
    pub installed: bool,
}

/// Page-facing catalog body for `GET /map/catalog`.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogView {
    pub schema: u32,
    pub updated: String,
    pub regions: Vec<RegionView>,
}

/// Metadata written after a catalog region installed completely.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledManifest {
    id: String,
    version: String,
    installed_at: String,
    files: Vec<InstalledFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledFile {
    role: FileRole,
    name: String,
    bytes: u64,
    sha256: String,
}

/// Outcome of asking an installer to start a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    Started,
    AlreadyRunning,
    /// No writable `packs/` directory or an invalid download plan.
    Unavailable(String),
    /// The catalog has no region with this id.
    UnknownRegion(String),
}

#[derive(Debug, Clone)]
struct FileState {
    phase: Phase,
    /// Transfer bytes received for the current job.
    downloaded_bytes: u64,
    /// Decompressed bytes written for the current job.
    progress_bytes: u64,
    error: Option<String>,
}

impl FileState {
    fn idle() -> Self {
        Self {
            phase: Phase::Idle,
            downloaded_bytes: 0,
            progress_bytes: 0,
            error: None,
        }
    }

    fn downloading() -> Self {
        Self {
            phase: Phase::Downloading,
            downloaded_bytes: 0,
            progress_bytes: 0,
            error: None,
        }
    }
}

#[derive(Debug)]
struct JobState {
    running: bool,
    /// Region of the current or most recent job.
    region: String,
    pack: FileState,
    graph: FileState,
}

impl JobState {
    fn new(region: &str) -> Self {
        Self {
            running: false,
            region: region.to_string(),
            pack: FileState::idle(),
            graph: FileState::idle(),
        }
    }

    fn state_mut(&mut self, role: FileRole) -> &mut FileState {
        match role {
            FileRole::Basemap => &mut self.pack,
            FileRole::Graph => &mut self.graph,
        }
    }
}

/// A region resolved for one job or status poll.
#[derive(Debug, Clone)]
struct Resolved {
    region: Region,
    dir: PathBuf,
    artifacts: Vec<Artifact>,
}

#[derive(Debug)]
enum Mode {
    /// Original single-region plan; installs below the root `packs/` dir.
    Plan(Box<DownloadPlan>),
    /// Catalog-driven multi-region installer.
    Catalog(Arc<CatalogCache>),
}

/// Download manager bound to one `packs/` directory.
pub struct OfflineInstaller {
    packs_dir: Option<PathBuf>,
    mode: Mode,
    job: Mutex<JobState>,
    /// Serialises legacy root-file migration without holding the job lock
    /// across filesystem calls.
    legacy: Mutex<()>,
}

impl OfflineInstaller {
    /// Installer for the production region catalog. The remote catalog is
    /// fetched lazily (five-second timeout, one-hour cache) and falls back to
    /// the bundled `web/catalog.json`.
    pub fn new(packs_dir: Option<PathBuf>) -> Self {
        Self::with_catalog_cache(
            packs_dir,
            Arc::new(CatalogCache::remote_default()),
            LEGACY_REGION,
        )
    }

    /// Installer with explicit artifact metadata (tests/fixtures). Installs
    /// below the root `packs/` directory like v2.3.0.
    pub fn with_plan(packs_dir: Option<PathBuf>, plan: DownloadPlan) -> Self {
        Self {
            packs_dir,
            mode: Mode::Plan(Box::new(plan)),
            job: Mutex::new(JobState::new(LEGACY_REGION)),
            legacy: Mutex::new(()),
        }
    }

    /// Installer with an explicit catalog (tests, bundled fallback, or an
    /// operator-pinned catalog). Installs below `packs/<region>/`.
    pub fn with_catalog(
        packs_dir: Option<PathBuf>,
        catalog: RegionCatalog,
        region_id: impl AsRef<str>,
    ) -> Self {
        Self::with_catalog_cache(packs_dir, Arc::new(CatalogCache::fixed(catalog)), region_id)
    }

    /// Installer with an explicit catalog cache (remote-fetch tests).
    pub fn with_catalog_cache(
        packs_dir: Option<PathBuf>,
        cache: Arc<CatalogCache>,
        region_id: impl AsRef<str>,
    ) -> Self {
        let selected = normalize_region(&cache.current(), region_id.as_ref());
        Self {
            packs_dir,
            mode: Mode::Catalog(cache),
            job: Mutex::new(JobState::new(&selected)),
            legacy: Mutex::new(()),
        }
    }

    /// Configured `packs/` directory, if any.
    pub fn packs_dir(&self) -> Option<&Path> {
        self.packs_dir.as_deref()
    }

    /// Currently selected region id.
    pub fn selected_region(&self) -> String {
        self.job
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .region
            .clone()
    }

    /// Path of an installed plan artifact. Only the two known root names are
    /// accepted, so callers cannot join arbitrary relative paths.
    pub fn artifact_path(&self, name: &str) -> Option<PathBuf> {
        if name != PACK_FILE && name != GRAPH_FILE {
            return None;
        }
        self.packs_dir.as_ref().map(|dir| dir.join(name))
    }

    /// The catalog used by the handlers. Plan mode exposes a synthetic
    /// single-region catalog so `/map/catalog` and region serving keep
    /// working for fixtures.
    pub async fn catalog(&self) -> Arc<RegionCatalog> {
        match &self.mode {
            Mode::Plan(plan) => Arc::new(RegionCatalog {
                schema: crate::region_catalog::CATALOG_SCHEMA,
                updated: String::new(),
                regions: vec![synthetic_region(plan)],
            }),
            Mode::Catalog(cache) => cache.get().await,
        }
    }

    /// Load a stale remote catalog before accepting a download id.
    pub async fn ensure_catalog(&self) {
        let _ = self.catalog().await;
    }

    /// Current status. Metadata checks touch the filesystem but never hash a
    /// multi-megabyte installed file on a status poll.
    pub async fn status(&self) -> OfflineStatus {
        self.migrate_legacy();
        let (region_id, pack_state, graph_state, running) = {
            let guard = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
            (
                guard.region.clone(),
                guard.pack.clone(),
                guard.graph.clone(),
                guard.running,
            )
        };
        let Some(resolved) = self.resolved_region(&region_id).await else {
            return OfflineStatus {
                pack: empty_file_status(),
                graph: empty_file_status(),
                expected_download_bytes: 0,
                region: Some(region_id),
            };
        };
        let pack = match resolved
            .artifacts
            .iter()
            .find(|artifact| artifact.role == FileRole::Basemap)
        {
            Some(artifact) => {
                self.file_status(&resolved.dir, artifact, &pack_state, running)
                    .await
            }
            None => empty_file_status(),
        };
        let graph = match resolved
            .artifacts
            .iter()
            .find(|artifact| artifact.role == FileRole::Graph)
        {
            Some(artifact) => {
                self.file_status(&resolved.dir, artifact, &graph_state, running)
                    .await
            }
            None => empty_file_status(),
        };
        let expected_download_bytes = resolved
            .artifacts
            .iter()
            .map(|artifact| artifact.transfer_bytes)
            .fold(0u64, u64::saturating_add);
        OfflineStatus {
            pack,
            graph,
            expected_download_bytes,
            region: Some(region_id),
        }
    }

    async fn file_status(
        &self,
        dir: &Path,
        artifact: &Artifact,
        state: &FileState,
        running: bool,
    ) -> FileStatus {
        let (installed, bytes) = match self
            .find_installed(dir, &artifact.installed_name, artifact.installed_bytes)
            .await
        {
            Some(bytes) => (true, bytes),
            None => (false, 0),
        };

        let phase = match state.phase {
            Phase::Idle if installed => Phase::Installed,
            other => other,
        };
        let (progress_bytes, downloaded_bytes) = if !running && phase == Phase::Installed {
            (bytes, artifact.transfer_bytes)
        } else {
            (state.progress_bytes, state.downloaded_bytes)
        };

        FileStatus {
            installed,
            bytes,
            expected_bytes: artifact.installed_bytes,
            download_bytes: artifact.transfer_bytes,
            downloaded_bytes,
            progress_bytes,
            phase,
            error: state.error.clone(),
        }
    }

    async fn find_installed(&self, dir: &Path, name: &str, expected: u64) -> Option<u64> {
        if let Ok(metadata) = tokio::fs::metadata(dir.join(name)).await {
            if metadata.is_file() && metadata.len() == expected {
                return Some(metadata.len());
            }
        }
        // A failed legacy migration leaves the v2.3.0 root file in place; keep
        // serving and reporting it.
        if name == PACK_FILE || name == GRAPH_FILE {
            if let Some(root) = self.packs_dir.as_ref() {
                if root != dir {
                    if let Ok(metadata) = tokio::fs::metadata(root.join(name)).await {
                        if metadata.is_file() && metadata.len() == expected {
                            return Some(metadata.len());
                        }
                    }
                }
            }
        }
        None
    }

    async fn file_matches(&self, dir: &Path, file: &RegionFile) -> bool {
        self.find_installed(dir, &file.installed_name, file.installed_bytes)
            .await
            .is_some()
    }

    /// `GET /map/config` body.
    pub async fn config(&self) -> MapConfig {
        self.migrate_legacy();
        let catalog = self.catalog().await;
        let selected = self.selected_region();
        let mut installed = Vec::new();
        let mut selected_entry = None;

        for region in &catalog.regions {
            let Some(dir) = self.storage_dir(&region.id) else {
                continue;
            };
            let Some(basemap) = region
                .files
                .iter()
                .find(|file| file.role == FileRole::Basemap)
            else {
                continue;
            };
            if !self.file_matches(&dir, basemap).await {
                continue;
            }
            let graph = region
                .files
                .iter()
                .find(|file| file.role == FileRole::Graph);
            let graph_installed = match graph {
                Some(graph) => self.file_matches(&dir, graph).await,
                None => false,
            };
            let entry = MapRegion {
                id: region.id.clone(),
                name: region.name.clone(),
                bbox: region.bbox,
                version: region.version.clone(),
                pmtiles_url: self.file_url(region, basemap),
                graph_url: match graph {
                    Some(graph) if graph_installed => Some(self.file_url(region, graph)),
                    _ => None,
                },
                installed: graph_installed,
            };
            if region.id == selected {
                selected_entry = Some(entry.clone());
            }
            installed.push(entry);
        }

        let chosen = selected_entry.or_else(|| installed.first().cloned());
        let (pmtiles_url, pack_installed, graph_installed) = match &chosen {
            Some(entry) => (entry.pmtiles_url.clone(), true, entry.installed),
            None => (FIXTURE_URL.to_string(), false, false),
        };
        MapConfig {
            pmtiles_url,
            graph_installed,
            pack_installed,
            regions: installed,
        }
    }

    /// `GET /map/catalog` body: install flags, no asset URLs.
    pub async fn catalog_view(&self) -> CatalogView {
        self.migrate_legacy();
        let catalog = self.catalog().await;
        let mut regions = Vec::new();
        for region in &catalog.regions {
            let dir = self.storage_dir(&region.id);
            let mut files = Vec::new();
            let mut installed = true;
            for file in &region.files {
                let file_installed = match &dir {
                    Some(dir) => self.file_matches(dir, file).await,
                    None => false,
                };
                installed &= file_installed;
                files.push(RegionFileView {
                    role: file.role,
                    transfer_bytes: file.transfer_bytes,
                    installed_bytes: file.installed_bytes,
                    installed: file_installed,
                });
            }
            let installed_version = match &dir {
                Some(dir) => self.manifest_version(dir, &region.id).await,
                None => None,
            };
            regions.push(RegionView {
                id: region.id.clone(),
                name: region.name.clone(),
                bbox: region.bbox,
                maxzoom: region.maxzoom,
                version: region.version.clone(),
                installed,
                installed_version,
                files,
            });
        }
        CatalogView {
            schema: catalog.schema,
            updated: catalog.updated.clone(),
            regions,
        }
    }

    async fn manifest_version(&self, dir: &Path, region_id: &str) -> Option<String> {
        let bytes = tokio::fs::read(dir.join(INSTALLED_MANIFEST)).await.ok()?;
        let manifest: InstalledManifest = serde_json::from_slice(&bytes).ok()?;
        (manifest.id == region_id).then_some(manifest.version)
    }

    fn storage_dir(&self, region_id: &str) -> Option<PathBuf> {
        let root = self.packs_dir.as_ref()?;
        Some(match self.mode {
            Mode::Plan(_) => root.clone(),
            Mode::Catalog(_) => root.join(region_id),
        })
    }

    fn file_url(&self, region: &Region, file: &RegionFile) -> String {
        match &self.mode {
            Mode::Plan(_) => format!("/packs/{}", file.installed_name),
            // Keep the v2.3.0 page URL working for the bundled region.
            Mode::Catalog(_) if region.id == LEGACY_REGION && file.role == FileRole::Basemap => {
                format!("/packs/{}", file.installed_name)
            }
            Mode::Catalog(_) => format!("/packs/{}/{}", region.id, file.installed_name),
        }
    }

    async fn resolved_region(&self, region_id: &str) -> Option<Resolved> {
        let root = self.packs_dir.as_ref()?;
        match &self.mode {
            Mode::Plan(plan) => {
                if region_id != LEGACY_REGION {
                    return None;
                }
                Some(Resolved {
                    region: synthetic_region(plan),
                    dir: root.clone(),
                    artifacts: plan_artifacts(plan),
                })
            }
            Mode::Catalog(cache) => {
                let catalog = cache.get().await;
                let region = catalog.region(region_id)?.clone();
                let artifacts = region.files.iter().map(Artifact::from_catalog).collect();
                Some(Resolved {
                    region,
                    dir: root.join(region_id),
                    artifacts,
                })
            }
        }
    }

    /// Start the single background job for the selected region.
    pub fn start(self: &Arc<Self>) -> StartOutcome {
        let region = self.selected_region();
        self.start_region(&region)
    }

    /// Start the single background job for one catalog region. The id is the
    /// only input; URL, size and hash always come from the catalog.
    pub fn start_region(self: &Arc<Self>, region_id: &str) -> StartOutcome {
        if !valid_region_id(region_id) {
            return StartOutcome::UnknownRegion(region_id.to_string());
        }
        self.migrate_legacy();
        let resolved = match self.resolve_for_start(region_id) {
            Ok(resolved) => resolved,
            Err(outcome) => return outcome,
        };
        let needed = resolved
            .artifacts
            .iter()
            .map(|artifact| artifact.installed_bytes)
            .fold(0u64, u64::saturating_add);
        if !space_is_sufficient(available_disk_bytes(&resolved.dir), needed) {
            return StartOutcome::Unavailable(format!(
                "not enough free disk space for {needed} installed bytes"
            ));
        }

        {
            let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
            if job.running {
                return StartOutcome::AlreadyRunning;
            }
            job.running = true;
            job.region = resolved.region.id.clone();
            job.pack = FileState::downloading();
            job.graph = FileState::downloading();
        }

        let installer = Arc::clone(self);
        tokio::spawn(async move {
            installer.run_job(resolved).await;
        });
        StartOutcome::Started
    }

    fn resolve_for_start(&self, region_id: &str) -> Result<Resolved, StartOutcome> {
        let Some(root) = self.packs_dir.as_ref() else {
            return Err(StartOutcome::Unavailable(
                "this map server has no writable packs directory".to_string(),
            ));
        };
        let resolved = match &self.mode {
            Mode::Plan(plan) => {
                if region_id != LEGACY_REGION {
                    return Err(StartOutcome::UnknownRegion(region_id.to_string()));
                }
                Resolved {
                    region: synthetic_region(plan),
                    dir: root.clone(),
                    artifacts: plan_artifacts(plan),
                }
            }
            Mode::Catalog(cache) => {
                let catalog = cache.current();
                let Some(region) = catalog.region(region_id).cloned() else {
                    return Err(StartOutcome::UnknownRegion(region_id.to_string()));
                };
                let artifacts = region.files.iter().map(Artifact::from_catalog).collect();
                Resolved {
                    region,
                    dir: root.join(region_id),
                    artifacts,
                }
            }
        };
        let strict_urls = matches!(&self.mode, Mode::Catalog(cache) if cache.is_remote());
        for artifact in &resolved.artifacts {
            validate_artifact(artifact, strict_urls).map_err(StartOutcome::Unavailable)?;
        }
        Ok(resolved)
    }

    async fn run_job(self: Arc<Self>, resolved: Resolved) {
        let client = match build_client() {
            Ok(client) => client,
            Err(error) => {
                self.fail_job(&error);
                return;
            }
        };
        let catalog_mode = matches!(self.mode, Mode::Catalog(_));

        for artifact in &resolved.artifacts {
            if let Err(error) = self.download_one(&client, &resolved.dir, artifact).await {
                tracing::warn!(
                    artifact = %artifact.installed_name,
                    %error,
                    "offline pack download failed"
                );
                self.fail_file(artifact.role, &error);
                self.finish_job(false);
                return;
            }
        }
        if catalog_mode {
            if let Err(error) = self
                .write_manifest(&resolved.dir, &resolved.region, &resolved.artifacts)
                .await
            {
                tracing::warn!(region = %resolved.region.id, %error, "could not write installed.json");
            }
        }
        self.finish_job(true);
        tracing::info!(
            region = %resolved.region.id,
            files = resolved.artifacts.len(),
            "offline region pack installed"
        );
    }

    async fn download_one(
        &self,
        client: &reqwest::Client,
        dir: &Path,
        artifact: &Artifact,
    ) -> Result<(), String> {
        let Some(root) = self.packs_dir.as_ref() else {
            return Err("no packs directory".to_string());
        };
        if !valid_file_name(&artifact.installed_name) || !valid_file_name(&artifact.transfer_name) {
            return Err(format!(
                "unsupported artifact {:?} / transfer {:?}",
                artifact.installed_name, artifact.transfer_name
            ));
        }
        if let Ok(metadata) = tokio::fs::symlink_metadata(dir).await {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "refusing to write through symlink {}",
                    dir.display()
                ));
            }
        }
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
        let root = tokio::fs::canonicalize(root)
            .await
            .map_err(|error| format!("could not resolve {}: {error}", dir.display()))?;
        let dir = tokio::fs::canonicalize(dir)
            .await
            .map_err(|error| format!("could not resolve {}: {error}", dir.display()))?;
        if !dir.starts_with(&root) {
            return Err(format!("{} escapes the packs directory", dir.display()));
        }

        let target = dir.join(&artifact.installed_name);
        if !target.starts_with(&root) {
            return Err(format!("{} escapes the packs directory", target.display()));
        }
        if let Ok(metadata) = tokio::fs::symlink_metadata(&target).await {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "refusing to replace symlinked artifact {}",
                    target.display()
                ));
            }
        }
        let temp = dir.join(format!(".{}.part", artifact.installed_name));
        // A previous crash may have left a partial temp file. It is never a
        // valid install and must not be resumed blindly.
        let _ = tokio::fs::remove_file(&temp).await;

        let (digest, transfer_digest) = match self
            .stream_artifact(client, artifact, &artifact.url, &temp)
            .await
        {
            Ok(digests) => digests,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(error);
            }
        };

        self.set_phase(artifact.role, Phase::Verifying);
        if !valid_sha256(&artifact.installed_sha256)
            || digest != artifact.installed_sha256.to_ascii_lowercase()
        {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!(
                "{} failed installed SHA-256 verification (got {digest}, expected {})",
                artifact.url, artifact.installed_sha256
            ));
        }
        // The transfer hash is mandatory; an empty expected hash is rejected
        // when the job starts, so this comparison is never skipped.
        if transfer_digest != artifact.transfer_sha256.to_ascii_lowercase() {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!(
                "{} failed transfer SHA-256 verification (got {transfer_digest}, expected {})",
                artifact.url, artifact.transfer_sha256
            ));
        }

        // Same-directory rename is atomic on the supported filesystems and
        // replaces an older verified pack on Windows and Unix alike.
        if let Err(error) = tokio::fs::rename(&temp, &target).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!("could not install {}: {error}", target.display()));
        }

        {
            let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
            let state = job.state_mut(artifact.role);
            state.phase = Phase::Installed;
            state.downloaded_bytes = artifact.transfer_bytes;
            state.progress_bytes = artifact.installed_bytes;
            state.error = None;
        }
        Ok(())
    }

    async fn write_manifest(
        &self,
        dir: &Path,
        region: &Region,
        artifacts: &[Artifact],
    ) -> Result<(), String> {
        let manifest = InstalledManifest {
            id: region.id.clone(),
            version: region.version.clone(),
            installed_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            files: artifacts
                .iter()
                .map(|artifact| InstalledFile {
                    role: artifact.role,
                    name: artifact.installed_name.clone(),
                    bytes: artifact.installed_bytes,
                    sha256: artifact.installed_sha256.clone(),
                })
                .collect(),
        };
        let bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| format!("could not encode installed.json: {error}"))?;
        let temp = dir.join(format!(".{INSTALLED_MANIFEST}.part"));
        tokio::fs::write(&temp, bytes)
            .await
            .map_err(|error| format!("could not write {}: {error}", temp.display()))?;
        if let Err(error) = tokio::fs::rename(&temp, dir.join(INSTALLED_MANIFEST)).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!("could not install installed.json: {error}"));
        }
        Ok(())
    }

    /// Move v2.3.0 root-level Moldova files into `packs/moldova/` once. The
    /// move is metadata-only on one volume; a copy fallback never deletes the
    /// source unless the destination is complete, and an existing destination
    /// is never overwritten.
    fn migrate_legacy(&self) {
        if !matches!(self.mode, Mode::Catalog(_)) {
            return;
        }
        let Some(root) = self.packs_dir.as_ref() else {
            return;
        };
        let _guard = self
            .legacy
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        {
            let job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
            if job.running {
                return;
            }
        }
        let Ok(root) = std::fs::canonicalize(root) else {
            return;
        };
        let region_dir = root.join(LEGACY_REGION);
        if !region_dir.starts_with(&root) {
            return;
        }
        if let Ok(metadata) = std::fs::symlink_metadata(&region_dir) {
            if metadata.file_type().is_symlink() {
                tracing::warn!("not migrating legacy packs into a symlinked directory");
                return;
            }
        }

        let pairs = [
            (PACK_FILE, FileRole::Basemap),
            (GRAPH_FILE, FileRole::Graph),
        ];
        let mut pending = Vec::new();
        for (name, role) in pairs {
            let source = root.join(name);
            let target = region_dir.join(name);
            if !source.is_file() || target.exists() {
                continue;
            }
            pending.push((name, role, source));
        }
        if pending.is_empty() {
            return;
        }
        if let Err(error) = std::fs::create_dir_all(&region_dir) {
            tracing::warn!(%error, "could not create the legacy region directory");
            return;
        }
        let Ok(region_dir) = std::fs::canonicalize(&region_dir) else {
            return;
        };
        if !region_dir.starts_with(&root) {
            tracing::warn!("legacy region directory escapes the packs directory");
            return;
        }

        let mut moved = Vec::new();
        for (name, role, source) in pending {
            let target = region_dir.join(name);
            match std::fs::rename(&source, &target) {
                Ok(()) => moved.push((name, role)),
                Err(rename_error) => match copy_then_remove(&source, &target) {
                    Ok(()) => moved.push((name, role)),
                    Err(error) => {
                        tracing::warn!(%rename_error, %error, %name, "legacy pack move failed");
                        roll_back(&root, &region_dir, &moved);
                        return;
                    }
                },
            }
        }
        if !region_dir.join(INSTALLED_MANIFEST).exists() {
            write_legacy_manifest(&region_dir);
        }
    }

    /// Stream one response into `temp`, decoding gzip as it arrives. Returns
    /// `(installed_sha256, transfer_sha256)`. The caller removes `temp` on
    /// any error, so a partial or corrupt transfer never reaches `packs/`.
    async fn stream_artifact(
        &self,
        client: &reqwest::Client,
        artifact: &Artifact,
        url: &str,
        temp: &Path,
    ) -> Result<(String, String), String> {
        let response = client
            .get(url)
            .header(header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(|error| format!("could not download {url}: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "{url} returned HTTP {} instead of a successful artifact",
                response.status()
            ));
        }
        if let Some(length) = response.content_length() {
            if length != artifact.transfer_bytes {
                return Err(format!(
                    "{url} declared {length} bytes but {} are expected",
                    artifact.transfer_bytes
                ));
            }
        }

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temp)
            .await
            .map_err(|error| format!("could not create {}: {error}", temp.display()))?;
        let mut installed_hasher = Sha256::new();
        let mut transfer_hasher = Sha256::new();
        let mut downloaded = 0_u64;
        let mut written = 0_u64;
        let identity = artifact.compression == Compression::Identity;
        // flate2's pure-Rust backend inflates raw DEFLATE; the gzip wrapper
        // is parsed incrementally here, so a 45 MB transfer is never buffered.
        let mut decoder = (!identity).then(|| Decompress::new(false));
        let mut header = (!identity).then(GzipHeaderParser::new);
        let mut finished = identity;
        let mut trailer = Vec::with_capacity(8);
        let mut buffered = Vec::new();
        let mut buffer = vec![0_u8; DECOMPRESS_BUFFER_BYTES];
        let mut body = response;

        loop {
            let chunk = body
                .chunk()
                .await
                .map_err(|error| format!("download of {url} failed: {error}"))?;
            let Some(chunk) = chunk else {
                break;
            };
            if downloaded.saturating_add(chunk.len() as u64) > artifact.transfer_bytes {
                return Err(format!(
                    "{url} sent more than the expected {} bytes",
                    artifact.transfer_bytes
                ));
            }
            transfer_hasher.update(&chunk);
            downloaded += chunk.len() as u64;

            if identity {
                write_installed(
                    &mut file,
                    &mut installed_hasher,
                    &mut written,
                    artifact.installed_bytes,
                    url,
                    &chunk,
                )
                .await?;
                self.set_progress(artifact.role, downloaded, written);
                continue;
            }

            if finished {
                trailer.extend_from_slice(&chunk);
                if trailer.len() > 8 {
                    return Err(format!("{url} sent bytes after the gzip trailer"));
                }
                self.set_progress(artifact.role, downloaded, written);
                continue;
            }

            let mut header_len = 0;
            let mut parsed_header = false;
            if let Some(parser) = header.as_mut() {
                match parser.push(&chunk)? {
                    Some(len) => {
                        let mut parser = header.take().expect("gzip header parser");
                        buffered = std::mem::take(&mut parser.bytes);
                        header_len = len;
                        parsed_header = true;
                    }
                    None => {
                        self.set_progress(artifact.role, downloaded, written);
                        continue;
                    }
                }
            }

            let mut input: &[u8] = if parsed_header {
                &buffered[header_len..]
            } else {
                &chunk
            };
            while !input.is_empty() {
                let decoder = decoder.as_mut().expect("gzip decoder");
                let before_in = decoder.total_in();
                let before_out = decoder.total_out();
                let status = decoder
                    .decompress(input, &mut buffer, FlushDecompress::None)
                    .map_err(|error| format!("{url} gzip decompression failed: {error}"))?;
                let consumed = (decoder.total_in() - before_in) as usize;
                let produced = (decoder.total_out() - before_out) as usize;
                if produced > 0 {
                    write_installed(
                        &mut file,
                        &mut installed_hasher,
                        &mut written,
                        artifact.installed_bytes,
                        url,
                        &buffer[..produced],
                    )
                    .await?;
                }
                input = &input[consumed..];
                if status == Status::StreamEnd {
                    finished = true;
                    trailer.extend_from_slice(input);
                    if trailer.len() > 8 {
                        return Err(format!("{url} sent bytes after the gzip trailer"));
                    }
                    break;
                }
                if input.is_empty() {
                    // A full output buffer may still hold pending decoded
                    // bytes; keep draining with empty input.
                    if produced > 0 && produced == buffer.len() {
                        continue;
                    }
                    break;
                }
                if consumed == 0 && produced == 0 {
                    return Err(format!("{url} gzip decoder made no progress"));
                }
            }
            self.set_progress(artifact.role, downloaded, written);
        }

        if downloaded != artifact.transfer_bytes {
            return Err(format!(
                "{url} ended after {downloaded} transfer bytes; {} were expected",
                artifact.transfer_bytes
            ));
        }
        if !identity {
            if !finished {
                return Err(format!("{url} gzip stream ended before its trailer"));
            }
            if trailer.len() != 8 {
                return Err(format!("{url} gzip trailer is incomplete"));
            }
            let declared = u32::from_le_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);
            if declared != written as u32 {
                return Err(format!(
                    "{url} gzip size trailer is {declared} but {written} bytes were decoded"
                ));
            }
        }
        if written != artifact.installed_bytes {
            return Err(format!(
                "{url} produced {written} bytes after decompression; {} were expected",
                artifact.installed_bytes
            ));
        }
        file.flush()
            .await
            .map_err(|error| format!("could not flush {}: {error}", temp.display()))?;
        file.sync_all()
            .await
            .map_err(|error| format!("could not sync {}: {error}", temp.display()))?;
        drop(file);

        Ok((
            hex(&installed_hasher.finalize()),
            hex(&transfer_hasher.finalize()),
        ))
    }

    fn set_progress(&self, role: FileRole, downloaded: u64, written: u64) {
        let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
        let state = job.state_mut(role);
        state.phase = Phase::Downloading;
        state.downloaded_bytes = downloaded;
        state.progress_bytes = written;
    }

    fn set_phase(&self, role: FileRole, phase: Phase) {
        let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
        job.state_mut(role).phase = phase;
    }

    fn fail_file(&self, role: FileRole, error: &str) {
        let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
        let state = job.state_mut(role);
        state.phase = Phase::Failed;
        state.error = Some(error.to_string());
    }

    fn fail_job(&self, error: &str) {
        {
            let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
            job.pack.phase = Phase::Failed;
            job.pack.error = Some(error.to_string());
            job.graph.phase = Phase::Failed;
            job.graph.error = Some(error.to_string());
        }
        self.finish_job(false);
    }

    fn finish_job(&self, success: bool) {
        let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
        job.running = false;
        if !success {
            // The failed artifact keeps `Failed`; an artifact that was never
            // reached must not stay stuck in `Downloading` forever.
            settle_unstarted(&mut job.pack);
            settle_unstarted(&mut job.graph);
        }
    }
}

fn empty_file_status() -> FileStatus {
    FileStatus {
        installed: false,
        bytes: 0,
        expected_bytes: 0,
        download_bytes: 0,
        downloaded_bytes: 0,
        progress_bytes: 0,
        phase: Phase::Idle,
        error: None,
    }
}

fn normalize_region(catalog: &RegionCatalog, preferred: &str) -> String {
    if valid_region_id(preferred) && catalog.region(preferred).is_some() {
        preferred.to_string()
    } else {
        catalog
            .regions
            .first()
            .map(|region| region.id.clone())
            .unwrap_or_else(|| LEGACY_REGION.to_string())
    }
}

fn plan_artifacts(plan: &DownloadPlan) -> Vec<Artifact> {
    let base = resolve_base_url(plan.base_url.as_deref());
    vec![
        Artifact::from_spec(&plan.pack, &base, FileRole::Basemap),
        Artifact::from_spec(&plan.graph, &base, FileRole::Graph),
    ]
}

fn synthetic_region(plan: &DownloadPlan) -> Region {
    let base = resolve_base_url(plan.base_url.as_deref());
    let mut region = RegionCatalog::bundled()
        .region(LEGACY_REGION)
        .cloned()
        .unwrap_or_else(|| Region {
            id: LEGACY_REGION.to_string(),
            name: "Moldova".to_string(),
            bbox: [0.0, 0.0, 0.0, 0.0],
            maxzoom: 14,
            version: "fixture".to_string(),
            files: Vec::new(),
        });
    region.id = LEGACY_REGION.to_string();
    region.files = vec![
        plan_region_file(&plan.pack, &base, FileRole::Basemap),
        plan_region_file(&plan.graph, &base, FileRole::Graph),
    ];
    region
}

fn plan_region_file(spec: &ArtifactSpec, base: &str, role: FileRole) -> RegionFile {
    RegionFile {
        role,
        transfer_name: spec.transfer_name.to_string(),
        installed_name: spec.name.to_string(),
        url: format!("{base}/{}", spec.transfer_name),
        transfer_bytes: spec.transfer_bytes,
        transfer_sha256: spec.transfer_sha256.clone(),
        installed_bytes: spec.expected_bytes,
        installed_sha256: spec.sha256.clone(),
        compression: spec.compression,
    }
}

fn validate_artifact(artifact: &Artifact, strict_urls: bool) -> Result<(), String> {
    if !valid_file_name(&artifact.installed_name) || !valid_file_name(&artifact.transfer_name) {
        return Err(format!(
            "unsupported artifact name {:?}",
            artifact.installed_name
        ));
    }
    let url_ok = if strict_urls {
        crate::region_catalog::valid_asset_url(&artifact.url)
    } else {
        crate::region_catalog::valid_local_asset_url(&artifact.url)
    };
    if !url_ok {
        return Err(format!(
            "refusing to download {:?} from a non-allowlisted URL",
            artifact.installed_name
        ));
    }
    if artifact.transfer_bytes == 0
        || artifact.installed_bytes == 0
        || artifact.transfer_bytes > crate::region_catalog::MAX_FILE_BYTES
        || artifact.installed_bytes > crate::region_catalog::MAX_FILE_BYTES
    {
        return Err(format!(
            "refusing to download {:?}: declared byte length is outside the 2 GiB cap",
            artifact.installed_name
        ));
    }
    if !valid_sha256(&artifact.transfer_sha256) {
        return Err(format!(
            "{:?} has no valid transfer SHA-256",
            artifact.installed_name
        ));
    }
    if !valid_sha256(&artifact.installed_sha256) {
        return Err(format!(
            "{:?} has no valid installed SHA-256",
            artifact.installed_name
        ));
    }
    Ok(())
}

fn settle_unstarted(state: &mut FileState) {
    if state.phase == Phase::Downloading || state.phase == Phase::Verifying {
        *state = FileState::idle();
    }
}

async fn write_installed(
    file: &mut tokio::fs::File,
    hasher: &mut Sha256,
    written: &mut u64,
    limit: u64,
    url: &str,
    bytes: &[u8],
) -> Result<(), String> {
    if written.saturating_add(bytes.len() as u64) > limit {
        return Err(format!(
            "{url} decompressed to more than the expected {limit} bytes"
        ));
    }
    file.write_all(bytes)
        .await
        .map_err(|error| format!("could not write the downloaded artifact: {error}"))?;
    hasher.update(bytes);
    *written += bytes.len() as u64;
    Ok(())
}

/// Optional gzip header fields (FEXTRA/FNAME/FCOMMENT) are capped so a
/// malformed stream cannot grow the header buffer without bound.
const MAX_GZIP_HEADER_BYTES: usize = 1024 * 1024;

/// Incremental RFC 1952 header parser. flate2's pure-Rust backend inflates
/// raw DEFLATE, so the gzip wrapper is stripped before the decoder sees it.
struct GzipHeaderParser {
    bytes: Vec<u8>,
}

impl GzipHeaderParser {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    /// Append one transfer chunk. `Some(offset)` means the header is complete
    /// and the DEFLATE payload starts at `offset` within [`Self::bytes`].
    fn push(&mut self, chunk: &[u8]) -> Result<Option<usize>, String> {
        self.bytes.extend_from_slice(chunk);
        match self.header_len()? {
            Some(offset) => Ok(Some(offset)),
            None if self.bytes.len() > MAX_GZIP_HEADER_BYTES => Err(format!(
                "gzip header is larger than {MAX_GZIP_HEADER_BYTES} bytes"
            )),
            None => Ok(None),
        }
    }

    fn header_len(&self) -> Result<Option<usize>, String> {
        let bytes = &self.bytes;
        if bytes.len() < 10 {
            return Ok(None);
        }
        if bytes[0] != 0x1f || bytes[1] != 0x8b {
            return Err("the transfer is not a gzip stream".to_string());
        }
        if bytes[2] != 8 {
            return Err(format!("unsupported gzip method {}", bytes[2]));
        }
        let flags = bytes[3];
        if flags & 0xe0 != 0 {
            return Err("gzip header has reserved flags set".to_string());
        }

        let mut offset = 10;
        if flags & 0x04 != 0 {
            if bytes.len() < offset + 2 {
                return Ok(None);
            }
            let extra = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
            offset += 2 + extra;
            if bytes.len() < offset {
                return Ok(None);
            }
        }
        for flag in [0x08_u8, 0x10] {
            if flags & flag != 0 {
                let Some(end) = bytes[offset..].iter().position(|byte| *byte == 0) else {
                    return Ok(None);
                };
                offset += end + 1;
            }
        }
        if flags & 0x02 != 0 {
            offset += 2;
            if bytes.len() < offset {
                return Ok(None);
            }
        }
        Ok(Some(offset))
    }
}

/// Only the published transfer names may be appended to a release URL.
#[cfg(test)]
fn valid_transfer_name(name: &str) -> bool {
    name == PACK_FILE || name == GRAPH_GZ_FILE
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= crate::region_catalog::MAX_REDIRECTS {
                attempt.error("too many release redirects")
            } else if crate::region_catalog::asset_redirect_allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("release redirect left the allowlisted hosts")
            }
        }))
        .user_agent(concat!(
            "Pinpoint/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/Prince-of-Nothing/Pinpoint)"
        ))
        .build()
        .map_err(|error| format!("could not create the download client: {error}"))
}

/// `PINPOINT_OFFLINE_BASE_URL`/plan override or the immutable release host.
/// Only an allowlisted release host or a loopback test/mirror URL is
/// accepted; anything else falls back to the pinned release.
fn resolve_base_url(plan_override: Option<&str>) -> String {
    resolve_base_url_with(plan_override, std::env::var(BASE_URL_ENV).ok().as_deref())
}

fn resolve_base_url_with(plan_override: Option<&str>, environment: Option<&str>) -> String {
    let raw = plan_override
        .map(str::to_string)
        .or_else(|| environment.map(str::to_string));
    match raw
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(base) => {
            let base = base.trim_end_matches('/');
            let probe = format!("{base}/{PACK_FILE}");
            if crate::region_catalog::valid_local_asset_url(&probe) {
                base.to_string()
            } else {
                DEFAULT_BASE_URL.to_string()
            }
        }
        None => DEFAULT_BASE_URL.to_string(),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

// ---------------------------------------------------------------------------
// Legacy migration helpers
// ---------------------------------------------------------------------------

fn path_under(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
}

fn copy_then_remove(source: &Path, target: &Path) -> std::io::Result<()> {
    let copied = std::fs::copy(source, target)?;
    let source_len = std::fs::metadata(source)?.len();
    let target_len = std::fs::metadata(target)?.len();
    if copied != source_len || target_len != source_len {
        return Err(std::io::Error::other("legacy pack copy is incomplete"));
    }
    std::fs::File::open(target)?.sync_all()?;
    std::fs::remove_file(source)
}

fn roll_back(root: &Path, region_dir: &Path, moved: &[(&str, FileRole)]) {
    for (name, _) in moved.iter().rev() {
        let target = region_dir.join(name);
        let source = root.join(name);
        if !path_under(root, &source) || !path_under(region_dir, &target) {
            continue;
        }
        if !source.exists() {
            if let Err(error) = std::fs::rename(&target, &source) {
                tracing::warn!(%error, %name, "could not roll back a partial legacy move");
            }
        }
    }
}

fn write_legacy_manifest(region_dir: &Path) {
    let bundled = RegionCatalog::bundled();
    let legacy = bundled.region(LEGACY_REGION);
    let mut files = Vec::new();
    for (name, role) in [
        (PACK_FILE, FileRole::Basemap),
        (GRAPH_FILE, FileRole::Graph),
    ] {
        let Ok(metadata) = std::fs::metadata(region_dir.join(name)) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        // shortcut: legacy files were installed by the hashing v2.3.0
        // installer; only the length is rechecked at adoption, hashing 204 MB
        // on a status poll is deferred until the sidecar lands.
        let sha256 = legacy
            .and_then(|region| region.files.iter().find(|file| file.installed_name == name))
            .filter(|file| file.installed_bytes == metadata.len())
            .map(|file| file.installed_sha256.clone())
            .unwrap_or_default();
        files.push(InstalledFile {
            role,
            name: name.to_string(),
            bytes: metadata.len(),
            sha256,
        });
    }
    let manifest = InstalledManifest {
        id: LEGACY_REGION.to_string(),
        version: "legacy".to_string(),
        installed_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        files,
    };
    let Ok(bytes) = serde_json::to_vec_pretty(&manifest) else {
        return;
    };
    let temp = region_dir.join(format!(".{INSTALLED_MANIFEST}.part"));
    if std::fs::write(&temp, bytes).is_err() {
        return;
    }
    if let Err(error) = std::fs::rename(&temp, region_dir.join(INSTALLED_MANIFEST)) {
        let _ = std::fs::remove_file(&temp);
        tracing::warn!(%error, "could not write the legacy installed.json");
    }
}

// ---------------------------------------------------------------------------
// Disk-space preflight
// ---------------------------------------------------------------------------

/// Required free space: the installed bytes plus 10%.
fn space_is_sufficient(available: Option<u64>, needed: u64) -> bool {
    match available {
        Some(available) => available >= needed.saturating_add(needed / 10),
        None => true,
    }
}

fn available_disk_bytes(path: &Path) -> Option<u64> {
    let existing = path
        .ancestors()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| Path::new("."));
    free_space_for(existing)
}

#[cfg(windows)]
fn free_space_for(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            directory: *const u16,
            free_bytes_available: *mut u64,
            total_bytes: *mut u64,
            total_free_bytes: *mut u64,
        ) -> i32;
    }

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut available = 0_u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(available)
}

#[cfg(unix)]
#[allow(clippy::useless_conversion)] // `f_bavail`/`f_frsize` are already u64 on 64-bit Unix.
fn free_space_for(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;

    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return None;
    }
    let stats = unsafe { stats.assume_init() };
    Some(u64::from(stats.f_bavail) * u64::from(stats.f_frsize))
}

#[cfg(not(any(windows, unix)))]
fn free_space_for(_path: &Path) -> Option<u64> {
    None
}

// ---------------------------------------------------------------------------
// Axum handlers
// ---------------------------------------------------------------------------

/// `GET /map/offline/status`
pub async fn status(Extension(installer): Extension<Arc<OfflineInstaller>>) -> Response {
    Json(installer.status().await).into_response()
}

/// `GET /map/catalog`
pub async fn catalog(Extension(installer): Extension<Arc<OfflineInstaller>>) -> Response {
    Json(installer.catalog_view().await).into_response()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadRequest {
    #[serde(default)]
    region: Option<String>,
}

/// `POST /map/offline/download`
pub async fn download(
    Extension(installer): Extension<Arc<OfflineInstaller>>,
    body: Bytes,
) -> Response {
    let region = match parse_download_region(&body) {
        Ok(region) => region,
        Err(message) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "INVALID_DOWNLOAD_REQUEST",
                &message,
            )
        }
    };
    installer.ensure_catalog().await;
    match installer.start_region(&region) {
        StartOutcome::Started => {
            (StatusCode::ACCEPTED, Json(installer.status().await)).into_response()
        }
        StartOutcome::AlreadyRunning => {
            (StatusCode::CONFLICT, Json(installer.status().await)).into_response()
        }
        StartOutcome::UnknownRegion(id) => error_response(
            StatusCode::NOT_FOUND,
            "UNKNOWN_REGION",
            &format!("unknown region {id:?}"),
        ),
        StartOutcome::Unavailable(message) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "OFFLINE_PACKS_UNAVAILABLE",
            &message,
        ),
    }
}

/// The page may send an empty body (Moldova, backwards compatible) or exactly
/// `{"region":"<id>"}`. URLs, names, sizes and hashes are never accepted.
fn parse_download_region(body: &[u8]) -> Result<String, String> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(LEGACY_REGION.to_string());
    }
    let request: DownloadRequest = serde_json::from_slice(body)
        .map_err(|error| format!("expected {{\"region\":\"<id>\"}}: {error}"))?;
    let region = request.region.unwrap_or_else(|| LEGACY_REGION.to_string());
    if !valid_region_id(&region) {
        return Err(format!("invalid region id {region:?}"));
    }
    Ok(region)
}

/// `GET /map/config`
pub async fn config(Extension(installer): Extension<Arc<OfflineInstaller>>) -> Response {
    Json(installer.config().await).into_response()
}

/// `GET|HEAD /packs/<region>/<name>`
pub async fn serve_region_pack(
    Extension(installer): Extension<Arc<OfflineInstaller>>,
    UrlPath((region, name)): UrlPath<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    serve_region_pack_named(&installer, &region, &name, &method, &headers).await
}

/// Shared implementation for servers that dispatch `/packs/...` themselves.
pub async fn serve_region_pack_named(
    installer: &OfflineInstaller,
    region: &str,
    name: &str,
    method: &Method,
    headers: &HeaderMap,
) -> Response {
    if !method_is_readable(method) {
        return method_not_allowed();
    }
    if !valid_region_id(region) || !valid_file_name(name) {
        return text_response(StatusCode::NOT_FOUND, "Not Found");
    }
    let Some(packs_dir) = installer.packs_dir() else {
        return text_response(StatusCode::NOT_FOUND, "No offline pack directory");
    };
    // Resolve the name through the catalog; a filesystem listing is never
    // exposed and unknown names inside a region stay closed.
    let catalog = installer.catalog().await;
    let Some(entry) = catalog.region(region) else {
        return text_response(StatusCode::NOT_FOUND, "Not Found");
    };
    if !entry.files.iter().any(|file| file.installed_name == name) {
        return text_response(StatusCode::NOT_FOUND, "Not Found");
    }
    let root = match tokio::fs::canonicalize(packs_dir).await {
        Ok(root) => root,
        Err(_) => return text_response(StatusCode::NOT_FOUND, "Not Found"),
    };
    let mut candidates = vec![root.join(region).join(name)];
    if region == LEGACY_REGION {
        candidates.push(root.join(name));
    }
    for candidate in candidates {
        let Ok(path) = tokio::fs::canonicalize(&candidate).await else {
            continue;
        };
        if !path.starts_with(&root) {
            continue;
        }
        let Ok(metadata) = tokio::fs::metadata(&path).await else {
            continue;
        };
        if metadata.is_file() {
            return serve_file(&path, metadata.len(), name, method, headers).await;
        }
    }
    text_response(StatusCode::NOT_FOUND, "Not Found")
}

/// `GET|HEAD /packs/<name>.pmtiles` (v2.3.0 route)
///
/// Only a single `*.pmtiles` file name is accepted. The final path is
/// canonicalised under the configured packs directory to reject symlinks and
/// traversal. Responses are streamed; a 93 MB archive is never buffered.
pub async fn serve_pack(
    Extension(installer): Extension<Arc<OfflineInstaller>>,
    UrlPath(name): UrlPath<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    serve_pack_named(&installer, &name, &method, &headers).await
}

/// Shared implementation used by the explicit route and by servers that
/// dispatch `/packs/...` from their own wildcard handler.
pub async fn serve_pack_named(
    installer: &OfflineInstaller,
    name: &str,
    method: &Method,
    headers: &HeaderMap,
) -> Response {
    if !method_is_readable(method) {
        return method_not_allowed();
    }
    if !valid_pack_name(name) {
        return text_response(StatusCode::NOT_FOUND, "Not Found");
    }
    let Some(packs_dir) = installer.packs_dir() else {
        return text_response(StatusCode::NOT_FOUND, "No offline pack directory");
    };

    let root = match tokio::fs::canonicalize(packs_dir).await {
        Ok(root) => root,
        Err(_) => return text_response(StatusCode::NOT_FOUND, "Not Found"),
    };
    let mut candidates = vec![root.join(name)];
    // Keep the v2.3.0 URL alive after the root file moved into `packs/moldova/`.
    if name == PACK_FILE {
        candidates.push(root.join(LEGACY_REGION).join(name));
    }
    for candidate in candidates {
        let Ok(path) = tokio::fs::canonicalize(&candidate).await else {
            continue;
        };
        if !path.starts_with(&root) {
            continue;
        }
        let Ok(metadata) = tokio::fs::metadata(&path).await else {
            continue;
        };
        if metadata.is_file() {
            return serve_file(&path, metadata.len(), name, method, headers).await;
        }
    }
    text_response(StatusCode::NOT_FOUND, "Not Found")
}

fn method_is_readable(method: &Method) -> bool {
    method == Method::GET || method == Method::HEAD
}

fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "GET, HEAD")],
        "Method Not Allowed",
    )
        .into_response()
}

async fn serve_file(
    path: &Path,
    size: u64,
    name: &str,
    method: &Method,
    headers: &HeaderMap,
) -> Response {
    let range = match headers.get(header::RANGE) {
        Some(value) => match value.to_str() {
            Ok(value) => Some(value),
            Err(_) => return range_not_satisfiable(size),
        },
        None => None,
    };

    match parse_range(range, size) {
        ByteRange::Unsatisfiable => range_not_satisfiable(size),
        ByteRange::Full => {
            if *method == Method::HEAD {
                return pack_response(StatusCode::OK, MIME_PMTILES, size, None, Body::empty());
            }
            match tokio::fs::File::open(path).await {
                Ok(file) => pack_response(
                    StatusCode::OK,
                    MIME_PMTILES,
                    size,
                    None,
                    Body::from_stream(ReaderStream::new(file)),
                ),
                Err(_) => text_response(StatusCode::NOT_FOUND, "Not Found"),
            }
        }
        ByteRange::Partial { start, end } => {
            let length = end - start + 1;
            if *method == Method::HEAD {
                return pack_response(
                    StatusCode::PARTIAL_CONTENT,
                    MIME_PMTILES,
                    length,
                    Some(format!("bytes {start}-{end}/{size}")),
                    Body::empty(),
                );
            }
            match tokio::fs::File::open(path).await {
                Ok(mut file) => {
                    if let Err(error) = file.seek(std::io::SeekFrom::Start(start)).await {
                        tracing::warn!(%error, pack = name, "map server: pack seek failed");
                        return text_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        );
                    }
                    pack_response(
                        StatusCode::PARTIAL_CONTENT,
                        MIME_PMTILES,
                        length,
                        Some(format!("bytes {start}-{end}/{size}")),
                        Body::from_stream(ReaderStream::new(file.take(length))),
                    )
                }
                Err(_) => text_response(StatusCode::NOT_FOUND, "Not Found"),
            }
        }
    }
}

fn valid_pack_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && name.ends_with(".pmtiles")
        && !name
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

enum ByteRange {
    Full,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

/// Parse one RFC 7233 byte range. Multipart ranges are rejected because
/// PMTiles only asks for a single contiguous range.
fn parse_range(raw: Option<&str>, size: u64) -> ByteRange {
    let Some(raw) = raw else {
        return ByteRange::Full;
    };
    let spec = raw.trim();
    let Some(spec) = spec
        .get(..6)
        .filter(|prefix| prefix.eq_ignore_ascii_case("bytes="))
        .map(|_| spec[6..].trim())
    else {
        return ByteRange::Unsatisfiable;
    };
    if size == 0 || spec.contains(',') {
        return ByteRange::Unsatisfiable;
    }
    let Some((start_raw, end_raw)) = spec.split_once('-') else {
        return ByteRange::Unsatisfiable;
    };
    if start_raw.is_empty() && end_raw.is_empty() {
        return ByteRange::Unsatisfiable;
    }
    let start_raw = start_raw.trim();
    let end_raw = end_raw.trim();

    if start_raw.is_empty() {
        let Ok(suffix) = end_raw.parse::<u64>() else {
            return ByteRange::Unsatisfiable;
        };
        if suffix == 0 {
            return ByteRange::Unsatisfiable;
        }
        return ByteRange::Partial {
            start: size.saturating_sub(suffix),
            end: size - 1,
        };
    }
    let Ok(start) = start_raw.parse::<u64>() else {
        return ByteRange::Unsatisfiable;
    };
    if start >= size {
        return ByteRange::Unsatisfiable;
    }
    let end = if end_raw.is_empty() {
        size - 1
    } else {
        match end_raw.parse::<u64>() {
            Ok(end) if end >= start => end.min(size - 1),
            _ => return ByteRange::Unsatisfiable,
        }
    };
    ByteRange::Partial { start, end }
}

fn pack_response(
    status: StatusCode,
    mime: &'static str,
    content_length: u64,
    content_range: Option<String>,
    body: Body,
) -> Response {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static(mime));
    headers.insert(
        header::ACCEPT_RANGES,
        header::HeaderValue::from_static("bytes"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-cache"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    if let Ok(value) = header::HeaderValue::from_str(&content_length.to_string()) {
        headers.insert(header::CONTENT_LENGTH, value);
    }
    if let Some(content_range) = content_range {
        if let Ok(value) = header::HeaderValue::from_str(&content_range) {
            headers.insert(header::CONTENT_RANGE, value);
        }
    }
    response
}

fn range_not_satisfiable(size: u64) -> Response {
    let mut response = text_response(StatusCode::RANGE_NOT_SATISFIABLE, "Range Not Satisfiable");
    if let Ok(value) = header::HeaderValue::from_str(&format!("bytes */{size}")) {
        response.headers_mut().insert(header::CONTENT_RANGE, value);
    }
    response
}

fn text_response(status: StatusCode, message: &'static str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, MIME_TEXT)],
        Body::from(message),
    )
        .into_response()
}

fn error_response(status: StatusCode, code: &'static str, message: &str) -> Response {
    let body = serde_json::json!({"error": code, "message": message});
    (status, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get, post};
    use axum::Router;
    use tokio::sync::Notify;
    use tower::ServiceExt;

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use std::io::Write as _;

        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).expect("gzip fixture");
        encoder.finish().expect("gzip fixture")
    }

    /// Local stand-in for the GitHub release. The graph is published gzip
    /// compressed, exactly like the real release asset. Bodies stream in
    /// small chunks so status polling observes real progress. An optional
    /// hold pauses the graph after its first chunk until the test releases
    /// it, making a partial transfer observable on any scheduler.
    async fn fixture_server(
        pack: Vec<u8>,
        graph_gz: Vec<u8>,
        chunk_delay: Duration,
        graph_hold: Option<Arc<Notify>>,
    ) -> String {
        async fn artifact(
            UrlPath(name): UrlPath<String>,
            axum::extract::State(state): axum::extract::State<Arc<FixtureArtifacts>>,
        ) -> Response {
            let bytes = match name.as_str() {
                PACK_FILE => state.pack.clone(),
                GRAPH_GZ_FILE => state.graph_gz.clone(),
                _ => return text_response(StatusCode::NOT_FOUND, "Not Found"),
            };
            let total = bytes.len() as u64;
            let delay = state.delay;
            let hold = state.graph_hold.clone().filter(|_| name == GRAPH_GZ_FILE);
            let (mut writer, reader) = tokio::io::duplex(8 * 1024);
            tokio::spawn(async move {
                for (index, chunk) in bytes.chunks(1024).enumerate() {
                    if writer.write_all(chunk).await.is_err() {
                        return;
                    }
                    if index == 0 {
                        if let Some(hold) = hold.as_ref() {
                            hold.notified().await;
                        }
                    }
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                }
            });
            pack_response(
                StatusCode::OK,
                MIME_PMTILES,
                total,
                None,
                Body::from_stream(ReaderStream::new(reader)),
            )
        }

        struct FixtureArtifacts {
            pack: Vec<u8>,
            graph_gz: Vec<u8>,
            delay: Duration,
            graph_hold: Option<Arc<Notify>>,
        }

        let state = Arc::new(FixtureArtifacts {
            pack,
            graph_gz,
            delay: chunk_delay,
            graph_hold,
        });
        let app = Router::new()
            .route("/:file", get(artifact))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture listener");
        let address = listener.local_addr().expect("fixture address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{address}")
    }

    struct Fixture {
        base: String,
        pack: Vec<u8>,
        graph: Vec<u8>,
        graph_gz: Vec<u8>,
    }

    impl Fixture {
        async fn new(pack: Vec<u8>, graph: Vec<u8>, chunk_delay: Duration) -> Self {
            Self::with_graph_hold(pack, graph, chunk_delay, None).await
        }

        async fn with_graph_hold(
            pack: Vec<u8>,
            graph: Vec<u8>,
            chunk_delay: Duration,
            graph_hold: Option<Arc<Notify>>,
        ) -> Self {
            let graph_gz = gzip(&graph);
            let base =
                fixture_server(pack.clone(), graph_gz.clone(), chunk_delay, graph_hold).await;
            Self {
                base,
                pack,
                graph,
                graph_gz,
            }
        }

        fn plan(&self) -> DownloadPlan {
            DownloadPlan::fixture_gzip(&self.base, &self.pack, &self.graph_gz, &self.graph)
        }
    }

    /// Local stand-in for `pinpoint-maps` with one URL path per region file.
    async fn region_fixture_server(files: Vec<(String, Vec<u8>)>) -> String {
        async fn artifact(
            UrlPath((region, name)): UrlPath<(String, String)>,
            axum::extract::State(state): axum::extract::State<
                Arc<std::collections::HashMap<String, Vec<u8>>>,
            >,
        ) -> Response {
            let key = format!("{region}/{name}");
            let Some(bytes) = state.get(&key).cloned() else {
                return text_response(StatusCode::NOT_FOUND, "Not Found");
            };
            let total = bytes.len() as u64;
            let (mut writer, reader) = tokio::io::duplex(8 * 1024);
            tokio::spawn(async move {
                for chunk in bytes.chunks(1024) {
                    if writer.write_all(chunk).await.is_err() {
                        return;
                    }
                }
            });
            pack_response(
                StatusCode::OK,
                MIME_PMTILES,
                total,
                None,
                Body::from_stream(ReaderStream::new(reader)),
            )
        }

        let state = Arc::new(files.into_iter().collect());
        let app = Router::new()
            .route("/:region/:file", get(artifact))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("region fixture listener");
        let address = listener.local_addr().expect("region fixture address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{address}")
    }

    fn fixture_region(
        base: &str,
        id: &str,
        name: &str,
        pack: &[u8],
        graph_gz: &[u8],
        graph: &[u8],
    ) -> Region {
        Region {
            id: id.to_string(),
            name: name.to_string(),
            bbox: [0.0, 0.0, 1.0, 1.0],
            maxzoom: 14,
            version: "2026-11-17".to_string(),
            files: vec![
                RegionFile {
                    role: FileRole::Basemap,
                    transfer_name: format!("{id}.pmtiles"),
                    installed_name: format!("{id}.pmtiles"),
                    url: format!("{base}/{id}/{id}.pmtiles"),
                    transfer_bytes: pack.len() as u64,
                    transfer_sha256: hex(&Sha256::digest(pack)),
                    installed_bytes: pack.len() as u64,
                    installed_sha256: hex(&Sha256::digest(pack)),
                    compression: Compression::Identity,
                },
                RegionFile {
                    role: FileRole::Graph,
                    transfer_name: format!("{id}.graph.bin.gz"),
                    installed_name: format!("{id}.graph.bin"),
                    url: format!("{base}/{id}/{id}.graph.bin.gz"),
                    transfer_bytes: graph_gz.len() as u64,
                    transfer_sha256: hex(&Sha256::digest(graph_gz)),
                    installed_bytes: graph.len() as u64,
                    installed_sha256: hex(&Sha256::digest(graph)),
                    compression: Compression::Gzip,
                },
            ],
        }
    }

    fn fixture_catalog(regions: Vec<Region>) -> RegionCatalog {
        RegionCatalog {
            schema: crate::region_catalog::CATALOG_SCHEMA,
            updated: "2026-11-17T12:00:00Z".to_string(),
            regions,
        }
    }

    async fn wait_finished(installer: &Arc<OfflineInstaller>) -> OfflineStatus {
        for _ in 0..600 {
            let status = installer.status().await;
            let running = matches!(status.pack.phase, Phase::Downloading | Phase::Verifying)
                || matches!(status.graph.phase, Phase::Downloading | Phase::Verifying);
            if !running {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("offline install did not finish in time");
    }

    #[tokio::test]
    async fn installs_the_gzip_graph_with_verified_bytes_and_no_temp_files() {
        let pack = b"PMTiles fixture pack".to_vec();
        let graph = b"synthetic graph bytes".to_vec();
        let fixture = Fixture::new(pack.clone(), graph.clone(), Duration::from_millis(2)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture.plan(),
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;

        assert!(status.pack.installed, "{status:?}");
        assert!(status.graph.installed, "{status:?}");
        assert_eq!(status.region.as_deref(), Some(LEGACY_REGION));
        assert_eq!(status.pack.bytes, pack.len() as u64);
        assert_eq!(status.graph.bytes, graph.len() as u64);
        assert_eq!(status.pack.phase, Phase::Installed);
        assert_eq!(status.graph.phase, Phase::Installed);
        assert_eq!(status.pack.progress_bytes, pack.len() as u64);
        assert_eq!(status.graph.progress_bytes, graph.len() as u64);
        assert_eq!(status.pack.downloaded_bytes, pack.len() as u64);
        assert_eq!(status.graph.downloaded_bytes, fixture.graph_gz.len() as u64);
        assert_eq!(status.pack.download_bytes, pack.len() as u64);
        assert_eq!(status.graph.download_bytes, fixture.graph_gz.len() as u64);
        assert_eq!(
            status.expected_download_bytes,
            pack.len() as u64 + fixture.graph_gz.len() as u64
        );
        assert_eq!(status.pack.error, None);
        assert_eq!(status.graph.error, None);
        assert_eq!(
            std::fs::read(dir.path().join(PACK_FILE)).unwrap(),
            pack,
            "installed pack bytes"
        );
        assert_eq!(
            std::fs::read(dir.path().join(GRAPH_FILE)).unwrap(),
            graph,
            "installed graph bytes are decompressed"
        );
        assert!(
            !dir.path().join(GRAPH_GZ_FILE).exists(),
            "the compressed transfer is not installed"
        );
        assert!(!dir.path().join(format!(".{PACK_FILE}.part")).exists());
        assert!(!dir.path().join(format!(".{GRAPH_FILE}.part")).exists());
    }

    #[tokio::test]
    async fn wrong_hash_is_rejected_and_never_replaces_an_installed_file() {
        let pack = b"new pack bytes".to_vec();
        let graph = b"graph".to_vec();
        let fixture = Fixture::new(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let old = b"old verified pack".to_vec();
        std::fs::write(dir.path().join(PACK_FILE), &old).unwrap();

        let mut plan = fixture.plan();
        plan.pack.sha256 = "00".repeat(32);
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            plan,
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;

        assert_eq!(status.pack.phase, Phase::Failed);
        assert!(!status.pack.installed);
        assert!(status
            .pack
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("SHA-256"));
        assert_eq!(
            std::fs::read(dir.path().join(PACK_FILE)).unwrap(),
            old,
            "a failed download must leave the old pack untouched"
        );
        assert!(!dir.path().join(format!(".{PACK_FILE}.part")).exists());
        // The job stops at the first failure; the graph is not attempted.
        assert!(!status.graph.installed);
        assert_eq!(status.graph.phase, Phase::Idle);
        assert!(!dir.path().join(GRAPH_FILE).exists());
    }

    #[tokio::test]
    async fn a_wrong_compressed_hash_is_rejected_and_keeps_the_old_graph() {
        let pack = b"pack bytes".to_vec();
        let graph = b"synthetic graph for the compressed-hash check".to_vec();
        let fixture = Fixture::new(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let old = b"old verified graph".to_vec();
        std::fs::write(dir.path().join(GRAPH_FILE), &old).unwrap();

        let mut plan = fixture.plan();
        plan.graph.transfer_sha256 = "00".repeat(32);
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            plan,
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;

        assert!(status.pack.installed, "{status:?}");
        assert_eq!(status.graph.phase, Phase::Failed);
        assert!(!status.graph.installed);
        assert!(
            status
                .graph
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("transfer SHA-256"),
            "error must name the compressed hash: {:?}",
            status.graph.error
        );
        assert_eq!(
            std::fs::read(dir.path().join(GRAPH_FILE)).unwrap(),
            old,
            "a failed compressed transfer must leave the old graph untouched"
        );
        assert!(!dir.path().join(format!(".{GRAPH_FILE}.part")).exists());
    }

    #[tokio::test]
    async fn a_wrong_uncompressed_hash_is_rejected_and_keeps_the_old_graph() {
        let pack = b"pack bytes".to_vec();
        let graph = b"synthetic graph for the decompressed-hash check".to_vec();
        let fixture = Fixture::new(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let old = b"old verified graph".to_vec();
        std::fs::write(dir.path().join(GRAPH_FILE), &old).unwrap();

        let mut plan = fixture.plan();
        plan.graph.sha256 = "00".repeat(32);
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            plan,
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;

        assert!(status.pack.installed, "{status:?}");
        assert_eq!(status.graph.phase, Phase::Failed);
        assert!(!status.graph.installed);
        assert!(
            status
                .graph
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("installed SHA-256"),
            "error must name the decompressed hash: {:?}",
            status.graph.error
        );
        assert_eq!(
            std::fs::read(dir.path().join(GRAPH_FILE)).unwrap(),
            old,
            "a wrong decompressed hash must leave the old graph untouched"
        );
        assert!(!dir.path().join(format!(".{GRAPH_FILE}.part")).exists());
    }

    #[tokio::test]
    async fn a_truncated_gzip_stream_is_rejected_and_keeps_the_old_graph() {
        let pack = b"pack bytes".to_vec();
        let graph = vec![5_u8; 64 * 1024];
        let graph_gz = gzip(&graph);
        let truncated = graph_gz[..graph_gz.len() - 8].to_vec();
        let base = fixture_server(pack.clone(), truncated.clone(), Duration::ZERO, None).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let old = b"old verified graph".to_vec();
        std::fs::write(dir.path().join(GRAPH_FILE), &old).unwrap();

        // The plan matches the truncated transfer, so only the gzip stream and
        // the uncompressed length can reject it.
        let plan = DownloadPlan::fixture_gzip(&base, &pack, &truncated, &graph);
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            plan,
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;

        assert!(status.pack.installed, "{status:?}");
        assert_eq!(status.graph.phase, Phase::Failed);
        assert!(!status.graph.installed);
        assert!(
            status
                .graph
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("gzip"),
            "error must name the truncated gzip stream: {:?}",
            status.graph.error
        );
        assert_eq!(
            std::fs::read(dir.path().join(GRAPH_FILE)).unwrap(),
            old,
            "a truncated gzip stream must leave the old graph untouched"
        );
        assert!(!dir.path().join(format!(".{GRAPH_FILE}.part")).exists());
    }

    #[tokio::test]
    async fn a_verified_redownload_replaces_the_old_file() {
        let pack = b"replacement pack".to_vec();
        let graph = b"replacement graph".to_vec();
        let fixture = Fixture::new(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(PACK_FILE), b"old pack").unwrap();
        std::fs::write(dir.path().join(GRAPH_FILE), b"old graph").unwrap();
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture.plan(),
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;
        assert!(status.pack.installed && status.graph.installed);
        assert_eq!(std::fs::read(dir.path().join(PACK_FILE)).unwrap(), pack);
        assert_eq!(std::fs::read(dir.path().join(GRAPH_FILE)).unwrap(), graph);
    }

    #[tokio::test]
    async fn a_short_artifact_is_rejected_and_keeps_the_old_file() {
        let pack = b"short".to_vec();
        let graph = b"graph".to_vec();
        let fixture = Fixture::new(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(PACK_FILE), b"old pack").unwrap();
        let mut plan = fixture.plan();
        // Ask for more bytes than the fixture host declares and sends.
        plan.pack.expected_bytes = pack.len() as u64 + 100;
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            plan,
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;
        assert_eq!(status.pack.phase, Phase::Failed);
        assert!(status
            .pack
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("expected"));
        assert_eq!(
            std::fs::read(dir.path().join(PACK_FILE)).unwrap(),
            b"old pack"
        );
    }

    #[tokio::test]
    async fn status_reports_downloading_progress_then_installed() {
        let pack = vec![7_u8; 24 * 1024];
        // Pseudo-random bytes gzip into several chunks, and the fixture holds
        // the graph after its first chunk until the poller has seen progress.
        let graph = (0..64 * 1024_u32)
            .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect::<Vec<_>>();
        let graph_hold = Arc::new(Notify::new());
        let fixture = Fixture::with_graph_hold(
            pack.clone(),
            graph.clone(),
            Duration::from_millis(12),
            Some(Arc::clone(&graph_hold)),
        )
        .await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture.plan(),
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let mut saw_downloading = false;
        let mut saw_progress = false;
        let mut saw_graph_transfer_progress = false;
        for _ in 0..500 {
            let status = installer.status().await;
            if status.pack.phase == Phase::Downloading {
                saw_downloading = true;
            }
            if status.pack.progress_bytes > 0 && status.pack.progress_bytes < pack.len() as u64 {
                saw_progress = true;
            }
            if status.graph.downloaded_bytes > 0
                && status.graph.downloaded_bytes < fixture.graph_gz.len() as u64
            {
                saw_graph_transfer_progress = true;
                break;
            }
            if status.pack.installed && status.graph.installed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        graph_hold.notify_one();
        let status = wait_finished(&installer).await;

        assert!(saw_downloading, "status must report the downloading phase");
        assert!(saw_progress, "status must report partial progress bytes");
        assert!(
            saw_graph_transfer_progress,
            "status must report compressed transfer progress for the graph"
        );
        assert!(status.pack.installed);
        assert!(status.graph.installed);
        assert_eq!(
            status.expected_download_bytes,
            pack.len() as u64 + fixture.graph_gz.len() as u64
        );
        assert_eq!(status.graph.downloaded_bytes, status.graph.download_bytes);
    }

    #[tokio::test]
    async fn a_second_start_while_running_is_rejected() {
        let pack = vec![1_u8; 16 * 1024];
        let graph = vec![2_u8; 4 * 1024];
        let fixture = Fixture::new(pack, graph, Duration::from_millis(10)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture.plan(),
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        assert_eq!(installer.start(), StartOutcome::AlreadyRunning);
        let status = wait_finished(&installer).await;
        assert!(status.pack.installed && status.graph.installed);
    }

    #[tokio::test]
    async fn download_route_returns_202_then_409() {
        let pack = vec![3_u8; 16 * 1024];
        let graph = vec![4_u8; 4 * 1024];
        let fixture = Fixture::new(pack, graph, Duration::from_millis(10)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture.plan(),
        ));
        let app = Router::new()
            .route("/map/offline/download", post(download))
            .route("/map/offline/status", get(status))
            .layer(Extension(Arc::clone(&installer)));
        let post = || {
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/map/offline/download")
                .body(Body::empty())
                .unwrap()
        };

        let first = app.clone().oneshot(post()).await.unwrap();
        assert_eq!(first.status(), StatusCode::ACCEPTED);
        let second = app.oneshot(post()).await.unwrap();
        assert_eq!(second.status(), StatusCode::CONFLICT);
        let status = wait_finished(&installer).await;
        assert!(status.pack.installed && status.graph.installed);
    }

    #[tokio::test]
    async fn download_takes_a_region_id_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let plan = DownloadPlan {
            base_url: Some("http://127.0.0.1:1".to_string()),
            ..DownloadPlan::default()
        };
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            plan,
        ));
        let app = Router::new()
            .route("/map/offline/download", post(download))
            .layer(Extension(Arc::clone(&installer)));
        let request = |body: &'static str| {
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/map/offline/download")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap()
        };

        let extra = app
            .clone()
            .oneshot(request(
                r#"{"region":"moldova","url":"http://evil.example/x.pmtiles"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(extra.status(), StatusCode::BAD_REQUEST);

        let unknown = app
            .clone()
            .oneshot(request(r#"{"region":"atlantis"}"#))
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);

        let invalid = app
            .clone()
            .oneshot(request(r#"{"region":"../moldova"}"#))
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn config_reports_regions_and_uses_the_fixture_until_installed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_catalog(
            Some(dir.path().to_path_buf()),
            RegionCatalog::bundled(),
            LEGACY_REGION,
        ));

        let config = installer.config().await;
        assert_eq!(config.pmtiles_url, FIXTURE_URL);
        assert!(!config.pack_installed);
        assert!(!config.graph_installed);
        assert!(config.regions.is_empty());

        // A file with the production length is treated as installed (the hash
        // was verified when it was installed). A short file is not.
        let region_dir = dir.path().join(LEGACY_REGION);
        std::fs::create_dir_all(&region_dir).unwrap();
        std::fs::File::create(region_dir.join(PACK_FILE))
            .unwrap()
            .set_len(PACK_BYTES)
            .unwrap();
        std::fs::write(region_dir.join(GRAPH_FILE), b"short").unwrap();
        let config = installer.config().await;
        assert_eq!(config.pmtiles_url, format!("/packs/{PACK_FILE}"));
        assert!(config.pack_installed);
        assert!(!config.graph_installed);
        assert_eq!(config.regions.len(), 1);
        assert!(!config.regions[0].installed);
        assert!(config.regions[0].graph_url.is_none());

        std::fs::File::create(region_dir.join(GRAPH_FILE))
            .unwrap()
            .set_len(GRAPH_BYTES)
            .unwrap();
        let config = installer.config().await;
        assert!(config.graph_installed);
        assert!(config.regions[0].installed);
        assert_eq!(
            config.regions[0].graph_url,
            Some(format!("/packs/{LEGACY_REGION}/{GRAPH_FILE}"))
        );
    }

    #[tokio::test]
    async fn serves_pack_ranges_and_rejects_unsatisfiable_and_traversal_paths() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(PACK_FILE), b"0123456789").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"not a pack").unwrap();
        let installer = Arc::new(OfflineInstaller::new(Some(dir.path().to_path_buf())));
        let app = Router::new()
            .route("/packs/:name", get(serve_pack))
            .layer(Extension(installer));

        let get_req = |uri: &str, range: Option<&str>, method: Method| {
            let mut builder = axum::http::Request::builder().method(method).uri(uri);
            if let Some(range) = range {
                builder = builder.header(header::RANGE, range);
            }
            builder.body(Body::empty()).unwrap()
        };

        let full = app
            .clone()
            .oneshot(get_req(&format!("/packs/{PACK_FILE}"), None, Method::GET))
            .await
            .unwrap();
        assert_eq!(full.status(), StatusCode::OK);
        assert_eq!(full.headers()[header::ACCEPT_RANGES], "bytes");
        let body = axum::body::to_bytes(full.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), b"0123456789");

        let partial = app
            .clone()
            .oneshot(get_req(
                &format!("/packs/{PACK_FILE}"),
                Some("bytes=2-5"),
                Method::GET,
            ))
            .await
            .unwrap();
        assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(partial.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
        assert_eq!(partial.headers()[header::CONTENT_LENGTH], "4");
        let body = axum::body::to_bytes(partial.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), b"2345");

        let suffix = app
            .clone()
            .oneshot(get_req(
                &format!("/packs/{PACK_FILE}"),
                Some("bytes=-4"),
                Method::GET,
            ))
            .await
            .unwrap();
        assert_eq!(suffix.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(suffix.headers()[header::CONTENT_RANGE], "bytes 6-9/10");

        for range in ["bytes=10-", "bytes=-0", "bytes=0-1,3-4", "items=0-1"] {
            let response = app
                .clone()
                .oneshot(get_req(
                    &format!("/packs/{PACK_FILE}"),
                    Some(range),
                    Method::GET,
                ))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::RANGE_NOT_SATISFIABLE,
                "range {range}"
            );
            assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */10");
        }

        let head = app
            .clone()
            .oneshot(get_req(
                &format!("/packs/{PACK_FILE}"),
                Some("bytes=0-3"),
                Method::HEAD,
            ))
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::PARTIAL_CONTENT);
        assert!(axum::body::to_bytes(head.into_body(), usize::MAX)
            .await
            .unwrap()
            .is_empty());

        for uri in [
            "/packs/notes.txt",
            "/packs/..%2FCargo.toml",
            "/packs/%2e%2e%2fCargo.toml",
        ] {
            let response = app
                .clone()
                .oneshot(get_req(uri, None, Method::GET))
                .await
                .unwrap();
            assert!(
                response.status() == StatusCode::NOT_FOUND
                    || response.status() == StatusCode::BAD_REQUEST,
                "{uri} should be closed, got {}",
                response.status()
            );
        }
    }

    #[test]
    fn base_url_resolution_prefers_the_override_and_rejects_untrusted_hosts() {
        assert_eq!(
            resolve_base_url(Some("http://127.0.0.1:9000/packs/")),
            "http://127.0.0.1:9000/packs"
        );
        assert_eq!(
            resolve_base_url(Some(
                "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/x"
            )),
            "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/x"
        );
        assert_eq!(
            resolve_base_url(Some("https://mirror.example/moldova")),
            DEFAULT_BASE_URL,
            "a foreign host is not an allowed release host"
        );
        assert_eq!(
            resolve_base_url_with(None, Some("https://evil.example/release/")),
            DEFAULT_BASE_URL
        );
        assert_eq!(
            resolve_base_url_with(Some("   "), Some("https://github.com/example/release")),
            DEFAULT_BASE_URL,
            "an empty plan override falls back instead of the environment"
        );
        assert_eq!(
            resolve_base_url_with(None, Some("ftp://example/x")),
            DEFAULT_BASE_URL
        );
    }

    #[test]
    fn plan_keeps_only_the_published_file_names() {
        let plan = DownloadPlan::default();
        assert_eq!(plan.pack.name, PACK_FILE);
        assert_eq!(plan.graph.name, GRAPH_FILE);
        assert_eq!(plan.pack.transfer_name, PACK_FILE);
        assert_eq!(plan.graph.transfer_name, GRAPH_GZ_FILE);
        let fixture =
            DownloadPlan::fixture_gzip("http://127.0.0.1:1", b"pack", b"gzip graph", b"graph");
        assert_eq!(fixture.for_file(PACK_FILE).expected_bytes, 4);
        assert_eq!(fixture.for_file(GRAPH_FILE).expected_bytes, 5);
        assert_eq!(fixture.for_file(GRAPH_FILE).transfer_name, GRAPH_GZ_FILE);
        assert!(valid_transfer_name(PACK_FILE));
        assert!(valid_transfer_name(GRAPH_GZ_FILE));
        assert!(!valid_transfer_name(GRAPH_FILE));
        assert!(!valid_pack_name("../")); // sanity: the scorer cannot join paths
        assert!(valid_pack_name(PACK_FILE));
        assert!(!valid_pack_name("notes.txt"));
        assert!(!valid_pack_name(".hidden.pmtiles"));
        assert!(!valid_pack_name("nested/pack.pmtiles"));
    }

    #[test]
    fn production_plan_transfers_139_mb_and_installs_204_mb() {
        let plan = DownloadPlan::default();
        assert_eq!(plan.graph.compression, Compression::Gzip);
        assert_eq!(plan.pack.compression, Compression::Identity);
        assert_eq!(plan.graph.transfer_bytes, 45_338_586);
        assert_eq!(plan.graph.transfer_sha256, GRAPH_GZ_SHA256);
        assert_eq!(plan.graph.expected_bytes, 110_858_815);
        assert_eq!(plan.graph.sha256, GRAPH_SHA256);
        assert_eq!(plan.expected_download_bytes(), 138_510_985);
        assert_eq!(plan.expected_installed_bytes(), 204_031_214);
        assert_eq!(DOWNLOAD_BYTES, 138_510_985);
        assert_eq!(INSTALLED_BYTES, 204_031_214);
    }

    #[test]
    fn a_plan_with_a_missing_hash_never_starts() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut plan = DownloadPlan::default();
        plan.pack.transfer_sha256.clear();
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            plan,
        ));
        match installer.start() {
            StartOutcome::Unavailable(message) => {
                assert!(message.contains("SHA-256"), "{message}");
            }
            other => panic!("a missing hash must be refused, got {other:?}"),
        }
    }

    #[test]
    fn space_preflight_requires_the_install_plus_ten_percent() {
        assert!(space_is_sufficient(None, u64::MAX));
        assert!(space_is_sufficient(Some(110), 100));
        assert!(!space_is_sufficient(Some(109), 100));
        assert!(!space_is_sufficient(
            Some(1024),
            crate::region_catalog::MAX_FILE_BYTES
        ));
    }

    #[tokio::test]
    async fn installs_two_regions_into_separate_directories() {
        let dir = tempfile::tempdir().expect("temp dir");
        let moldova_pack = b"moldova basemap fixture".to_vec();
        let moldova_graph = b"moldova graph fixture".to_vec();
        let romania_pack = b"romania basemap fixture, different".to_vec();
        let romania_graph = b"romania graph fixture, different".to_vec();
        let moldova_gz = gzip(&moldova_graph);
        let romania_gz = gzip(&romania_graph);
        let base = region_fixture_server(vec![
            ("moldova/moldova.pmtiles".to_string(), moldova_pack.clone()),
            (
                "moldova/moldova.graph.bin.gz".to_string(),
                moldova_gz.clone(),
            ),
            ("romania/romania.pmtiles".to_string(), romania_pack.clone()),
            (
                "romania/romania.graph.bin.gz".to_string(),
                romania_gz.clone(),
            ),
        ])
        .await;
        let catalog = fixture_catalog(vec![
            fixture_region(
                &base,
                "moldova",
                "Moldova",
                &moldova_pack,
                &moldova_gz,
                &moldova_graph,
            ),
            fixture_region(
                &base,
                "romania",
                "Romania",
                &romania_pack,
                &romania_gz,
                &romania_graph,
            ),
        ]);
        let installer = Arc::new(OfflineInstaller::with_catalog(
            Some(dir.path().to_path_buf()),
            catalog,
            "moldova",
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;
        assert_eq!(status.region.as_deref(), Some("moldova"));
        assert!(
            status.pack.installed && status.graph.installed,
            "{status:?}"
        );
        assert_eq!(
            std::fs::read(dir.path().join("moldova/moldova.pmtiles")).unwrap(),
            moldova_pack
        );
        assert_eq!(
            std::fs::read(dir.path().join("moldova/moldova.graph.bin")).unwrap(),
            moldova_graph
        );
        assert!(!dir.path().join("moldova/moldova.graph.bin.gz").exists());
        assert!(!dir.path().join("moldova/.moldova.pmtiles.part").exists());
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("moldova/installed.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["id"], "moldova");
        assert_eq!(manifest["version"], "2026-11-17");
        assert_eq!(manifest["files"][1]["role"], "graph");

        assert_eq!(installer.start_region("romania"), StartOutcome::Started);
        let status = wait_finished(&installer).await;
        assert_eq!(status.region.as_deref(), Some("romania"));
        assert!(
            status.pack.installed && status.graph.installed,
            "{status:?}"
        );
        assert_eq!(
            std::fs::read(dir.path().join("romania/romania.pmtiles")).unwrap(),
            romania_pack
        );
        assert!(
            dir.path().join("moldova/moldova.pmtiles").is_file(),
            "installing another region keeps Moldova"
        );

        let config = installer.config().await;
        assert_eq!(config.pmtiles_url, "/packs/romania/romania.pmtiles");
        assert_eq!(config.regions.len(), 2);
        assert!(config.regions.iter().all(|region| region.installed));

        let view = installer.catalog_view().await;
        assert!(view.regions.iter().all(|region| region.installed));
        assert_eq!(
            view.regions[0].installed_version.as_deref(),
            Some("2026-11-17")
        );
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("https://"), "catalog view leaks asset URLs");
        assert!(!json.contains("http://"), "catalog view leaks asset URLs");
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        let file = &json["regions"][0]["files"][0];
        assert_eq!(file["role"], "basemap");
        assert_eq!(file["transfer_bytes"], moldova_pack.len());
        assert!(file.get("url").is_none());
        assert!(file.get("transfer_sha256").is_none());
    }

    #[tokio::test]
    async fn a_catalog_hash_mismatch_keeps_the_old_region_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let pack = b"new catalog pack".to_vec();
        let graph = b"new catalog graph".to_vec();
        let graph_gz = gzip(&graph);
        let base = region_fixture_server(vec![
            ("moldova/moldova.pmtiles".to_string(), pack.clone()),
            ("moldova/moldova.graph.bin.gz".to_string(), graph_gz.clone()),
        ])
        .await;
        let mut catalog = fixture_catalog(vec![fixture_region(
            &base, "moldova", "Moldova", &pack, &graph_gz, &graph,
        )]);
        catalog.regions[0].files[0].installed_sha256 = "00".repeat(32);
        let region_dir = dir.path().join("moldova");
        std::fs::create_dir_all(&region_dir).unwrap();
        std::fs::write(region_dir.join("moldova.pmtiles"), b"old verified pack").unwrap();
        std::fs::write(region_dir.join("moldova.graph.bin"), b"old verified graph").unwrap();

        let installer = Arc::new(OfflineInstaller::with_catalog(
            Some(dir.path().to_path_buf()),
            catalog,
            "moldova",
        ));
        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;

        assert_eq!(status.pack.phase, Phase::Failed);
        assert_eq!(
            std::fs::read(region_dir.join("moldova.pmtiles")).unwrap(),
            b"old verified pack"
        );
        assert_eq!(
            std::fs::read(region_dir.join("moldova.graph.bin")).unwrap(),
            b"old verified graph"
        );
        assert!(!region_dir.join(".moldova.pmtiles.part").exists());
    }

    #[tokio::test]
    async fn migrates_legacy_root_files_and_writes_a_manifest() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::File::create(dir.path().join(PACK_FILE))
            .unwrap()
            .set_len(PACK_BYTES)
            .unwrap();
        std::fs::File::create(dir.path().join(GRAPH_FILE))
            .unwrap()
            .set_len(GRAPH_BYTES)
            .unwrap();
        let installer = Arc::new(OfflineInstaller::with_catalog(
            Some(dir.path().to_path_buf()),
            RegionCatalog::bundled(),
            LEGACY_REGION,
        ));

        let status = installer.status().await;
        assert!(
            status.pack.installed && status.graph.installed,
            "{status:?}"
        );
        assert!(!dir.path().join(PACK_FILE).exists());
        assert!(!dir.path().join(GRAPH_FILE).exists());
        assert_eq!(
            std::fs::metadata(dir.path().join("moldova").join(PACK_FILE))
                .unwrap()
                .len(),
            PACK_BYTES
        );
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("moldova").join(INSTALLED_MANIFEST)).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["id"], "moldova");
        assert_eq!(manifest["version"], "legacy");
        assert_eq!(manifest["files"].as_array().unwrap().len(), 2);

        let view = installer.catalog_view().await;
        assert_eq!(view.regions[0].installed_version.as_deref(), Some("legacy"));
        assert!(view.regions[0].installed);
    }

    #[tokio::test]
    async fn migrates_a_partial_legacy_layout_and_tolerates_a_missing_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::File::create(dir.path().join(PACK_FILE))
            .unwrap()
            .set_len(PACK_BYTES)
            .unwrap();
        let installer = Arc::new(OfflineInstaller::with_catalog(
            Some(dir.path().to_path_buf()),
            RegionCatalog::bundled(),
            LEGACY_REGION,
        ));

        let status = installer.status().await;
        assert!(status.pack.installed);
        assert!(!status.graph.installed);
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.path().join("moldova").join(INSTALLED_MANIFEST)).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["files"].as_array().unwrap().len(), 1);

        // A second poll is idempotent and does not create a second manifest.
        let status = installer.status().await;
        assert!(status.pack.installed);
        assert!(!status.graph.installed);
    }

    #[tokio::test]
    async fn migration_does_not_overwrite_an_existing_region_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let region_dir = dir.path().join("moldova");
        std::fs::create_dir_all(&region_dir).unwrap();
        std::fs::write(region_dir.join(PACK_FILE), b"already here").unwrap();
        std::fs::File::create(dir.path().join(PACK_FILE))
            .unwrap()
            .set_len(PACK_BYTES)
            .unwrap();
        let installer = Arc::new(OfflineInstaller::with_catalog(
            Some(dir.path().to_path_buf()),
            RegionCatalog::bundled(),
            LEGACY_REGION,
        ));

        let _ = installer.status().await;
        assert_eq!(
            std::fs::read(region_dir.join(PACK_FILE)).unwrap(),
            b"already here",
            "the existing region file must win"
        );
        assert!(
            dir.path().join(PACK_FILE).is_file(),
            "the root file is never deleted when the move is skipped"
        );
    }

    #[tokio::test]
    async fn serves_region_packs_with_ranges_and_closes_unknown_names() {
        let dir = tempfile::tempdir().expect("temp dir");
        for (region, bytes) in [
            ("moldova", b"abcdefghij".as_slice()),
            ("romania", b"KLMNOPQRST"),
        ] {
            let region_dir = dir.path().join(region);
            std::fs::create_dir_all(&region_dir).unwrap();
            std::fs::write(region_dir.join(format!("{region}.pmtiles")), bytes).unwrap();
        }
        let catalog = fixture_catalog(vec![
            fixture_region(
                "http://127.0.0.1:1",
                "moldova",
                "Moldova",
                b"abcdefghij",
                b"gzip",
                b"graph",
            ),
            fixture_region(
                "http://127.0.0.1:1",
                "romania",
                "Romania",
                b"KLMNOPQRST",
                b"gzip",
                b"graph",
            ),
        ]);
        let installer = Arc::new(OfflineInstaller::with_catalog(
            Some(dir.path().to_path_buf()),
            catalog,
            "moldova",
        ));
        let app = Router::new()
            .route("/packs/:region/:name", get(serve_region_pack))
            .route("/packs/:name", get(serve_pack))
            .layer(Extension(Arc::clone(&installer)));
        let get_req = |uri: &str, range: Option<&str>| {
            let mut builder = axum::http::Request::builder().method(Method::GET).uri(uri);
            if let Some(range) = range {
                builder = builder.header(header::RANGE, range);
            }
            builder.body(Body::empty()).unwrap()
        };

        for (uri, expected) in [
            ("/packs/moldova/moldova.pmtiles", b"cdef".as_slice()),
            ("/packs/romania/romania.pmtiles", b"MNOP"),
        ] {
            let response = app
                .clone()
                .oneshot(get_req(uri, Some("bytes=2-5")))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT, "{uri}");
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(body.as_ref(), expected, "{uri}");
        }

        // The v2.3.0 URL still resolves to the region copy.
        let legacy = app
            .clone()
            .oneshot(get_req("/packs/moldova.pmtiles", Some("bytes=0-3")))
            .await
            .unwrap();
        assert_eq!(legacy.status(), StatusCode::PARTIAL_CONTENT);
        let body = axum::body::to_bytes(legacy.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), b"abcd");

        for uri in [
            "/packs/atlantis/moldova.pmtiles",
            "/packs/moldova/notes.txt",
            "/packs/moldova/../moldova.pmtiles",
            "/packs/..%2Fmoldova/moldova.pmtiles",
        ] {
            let response = app.clone().oneshot(get_req(uri, None)).await.unwrap();
            assert!(
                response.status() == StatusCode::NOT_FOUND
                    || response.status() == StatusCode::BAD_REQUEST,
                "{uri} should be closed, got {}",
                response.status()
            );
        }
    }
}

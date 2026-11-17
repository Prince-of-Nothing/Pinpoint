//! Offline map + routing graph download manager and pack server.
//!
//! Both loopback map servers (desktop [`crate::map_server`] and Android
//! [`crate::mobile::map_server`]) mount the same routes from this module:
//!
//! * `GET /map/config` — which PMTiles archive the page should load and
//!   whether the routing graph is installed;
//! * `GET /map/offline/status` — pollable per-artifact install state;
//! * `POST /map/offline/download` — start the single background job;
//! * `GET|HEAD /packs/<name>.pmtiles` — the installed archive with HTTP
//!   `Range` support (`206`/`416`), which PMTiles requires.
//!
//! The Rust process downloads the two data-release artifacts itself, streams
//! each response into a sibling temp file, checks the exact byte length and an
//! incremental SHA-256 digest, and only then atomically renames the temp file
//! over the installed path. A failed or truncated download therefore never
//! replaces a working pack. Nothing here calls a CDN, a geocoder or a routing
//! service, and no session or account data is involved.
//!
//! Release artifacts (immutable `data-moldova-20261116` tag):
//!
//! | file | bytes | SHA-256 |
//! |---|---:|---|
//! | `moldova.pmtiles` | 93,172,399 | `7b2715…24f2e` |
//! | `moldova.graph.bin` | 110,858,815 | `8cdbb0…0e75` |
//!
//! `PINPOINT_OFFLINE_BASE_URL` overrides the release host for local tests and
//! offline mirrors. Both map servers share the module, but each server owns an
//! installer bound to its own `packs/` directory, so tests can use temp dirs.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Extension, Path as UrlPath};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;

/// Immutable data release that holds the published Moldova artifacts.
pub const DEFAULT_BASE_URL: &str =
    "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/data-moldova-20261116";

/// Environment override for the artifact host (tests and mirrors).
pub const BASE_URL_ENV: &str = "PINPOINT_OFFLINE_BASE_URL";

/// Installed PMTiles archive name below `packs/`.
pub const PACK_FILE: &str = "moldova.pmtiles";

/// Installed routing graph name below `packs/`.
pub const GRAPH_FILE: &str = "moldova.graph.bin";

pub const PACK_BYTES: u64 = 93_172_399;
pub const PACK_SHA256: &str = "7b27155519e3384e1520d1a3dc6488f93cf1e46292dae22129f972bc97524f2e";

pub const GRAPH_BYTES: u64 = 110_858_815;
pub const GRAPH_SHA256: &str = "8cdbb0cb1d61c2d5857cb2677ab1343968223e4ae10892f0268c8feb0eb50e75";

/// The Monaco fixture used until the Moldova pack is installed. It matches
/// `web/map/fixtures/monaco.pmtiles` and is served by both map servers.
pub const FIXTURE_URL: &str = "./fixtures/monaco.pmtiles";

const MIME_PMTILES: &str = "application/octet-stream";
const MIME_TEXT: &str = "text/plain; charset=utf-8";

/// One release artifact and the metadata a successful install must match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSpec {
    pub name: &'static str,
    pub expected_bytes: u64,
    pub sha256: String,
}

impl ArtifactSpec {
    fn pack() -> Self {
        Self {
            name: PACK_FILE,
            expected_bytes: PACK_BYTES,
            sha256: PACK_SHA256.to_string(),
        }
    }

    fn graph() -> Self {
        Self {
            name: GRAPH_FILE,
            expected_bytes: GRAPH_BYTES,
            sha256: GRAPH_SHA256.to_string(),
        }
    }
}

/// Artifact metadata used by one installer.
///
/// Production always uses [`DownloadPlan::default`]. Tests inject a small
/// fixture plan (bytes + SHA-256) so the real 93/106 MB files are not needed.
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
    /// Build a fixture plan with the production file names and custom
    /// metadata. Both names stay fixed, so a plan can never escape `packs/`.
    pub fn fixture(
        base_url: impl Into<String>,
        pack_bytes: u64,
        pack_sha256: impl Into<String>,
        graph_bytes: u64,
        graph_sha256: impl Into<String>,
    ) -> Self {
        Self {
            base_url: Some(base_url.into()),
            pack: ArtifactSpec {
                name: PACK_FILE,
                expected_bytes: pack_bytes,
                sha256: pack_sha256.into(),
            },
            graph: ArtifactSpec {
                name: GRAPH_FILE,
                expected_bytes: graph_bytes,
                sha256: graph_sha256.into(),
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
    /// Current installed file length, or `0` when absent.
    pub bytes: u64,
    pub expected_bytes: u64,
    /// Download progress for the running job, or the installed size at rest.
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
}

/// Config body for `GET /map/config`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MapConfig {
    pub pmtiles_url: String,
    pub graph_installed: bool,
    pub pack_installed: bool,
}

/// Outcome of asking an installer to start a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    Started,
    AlreadyRunning,
    /// No writable `packs/` directory is configured for this server.
    Unavailable(String),
}

#[derive(Debug, Clone)]
struct FileState {
    phase: Phase,
    progress_bytes: u64,
    error: Option<String>,
}

impl FileState {
    fn idle() -> Self {
        Self {
            phase: Phase::Idle,
            progress_bytes: 0,
            error: None,
        }
    }
}

#[derive(Debug)]
struct JobState {
    running: bool,
    pack: FileState,
    graph: FileState,
}

impl JobState {
    fn new() -> Self {
        Self {
            running: false,
            pack: FileState::idle(),
            graph: FileState::idle(),
        }
    }

    fn state_mut(&mut self, name: &str) -> &mut FileState {
        if name == GRAPH_FILE {
            &mut self.graph
        } else {
            &mut self.pack
        }
    }
}

/// Download manager bound to one `packs/` directory.
pub struct OfflineInstaller {
    packs_dir: Option<PathBuf>,
    plan: DownloadPlan,
    job: Mutex<JobState>,
}

impl OfflineInstaller {
    /// Installer for the production release metadata.
    pub fn new(packs_dir: Option<PathBuf>) -> Self {
        Self::with_plan(packs_dir, DownloadPlan::default())
    }

    /// Installer with explicit artifact metadata (tests/fixtures).
    pub fn with_plan(packs_dir: Option<PathBuf>, plan: DownloadPlan) -> Self {
        Self {
            packs_dir,
            plan,
            job: Mutex::new(JobState::new()),
        }
    }

    /// Configured `packs/` directory, if any.
    pub fn packs_dir(&self) -> Option<&Path> {
        self.packs_dir.as_deref()
    }

    /// Path of an installed artifact. Only the two known file names are
    /// accepted, so callers cannot join arbitrary relative paths.
    pub fn artifact_path(&self, name: &str) -> Option<PathBuf> {
        if name != PACK_FILE && name != GRAPH_FILE {
            return None;
        }
        self.packs_dir.as_ref().map(|dir| dir.join(name))
    }

    /// Current status. Metadata checks touch the filesystem but never hash a
    /// multi-megabyte installed file on a status poll.
    pub async fn status(&self) -> OfflineStatus {
        let (pack_state, graph_state, running) = {
            let guard = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
            (guard.pack.clone(), guard.graph.clone(), guard.running)
        };
        let pack = self
            .file_status(&self.plan.pack, &pack_state, running)
            .await;
        let graph = self
            .file_status(&self.plan.graph, &graph_state, running)
            .await;
        OfflineStatus { pack, graph }
    }

    async fn file_status(
        &self,
        spec: &ArtifactSpec,
        state: &FileState,
        running: bool,
    ) -> FileStatus {
        let (installed, bytes) = match self.artifact_path(spec.name) {
            Some(path) => match tokio::fs::metadata(&path).await {
                Ok(metadata) if metadata.is_file() => {
                    (metadata.len() == spec.expected_bytes, metadata.len())
                }
                _ => (false, 0),
            },
            None => (false, 0),
        };

        let phase = match state.phase {
            Phase::Idle if installed => Phase::Installed,
            other => other,
        };
        let progress_bytes = if running {
            state.progress_bytes
        } else if phase == Phase::Installed {
            bytes
        } else {
            state.progress_bytes
        };

        FileStatus {
            installed,
            bytes,
            expected_bytes: spec.expected_bytes,
            progress_bytes,
            phase,
            error: state.error.clone(),
        }
    }

    /// `GET /map/config` body.
    pub async fn config(&self) -> MapConfig {
        let pack_installed = self.file_matches(&self.plan.pack).await;
        let graph_installed = self.file_matches(&self.plan.graph).await;
        MapConfig {
            pmtiles_url: if pack_installed {
                format!("/packs/{PACK_FILE}")
            } else {
                FIXTURE_URL.to_string()
            },
            graph_installed,
            pack_installed,
        }
    }

    async fn file_matches(&self, spec: &ArtifactSpec) -> bool {
        let Some(path) = self.artifact_path(spec.name) else {
            return false;
        };
        match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata.is_file() && metadata.len() == spec.expected_bytes,
            Err(_) => false,
        }
    }

    /// Start the single background job. Returns immediately; the caller polls
    /// [`Self::status`] or waits for the page to do so.
    pub fn start(self: &Arc<Self>) -> StartOutcome {
        if self.packs_dir.is_none() {
            return StartOutcome::Unavailable(
                "this map server has no writable packs directory".to_string(),
            );
        }
        if !valid_artifact_name(self.plan.pack.name) || !valid_artifact_name(self.plan.graph.name) {
            return StartOutcome::Unavailable(
                "the offline download plan names an unsupported artifact".to_string(),
            );
        }

        {
            let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
            if job.running {
                return StartOutcome::AlreadyRunning;
            }
            job.running = true;
            job.pack = FileState {
                phase: Phase::Downloading,
                progress_bytes: 0,
                error: None,
            };
            job.graph = FileState {
                phase: Phase::Downloading,
                progress_bytes: 0,
                error: None,
            };
        }

        let installer = Arc::clone(self);
        tokio::spawn(async move {
            installer.run_job().await;
        });
        StartOutcome::Started
    }

    async fn run_job(self: Arc<Self>) {
        let base = resolve_base_url(self.plan.base_url.as_deref());
        let client = match build_client() {
            Ok(client) => client,
            Err(error) => {
                self.fail_job(&error);
                return;
            }
        };
        let specs = [self.plan.pack.clone(), self.plan.graph.clone()];

        for spec in specs {
            if let Err(error) = self.download_one(&client, &base, &spec).await {
                tracing::warn!(artifact = spec.name, %error, "offline pack download failed");
                self.fail_file(spec.name, &error);
                self.finish_job(false);
                return;
            }
        }
        self.finish_job(true);
        tracing::info!(
            pack = PACK_FILE,
            graph = GRAPH_FILE,
            "offline map pack installed"
        );
    }

    async fn download_one(
        &self,
        client: &reqwest::Client,
        base: &str,
        spec: &ArtifactSpec,
    ) -> Result<(), String> {
        if !valid_artifact_name(spec.name) {
            return Err(format!("unsupported artifact name {:?}", spec.name));
        }
        let dir = self
            .packs_dir
            .as_ref()
            .ok_or_else(|| "no packs directory".to_string())?;
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|error| format!("could not create {}: {error}", dir.display()))?;

        let target = dir.join(spec.name);
        let temp = dir.join(format!(".{}.part", spec.name));
        // A previous crash may have left a partial temp file. It is never a
        // valid install and must not be resumed blindly.
        let _ = tokio::fs::remove_file(&temp).await;

        let url = format!("{base}/{}", spec.name);
        let response = client
            .get(&url)
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
            if length != spec.expected_bytes {
                return Err(format!(
                    "{url} declared {length} bytes but {} is expected",
                    spec.expected_bytes
                ));
            }
        }

        let mut file = tokio::fs::File::create(&temp)
            .await
            .map_err(|error| format!("could not create {}: {error}", temp.display()))?;
        let mut hasher = Sha256::new();
        let mut written = 0_u64;
        let mut body = response;

        loop {
            let chunk = match body.chunk().await {
                Ok(chunk) => chunk,
                Err(error) => {
                    drop(file);
                    let _ = tokio::fs::remove_file(&temp).await;
                    return Err(format!("download of {url} failed: {error}"));
                }
            };
            let Some(chunk) = chunk else {
                break;
            };
            if written.saturating_add(chunk.len() as u64) > spec.expected_bytes {
                drop(file);
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(format!(
                    "{url} sent more than the expected {} bytes",
                    spec.expected_bytes
                ));
            }
            if let Err(error) = file.write_all(&chunk).await {
                drop(file);
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(format!("could not write {}: {error}", temp.display()));
            }
            hasher.update(&chunk);
            written += chunk.len() as u64;
            self.set_progress(spec.name, written);
        }

        if let Err(error) = file.flush().await {
            drop(file);
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!("could not flush {}: {error}", temp.display()));
        }
        if let Err(error) = file.sync_all().await {
            drop(file);
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!("could not sync {}: {error}", temp.display()));
        }
        drop(file);

        if written != spec.expected_bytes {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!(
                "{url} ended after {written} bytes; {} were expected",
                spec.expected_bytes
            ));
        }

        self.set_phase(spec.name, Phase::Verifying);
        let digest = hex(&hasher.finalize());
        if digest != spec.sha256.to_ascii_lowercase() {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!(
                "{url} failed SHA-256 verification (got {digest}, expected {})",
                spec.sha256
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
            let state = job.state_mut(spec.name);
            state.phase = Phase::Installed;
            state.progress_bytes = spec.expected_bytes;
            state.error = None;
        }
        Ok(())
    }

    fn set_progress(&self, name: &str, progress: u64) {
        let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
        let state = job.state_mut(name);
        state.phase = Phase::Downloading;
        state.progress_bytes = progress;
    }

    fn set_phase(&self, name: &str, phase: Phase) {
        let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
        job.state_mut(name).phase = phase;
    }

    fn fail_file(&self, name: &str, error: &str) {
        let mut job = self.job.lock().unwrap_or_else(|poison| poison.into_inner());
        let state = job.state_mut(name);
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
        if success {
            job.pack.phase = Phase::Installed;
            job.pack.progress_bytes = self.plan.pack.expected_bytes;
            job.pack.error = None;
            job.graph.phase = Phase::Installed;
            job.graph.progress_bytes = self.plan.graph.expected_bytes;
            job.graph.error = None;
        } else {
            // The failed artifact keeps `Failed`; an artifact that was never
            // reached must not stay stuck in `Downloading` forever.
            settle_unstarted(&mut job.pack);
            settle_unstarted(&mut job.graph);
        }
    }
}

fn settle_unstarted(state: &mut FileState) {
    if state.phase == Phase::Downloading || state.phase == Phase::Verifying {
        *state = FileState::idle();
    }
}

/// Only the two published artifact names may be joined with `packs/`.
fn valid_artifact_name(name: &str) -> bool {
    name == PACK_FILE || name == GRAPH_FILE
}

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .user_agent(concat!(
            "Pinpoint/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/Prince-of-Nothing/Pinpoint)"
        ))
        .build()
        .map_err(|error| format!("could not create the download client: {error}"))
}

/// `PINPOINT_OFFLINE_BASE_URL`/plan override or the immutable release host.
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
        Some(base) if base.starts_with("http://") || base.starts_with("https://") => {
            base.trim_end_matches('/').to_string()
        }
        _ => DEFAULT_BASE_URL.to_string(),
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
// Axum handlers
// ---------------------------------------------------------------------------

/// `GET /map/offline/status`
pub async fn status(Extension(installer): Extension<Arc<OfflineInstaller>>) -> Response {
    Json(installer.status().await).into_response()
}

/// `POST /map/offline/download`
pub async fn download(Extension(installer): Extension<Arc<OfflineInstaller>>) -> Response {
    match installer.start() {
        StartOutcome::Started => {
            (StatusCode::ACCEPTED, Json(installer.status().await)).into_response()
        }
        StartOutcome::AlreadyRunning => {
            (StatusCode::CONFLICT, Json(installer.status().await)).into_response()
        }
        StartOutcome::Unavailable(message) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "OFFLINE_PACKS_UNAVAILABLE",
            &message,
        ),
    }
}

/// `GET /map/config`
pub async fn config(Extension(installer): Extension<Arc<OfflineInstaller>>) -> Response {
    Json(installer.config().await).into_response()
}

/// `GET|HEAD /packs/<name>.pmtiles`
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
    if method != Method::GET && method != Method::HEAD {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, "GET, HEAD")],
            "Method Not Allowed",
        )
            .into_response();
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
    let candidate = root.join(name);
    let path = match tokio::fs::canonicalize(&candidate).await {
        Ok(path) if path.starts_with(&root) => path,
        _ => return text_response(StatusCode::NOT_FOUND, "Not Found"),
    };
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return text_response(StatusCode::NOT_FOUND, "Not Found"),
    };
    let size = metadata.len();

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
            if method == Method::HEAD {
                return pack_response(StatusCode::OK, MIME_PMTILES, size, None, Body::empty());
            }
            match tokio::fs::File::open(&path).await {
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
            if method == Method::HEAD {
                return pack_response(
                    StatusCode::PARTIAL_CONTENT,
                    MIME_PMTILES,
                    length,
                    Some(format!("bytes {start}-{end}/{size}")),
                    Body::empty(),
                );
            }
            match tokio::fs::File::open(&path).await {
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
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn sha256_hex(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    fn fixture_plan(base: &str, pack: &[u8], graph: &[u8]) -> DownloadPlan {
        DownloadPlan::fixture(
            base,
            pack.len() as u64,
            sha256_hex(pack),
            graph.len() as u64,
            sha256_hex(graph),
        )
    }

    /// Local stand-in for the GitHub release. The body is streamed in small
    /// chunks so status polling observes real progress instead of one jump.
    async fn fixture_server(pack: Vec<u8>, graph: Vec<u8>, chunk_delay: Duration) -> String {
        async fn artifact(
            UrlPath(name): UrlPath<String>,
            axum::extract::State(state): axum::extract::State<Arc<FixtureArtifacts>>,
        ) -> Response {
            let bytes = match name.as_str() {
                PACK_FILE => state.pack.clone(),
                GRAPH_FILE => state.graph.clone(),
                _ => return text_response(StatusCode::NOT_FOUND, "Not Found"),
            };
            let total = bytes.len() as u64;
            let delay = state.delay;
            let (mut writer, reader) = tokio::io::duplex(8 * 1024);
            tokio::spawn(async move {
                for chunk in bytes.chunks(1024) {
                    if writer.write_all(chunk).await.is_err() {
                        return;
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
            graph: Vec<u8>,
            delay: Duration,
        }

        let state = Arc::new(FixtureArtifacts {
            pack,
            graph,
            delay: chunk_delay,
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
    async fn installs_both_artifacts_with_verified_bytes_and_no_temp_files() {
        let pack = b"PMTiles fixture pack".to_vec();
        let graph = b"synthetic graph bytes".to_vec();
        let base = fixture_server(pack.clone(), graph.clone(), Duration::from_millis(2)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture_plan(&base, &pack, &graph),
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let status = wait_finished(&installer).await;

        assert!(status.pack.installed, "{status:?}");
        assert!(status.graph.installed, "{status:?}");
        assert_eq!(status.pack.bytes, pack.len() as u64);
        assert_eq!(status.graph.bytes, graph.len() as u64);
        assert_eq!(status.pack.phase, Phase::Installed);
        assert_eq!(status.graph.phase, Phase::Installed);
        assert_eq!(status.pack.progress_bytes, pack.len() as u64);
        assert_eq!(status.graph.progress_bytes, graph.len() as u64);
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
            "installed graph bytes"
        );
        assert!(!dir.path().join(format!(".{PACK_FILE}.part")).exists());
        assert!(!dir.path().join(format!(".{GRAPH_FILE}.part")).exists());
    }

    #[tokio::test]
    async fn wrong_hash_is_rejected_and_never_replaces_an_installed_file() {
        let pack = b"new pack bytes".to_vec();
        let graph = b"graph".to_vec();
        let base = fixture_server(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let old = b"old verified pack".to_vec();
        std::fs::write(dir.path().join(PACK_FILE), &old).unwrap();

        let mut plan = fixture_plan(&base, &pack, &graph);
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
    async fn a_verified_redownload_replaces_the_old_file() {
        let pack = b"replacement pack".to_vec();
        let graph = b"replacement graph".to_vec();
        let base = fixture_server(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(PACK_FILE), b"old pack").unwrap();
        std::fs::write(dir.path().join(GRAPH_FILE), b"old graph").unwrap();
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture_plan(&base, &pack, &graph),
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
        let base = fixture_server(pack.clone(), graph.clone(), Duration::ZERO).await;
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(PACK_FILE), b"old pack").unwrap();
        let mut plan = fixture_plan(&base, &pack, &graph);
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
        let graph = vec![9_u8; 8 * 1024];
        let base = fixture_server(pack.clone(), graph.clone(), Duration::from_millis(8)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture_plan(&base, &pack, &graph),
        ));

        assert_eq!(installer.start(), StartOutcome::Started);
        let mut saw_downloading = false;
        let mut saw_progress = false;
        for _ in 0..500 {
            let status = installer.status().await;
            if status.pack.phase == Phase::Downloading {
                saw_downloading = true;
            }
            if status.pack.progress_bytes > 0 && status.pack.progress_bytes < pack.len() as u64 {
                saw_progress = true;
            }
            if status.pack.installed && status.graph.installed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let status = wait_finished(&installer).await;

        assert!(saw_downloading, "status must report the downloading phase");
        assert!(saw_progress, "status must report partial progress bytes");
        assert!(status.pack.installed);
        assert!(status.graph.installed);
    }

    #[tokio::test]
    async fn a_second_start_while_running_is_rejected() {
        let pack = vec![1_u8; 16 * 1024];
        let graph = vec![2_u8; 4 * 1024];
        let base = fixture_server(pack.clone(), graph.clone(), Duration::from_millis(10)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture_plan(&base, &pack, &graph),
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
        let base = fixture_server(pack.clone(), graph.clone(), Duration::from_millis(10)).await;
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::with_plan(
            Some(dir.path().to_path_buf()),
            fixture_plan(&base, &pack, &graph),
        ));
        let app = Router::new()
            .route("/map/offline/download", axum::routing::post(download))
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
    async fn config_uses_the_fixture_until_the_pack_is_installed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let installer = Arc::new(OfflineInstaller::new(Some(dir.path().to_path_buf())));

        let config = installer.config().await;
        assert_eq!(config.pmtiles_url, FIXTURE_URL);
        assert!(!config.pack_installed);
        assert!(!config.graph_installed);

        // A file with the production length is treated as installed (the hash
        // was verified when it was installed). A short file is not.
        std::fs::File::create(dir.path().join(PACK_FILE))
            .unwrap()
            .set_len(PACK_BYTES)
            .unwrap();
        std::fs::write(dir.path().join(GRAPH_FILE), b"short").unwrap();
        let config = installer.config().await;
        assert_eq!(config.pmtiles_url, format!("/packs/{PACK_FILE}"));
        assert!(config.pack_installed);
        assert!(!config.graph_installed);

        std::fs::File::create(dir.path().join(GRAPH_FILE))
            .unwrap()
            .set_len(GRAPH_BYTES)
            .unwrap();
        assert!(installer.config().await.graph_installed);
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
    fn base_url_resolution_prefers_the_override_and_rejects_other_schemes() {
        assert_eq!(
            resolve_base_url(Some("http://127.0.0.1:9000/packs/")),
            "http://127.0.0.1:9000/packs"
        );
        assert_eq!(
            resolve_base_url(Some("https://mirror.example/moldova")),
            "https://mirror.example/moldova"
        );
        assert_eq!(
            resolve_base_url_with(None, Some("https://env.example/release/")),
            "https://env.example/release"
        );
        assert_eq!(
            resolve_base_url_with(Some("   "), Some("https://env.example/release")),
            DEFAULT_BASE_URL,
            "an empty plan override falls back instead of the environment"
        );
        assert_eq!(
            resolve_base_url_with(None, Some("ftp://example/x")),
            DEFAULT_BASE_URL
        );
    }

    #[test]
    fn plan_keeps_only_the_two_published_file_names() {
        let plan = DownloadPlan::default();
        assert_eq!(plan.pack.name, PACK_FILE);
        assert_eq!(plan.graph.name, GRAPH_FILE);
        let fixture = DownloadPlan::fixture("http://127.0.0.1:1", 1, "aa", 2, "bb");
        assert_eq!(fixture.for_file(PACK_FILE).expected_bytes, 1);
        assert_eq!(fixture.for_file(GRAPH_FILE).expected_bytes, 2);
        assert!(!valid_pack_name("../")); // sanity: the scorer cannot join paths
        assert!(valid_pack_name(PACK_FILE));
        assert!(!valid_pack_name("notes.txt"));
        assert!(!valid_pack_name(".hidden.pmtiles"));
        assert!(!valid_pack_name("nested/pack.pmtiles"));
    }
}

# Offline Data Pipeline (Moldova) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a reproducible Moldova offline pack: OSM extract → PMTiles basemap, SQLite FTS5 search index, Valhalla routing tiles, and a verified manifest.

**Architecture:** PowerShell orchestration calls pinned Docker images for Planetiler and Valhalla, and a small Rust CLI builds the SQLite search index. Outputs are large files under `dist/offline/<region>/` (not committed); the manifest, checksums and size report are committed.

**Tech Stack:** PowerShell 7, Docker, Planetiler OpenMapTiles, Valhalla, Rust (`rusqlite`, `osmpbfreader`), SQLite FTS5, SHA-256.

**Spec:** `docs/superpowers/specs/2026-11-16-offline-maps-design.md`

## Global Constraints

- OSM source: Geofabrik `moldova-latest.osm.pbf`; never `tile.openstreetmap.org`.
- Attribution `© OpenStreetMap contributors`; license `ODbL-1.0`; license URL `https://opendatacommons.org/licenses/odbl/1-0/`.
- Pack version format `moldova-YYYYMMDD`; artifacts are immutable once published.
- Output layout: `dist/offline/moldova/{moldova.pmtiles,moldova-search.sqlite,moldova-valhalla.tar,manifest.json,ATTRIBUTION.md}`.
- Builds are atomic: write to a temp directory, verify, then rename; never publish a partial pack.
- The offline CLI is behind the `offline-build` Cargo feature; the normal app build/CI must not require `rusqlite` or `osmpbfreader`.
- Scripts run on Windows PowerShell 5.1+ and `pwsh` on Linux CI.
- No secrets in the repo; the build only downloads public OSM data.

## Review Focus

- A failed Docker/network run must leave no `manifest.json` and no partial pack.
- Missing or altered ODbL attribution/license must fail verification.
- Search must find `Bălți` by `balti` and `Кишинёв` by `кишин`.
- An empty/truncated PBF or an artifact with a bad SHA-256 must be rejected.
- Missing Docker/Java must fail with a clear message, not silently skip an artifact.

---

### Task 1: Region config and dry-run build skeleton

**Files:**
- Create: `scripts/offline/regions.json`
- Create: `scripts/offline/build.ps1`
- Create: `scripts/offline/tests/build-dryrun.ps1`

**Interfaces:**
- Consumes: nothing.
- Produces: `build.ps1 -Region <id> [-OutputRoot <path>] [-DryRun]` and a region schema with `name`, `geofabrik`, `bbox` `[west,south,east,north]`, `minzoom`, `maxzoom`, `valhalla_config`.

- [ ] **Step 1: Write the failing dry-run test**

`scripts/offline/tests/build-dryrun.ps1`:

```powershell
$ErrorActionPreference = "Stop"
$root = Join-Path ([IO.Path]::GetTempPath()) ("offline-dryrun-" + [guid]::NewGuid())
$out = & "$PSScriptRoot/../build.ps1" -Region moldova -OutputRoot $root -DryRun 2>&1 | Out-String
foreach ($needle in @("moldova.pmtiles", "moldova-search.sqlite", "moldova-valhalla.tar", "planetiler", "valhalla_build_tiles", "pinpoint-offline-index")) {
    if ($out -notmatch [regex]::Escape($needle)) { throw "dry-run missing $needle" }
}
if ($out -notmatch "DRY RUN") { throw "dry-run marker missing" }
if (Test-Path $root) { throw "dry-run created the output directory" }
Write-Host "build dry-run ok"
```

- [ ] **Step 2: Run it to verify it fails**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/build-dryrun.ps1`
Expected: FAIL because `build.ps1` does not exist.

- [ ] **Step 3: Add the region config**

`scripts/offline/regions.json`:

```json
{
  "moldova": {
    "name": "Moldova",
    "geofabrik": "https://download.geofabrik.de/europe/moldova-latest.osm.pbf",
    "bbox": [26.6, 45.4, 30.2, 48.5],
    "minzoom": 0,
    "maxzoom": 14,
    "valhalla_config": "scripts/offline/valhalla-moldova.json"
  },
  "monaco": {
    "name": "Monaco",
    "geofabrik": "https://download.geofabrik.de/europe/monaco-latest.osm.pbf",
    "bbox": [7.40, 43.72, 7.44, 43.75],
    "minzoom": 0,
    "maxzoom": 12,
    "valhalla_config": "scripts/offline/valhalla-monaco.json"
  }
}
```

- [ ] **Step 4: Add the dry-run branch**

`scripts/offline/build.ps1` parses `-Region`, `-OutputRoot` (default `dist/offline`), `-DryRun`, `-SkipDownload`, `-SkipBasemap`, `-SkipSearch`, `-SkipRouting`. It loads `regions.json`, computes `$packDir = Join-Path $OutputRoot $Region`, and on `-DryRun` writes the planned artifact paths plus the Docker/CLI commands (as text) to stdout and exits 0. No `Invoke-WebRequest`, no `docker run`, no directory creation.

- [ ] **Step 5: Run the test to verify it passes**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/build-dryrun.ps1`
Expected: `build dry-run ok`.

- [ ] **Step 6: Commit**

```bash
git add scripts/offline/regions.json scripts/offline/build.ps1 scripts/offline/tests/build-dryrun.ps1
git commit -m "[FEATURE] offline: add region config and dry-run build"
```

---

### Task 2: Atomic OSM extract download and checksum

**Files:**
- Create: `scripts/offline/download.ps1`
- Create: `scripts/offline/tests/download.ps1`
- Modify: `scripts/offline/build.ps1` (call the downloader unless `-SkipDownload`)

**Interfaces:**
- Consumes: region config from Task 1.
- Produces: `Get-OsmExtract -Region <object> -Destination <path> [-SourcePath <local-file>] [-ExpectedSha256 <hex>]` returns the local path; throws on HTTP/size/hash failure.

- [ ] **Step 1: Write the failing test**

`scripts/offline/tests/download.ps1` creates a temp file with `hello osm`, calls `Get-OsmExtract -SourcePath <temp> -Destination <temp-out>`, asserts content matches. Then calls it with `-ExpectedSha256 "0000..."` and asserts it throws. It must leave no destination file after the failure.

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/download.ps1`
Expected: FAIL because `download.ps1` does not exist.

- [ ] **Step 2: Implement the downloader**

`download.ps1` uses `Invoke-WebRequest -UseBasicParsing` for URLs or `Copy-Item` for `-SourcePath`. It writes to `<Destination>.part`, optionally checks `Get-FileHash -Algorithm SHA256`, then `Move-Item -Force` to the destination. On any error it removes `.part` and rethrows. An existing destination with the expected hash is reused.

- [ ] **Step 3: Wire it into `build.ps1`**

Unless `-SkipDownload`, call `Get-OsmExtract` for `<packDir>/<region>.osm.pbf`.

- [ ] **Step 4: Run the test and the dry-run test**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/download.ps1`
Expected: `download ok`.
Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/build-dryrun.ps1`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/offline/download.ps1 scripts/offline/tests/download.ps1 scripts/offline/build.ps1
git commit -m "[FEATURE] offline: download OSM extracts atomically"
```

---

### Task 3: Rust search-index CLI (SQLite FTS5)

**Files:**
- Modify: `Cargo.toml` (feature, optional deps, bin target)
- Create: `src/bin/offline_index.rs`
- Test: `src/bin/offline_index.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: the region PBF.
- Produces: `pinpoint-offline-index --osm <pbf> --out <sqlite>` and `pinpoint-offline-index --verify <sqlite>`.
- Produces: `normalize_name(&str) -> String`, `init_schema(&Connection) -> rusqlite::Result<()>`, `insert_places(&mut Connection, &[Place]) -> rusqlite::Result<()>`, `search(&Connection, &str, usize) -> rusqlite::Result<Vec<Place>>`.

- [ ] **Step 1: Write the failing tests**

In `src/bin/offline_index.rs`:

```rust
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
    insert_places(&mut conn, &[
        place(1, "Bălți", "balti", 47.76, 27.93),
        place(2, "Кишинёв", "кишинев", 47.01, 28.86),
    ]).unwrap();
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
        kind: "place".into(),
        lat,
        lon,
        address: None,
        aliases: name.into(),
        importance: 0.5,
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --features offline-build --bin pinpoint-offline-index`
Expected: FAIL because the feature/bin does not exist.

- [ ] **Step 3: Add the feature, deps and bin**

`Cargo.toml`:

```toml
[features]
offline-build = ["dep:rusqlite", "dep:osmpbfreader"]

[[bin]]
name = "pinpoint-offline-index"
path = "src/bin/offline_index.rs"
required-features = ["offline-build"]

[dependencies]
rusqlite = { version = "0.32", features = ["bundled"], optional = true }
osmpbfreader = { version = "0.19", optional = true }
```

- [ ] **Step 4: Implement the schema and search**

Tables:

```sql
CREATE TABLE places (
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
CREATE VIRTUAL TABLE places_fts USING fts5(
  name, normalized, aliases, address,
  content='places', content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER places_ai AFTER INSERT ON places BEGIN
  INSERT INTO places_fts(rowid, name, normalized, aliases, address)
  VALUES (new.id, new.name, new.normalized, new.aliases, new.address);
END;
```

`normalize_name` lowercases, maps `ă â î ș ț` (and uppercase) to `a a i s t`, maps Cyrillic `ё` to `е`, and leaves other characters. `search` normalizes the query and uses `places_fts MATCH ?` ordered by `rank`.

- [ ] **Step 5: Implement the PBF loop**

Parse `osmpbfreader::OsmObj::Node`; skip nodes without any name tag. Build `names` from `name`, `name:ro`, `name:ru`, `name:uk`, `name:bg`, `name:gag`, `name:en`, `alt_name`, `official_name`, `short_name`. The display `name` is the first available in preference order ro → ru → uk → bg → gag → en → `name`; `aliases` is every collected name joined with a single space; `normalized = normalize_name(name)`. Insert one row per node. `kind` = `place` tag value if present, else `amenity`/`shop`/`tourism`/`leisure` value, else `"place"`. `importance` = 0.9 city, 0.7 town, 0.5 village, 0.3 hamlet, else 0.1; add `population / 1_000_000` capped at 0.5.

`kind` = `place` tag value if present, else `amenity`/`shop`/`tourism`/`leisure` value, else `"place"`. `importance` = 0.9 city, 0.7 town, 0.5 village, 0.3 hamlet, else 0.1; add `population / 1_000_000` capped at 0.5.

- [ ] **Step 6: Implement CLI args and verify**

`--osm/--out` builds; `--verify <sqlite>` opens read-only, runs `select count(*) from places`, prints `{"places":N}`, exits 0 only if N > 0 and `places_fts` exists. `--fixture <sqlite>` creates a fresh DB with the two sample places from the test (for the pipeline fixture, no PBF needed).

- [ ] **Step 7: Run the tests and clippy**

Run: `cargo test --features offline-build --bin pinpoint-offline-index`
Expected: PASS.
Run: `cargo clippy --features offline-build --all-targets -- -D warnings`
Expected: PASS.
Run: `cargo build --locked` (without the feature)
Expected: PASS; the bin is skipped.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src/bin/offline_index.rs
git commit -m "[FEATURE] offline: build SQLite FTS5 search index"
```

---

### Task 4: Manifest generation with SHA-256

**Files:**
- Create: `scripts/offline/manifest.ps1`
- Create: `scripts/offline/tests/manifest.ps1`
- Modify: `scripts/offline/build.ps1`

**Interfaces:**
- Consumes: artifact paths + region config.
- Produces: `New-OfflineManifest -Region <object> -Version <string> -Files <string[]> -Output <path> [-BaseUrl <string>]` writes `manifest.json` with `id`, `name`, `version`, `built_at`, `bbox`, `minzoom`, `maxzoom`, `files[] {name,url,bytes,sha256}`, `attribution`, `license`, `license_url`. `url` is `"$BaseUrl/<name>"` when `-BaseUrl` is supplied, otherwise `""`.

- [ ] **Step 1: Write the failing test**

`scripts/offline/tests/manifest.ps1` creates two temp files (`a.pmtiles`, `b.sqlite`) with known bytes, calls `New-OfflineManifest -BaseUrl https://example.test/packs/moldova`, asserts the JSON has both files with correct `bytes`/`sha256` and `url` values, and asserts a missing file throws and leaves no manifest.

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/manifest.ps1`
Expected: FAIL because `manifest.ps1` does not exist.

- [ ] **Step 2: Implement the generator**

Use `Get-FileHash -Algorithm SHA256`, `(Get-Item).Length`, `ConvertTo-Json -Depth 6`. Reject zero-byte files. Write to `<Output>.part`, then `Move-Item` to `manifest.json`.

- [ ] **Step 3: Wire it into `build.ps1`**

After all artifacts exist, call the generator with the files that were built (allow `-SkipRouting` to omit the tar; the manifest lists only present files).

- [ ] **Step 4: Run the test**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/manifest.ps1`
Expected: `manifest ok`.

- [ ] **Step 5: Commit**

```bash
git add scripts/offline/manifest.ps1 scripts/offline/tests/manifest.ps1 scripts/offline/build.ps1
git commit -m "[FEATURE] offline: write signed-by-hash manifests"
```

---

### Task 5: Pack verification script

**Files:**
- Create: `scripts/offline/verify.ps1`
- Create: `scripts/offline/tests/verify.ps1`
- Create: `scripts/offline/tests/fixture.ps1`
- Create: `scripts/offline/ATTRIBUTION.md`

**Interfaces:**
- Consumes: `manifest.json` + artifacts.
- Produces: `verify.ps1 -PackDir <path>` exits 0 only when every check passes; prints one line per check.

- [ ] **Step 1: Write the fixture builder**

`scripts/offline/tests/fixture.ps1` creates a temp pack with:
- `moldova.pmtiles` starting with ASCII `PMTiles` + version byte `3` + zero padding;
- `moldova-search.sqlite` created by `pinpoint-offline-index --fixture <path>`;
- `moldova-valhalla.tar` containing `valhalla_tiles/0/000/000/000.gph` (create with `tar -cf`);
- `ATTRIBUTION.md` with the ODbL line;
- a manifest generated by Task 4.

- [ ] **Step 2: Write the failing verify test**

`scripts/offline/tests/verify.ps1` runs `verify.ps1` on the fixture (expect pass), then flips one byte in `moldova.pmtiles` and expects failure with a hash message.

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/verify.ps1`
Expected: FAIL because `verify.ps1` does not exist.

- [ ] **Step 3: Implement `verify.ps1`**

Checks:
1. manifest exists and parses; `attribution` contains `OpenStreetMap`; `license` is `ODbL-1.0`.
2. every listed file exists; `bytes` and SHA-256 match.
3. `moldova.pmtiles` first 7 bytes are `PMTiles`.
4. `moldova-search.sqlite` passes `pinpoint-offline-index --verify`.
5. `moldova-valhalla.tar` lists at least one `.gph` entry (`tar -tf`).
6. `ATTRIBUTION.md` exists and contains `OpenStreetMap`.

- [ ] **Step 4: Add the attribution file**

`scripts/offline/ATTRIBUTION.md`:

```markdown
Map data © OpenStreetMap contributors, available under the Open Database License (ODbL) 1.0.
https://www.openstreetmap.org/copyright
https://opendatacommons.org/licenses/odbl/1-0/
```

- [ ] **Step 5: Run the test**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/verify.ps1`
Expected: `verify ok` and then `tamper rejected`.

- [ ] **Step 6: Commit**

```bash
git add scripts/offline/verify.ps1 scripts/offline/tests/verify.ps1 scripts/offline/tests/fixture.ps1 scripts/offline/ATTRIBUTION.md
git commit -m "[FEATURE] offline: verify pack hashes, PMTiles, search and attribution"
```

---

### Task 6: Valhalla tiles and build wiring

**Files:**
- Create: `scripts/offline/valhalla-moldova.json`
- Create: `scripts/offline/valhalla-monaco.json`
- Modify: `scripts/offline/build.ps1`
- Modify: `scripts/offline/tests/build-dryrun.ps1`

**Interfaces:**
- Consumes: the PBF and region config.
- Produces: `<region>-valhalla.tar` containing `valhalla_tiles/**/*.gph`.

- [ ] **Step 1: Generate the Valhalla config**

Use the official Valhalla image's `valhalla_build_config` to write the config with `mjolnir.tile_dir=/data/valhalla_tiles`, `mjolnir.tile_extract=/data/<region>-valhalla.tar`, `mjolnir.concurrency=2`, and the default service limits. Commit the generated config for Moldova and Monaco.

- [ ] **Step 2: Add the Docker command to `build.ps1`**

Unless `-SkipRouting`:

```text
docker run --rm --entrypoint valhalla_build_tiles -v "<packDir>:/data" ghcr.io/valhalla/valhalla:latest \
  -c /data/<region>-valhalla.json /data/<region>.osm.pbf
```

Then `tar -C <packDir> -cf <packDir>/<region>-valhalla.tar valhalla_tiles`.

- [ ] **Step 3: Extend the dry-run test**

Assert the dry-run output contains `valhalla_build_tiles` and `<region>-valhalla.tar`.

- [ ] **Step 4: Run the tests**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/build-dryrun.ps1`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add scripts/offline/valhalla-moldova.json scripts/offline/valhalla-monaco.json scripts/offline/build.ps1 scripts/offline/tests/build-dryrun.ps1
git commit -m "[FEATURE] offline: build Valhalla routing tiles"
```

---

### Task 7: End-to-end Moldova build and size report

**Files:**
- Create: `docs/offline/moldova.md`
- Modify: `scripts/offline/build.ps1` (final verify call)
- Test: `scripts/offline/tests/e2e.ps1` (manual, requires Docker/network)

**Interfaces:**
- Consumes: all prior tasks.
- Produces: a verified `dist/offline/moldova/` pack and a committed size report.

- [ ] **Step 1: Add the manual end-to-end script**

`scripts/offline/tests/e2e.ps1`:

```powershell
$ErrorActionPreference = "Stop"
& "$PSScriptRoot/../build.ps1" -Region moldova
if ($LASTEXITCODE -ne 0) { throw "build failed" }
& "$PSScriptRoot/../verify.ps1" -PackDir "$PSScriptRoot/../../../dist/offline/moldova"
if ($LASTEXITCODE -ne 0) { throw "verify failed" }
Write-Host "e2e ok"
```

- [ ] **Step 2: Run the full build**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/tests/e2e.ps1`
Expected: `moldova.pmtiles`, `moldova-search.sqlite`, `moldova-valhalla.tar`, `manifest.json`, `ATTRIBUTION.md`; `e2e ok`.

- [ ] **Step 3: Record the sizes and versions**

`docs/offline/moldova.md` includes build date, Planetiler/Valhalla image digests, artifact bytes, SHA-256 prefixes, min/max zoom, bbox, and the exact verify command. Do **not** commit the pack files.

- [ ] **Step 4: Run the verify script separately**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/offline/verify.ps1 -PackDir dist/offline/moldova`
Expected: all checks pass.

- [ ] **Step 5: Commit**

```bash
git add docs/offline/moldova.md scripts/offline/build.ps1 scripts/offline/tests/e2e.ps1
git commit -m "[DOCS] offline: record Moldova pack build and sizes"
```

---

### Task 8: CI smoke job (Monaco, no routing)

**Files:**
- Create: `.github/workflows/offline-data.yml`
- Modify: `scripts/offline/tests/build-dryrun.ps1` (if needed)

**Interfaces:**
- Consumes: the pipeline.
- Produces: a `workflow_dispatch` job that builds Monaco basemap + search + manifest, runs verify, and uploads the manifest as an artifact (not the pack).

- [ ] **Step 1: Add the workflow**

`workflow_dispatch` inputs: `region` (default `monaco`), `skip_routing` (default `true`). The job runs on `ubuntu-latest`, installs Rust, runs `cargo test --features offline-build --bin pinpoint-offline-index`, then `pwsh scripts/offline/build.ps1 -Region monaco -SkipRouting` and `pwsh scripts/offline/verify.ps1 -PackDir dist/offline/monaco`. Upload `manifest.json` and `docs` only.

- [ ] **Step 2: Run the workflow manually**

Run: `gh workflow run offline-data.yml -f region=monaco -f skip_routing=true`
Expected: success; the artifact contains `manifest.json` with the PMTiles and SQLite hashes.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/offline-data.yml
git commit -m "[INFRA] ci: add offline data smoke build"
```

---

## Self-Review

- **Spec coverage:** the plan covers the data pipeline from design §7 (Planetiler, search, Valhalla, manifest, hosting artifacts), §8 pack format, §9 search languages, §11 download/verify, §15 ODbL. It does not cover the web/Android/desktop map clients, search/routing UI or pack download manager — those are separate plans.
- **Type consistency:** `Place` / `normalize_name` / `init_schema` / `insert_places` / `search` are defined once in Task 3 and used by Tasks 5 and 7. `New-OfflineManifest` is defined in Task 4 and used by Task 5. `Get-OsmExtract` is defined in Task 2 and used by Task 7.
- **Review Focus:** each line has a test in Tasks 2, 3 and 5 (missing Docker is covered by the dry-run/clear error in Task 1; empty PBF/hash in Task 5; attribution in Task 5).
- **Proportion:** the plan is mostly decisions, commands and test assertions; code bodies are limited to the FTS schema and the normalization rules.

## Next plans

After this pipeline produces a verified Moldova pack:

1. `2026-11-17-offline-web-map.md` — MapLibre GL JS + PMTiles + OPFS + service worker.
2. `2026-11-17-offline-android-desktop.md` — WebView shells, loopback server, bridge.
3. `2026-11-17-offline-search-ui.md` — search UI on all platforms.
4. `2026-11-17-offline-routing.md` — Valhalla native/WASM, turn-by-turn, voice.

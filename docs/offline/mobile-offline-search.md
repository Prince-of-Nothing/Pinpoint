# Native/desktop offline search (per region)

**Status:** Proposed · 2026-11-17 · Search slice of
`docs/superpowers/specs/2026-11-17-global-maps-design.md`; native counterpart
to `web/map/search.js`. Related: ADR-0020, `global-maps-test-plan.md`.

## Index that already exists

`pinpoint-offline-index` (`src/bin/offline_index.rs`, feature `offline-build`)
writes `dist/offline/<region>/<region>-search.sqlite`; Moldova is 6,770,688 B
with 36,913 places. `init_schema` creates:

```sql
places(id INTEGER PRIMARY KEY, name TEXT, normalized TEXT, aliases TEXT,
       kind TEXT, lat REAL, lon REAL, address TEXT, importance REAL);
places_fts USING fts5(name, normalized, aliases, address,
       content='places', content_rowid='id',
       tokenize='unicode61 remove_diacritics 2');
-- places_ai / places_ad triggers keep places_fts in sync
```

`aliases` holds `name`, `name:ro|ru|uk|bg|gag|en`, `alt_name`,
`official_name`, `short_name`, so one MATCH covers every language.
`normalized` and every query token pass through `normalize_name`
(ă/â→a, î→i, ș/ş→s, ț/ţ→t, ё→е, lowercase); `unicode61` keeps Cyrillic.
It is a rollback-journal DB, so it opens read-only with no `-wal`/`-shm`
sibling (the existing `--verify` proves this).

**One module, both sides.** Move `Place`, `normalize_name`, `search_terms`,
`init_schema`, `insert_places`, `search` into `src/offline_search.rs`
(`not(wasm32)`) and have the bin import them, so the endpoint runs the SQL the
builder tests. This un-gates `rusqlite` (bundled) from `offline-build`;
`osmpbfreader` stays there.

## Query path

```sql
SELECT p.name, p.kind, p.lat, p.lon, p.address
  FROM places_fts JOIN places p ON p.id = places_fts.rowid
 WHERE places_fts MATCH ?1
 ORDER BY (places_fts.rank - p.importance * 10.0)
 LIMIT ?2
```

- **Prefix:** `search_terms` splits on non-alphanumerics and appends `*`, so
  `balt` matches `Bălți`; punctuation is a separator, never syntax.
- **Folding:** index and query both fold diacritics, so `Chisinau` finds
  `Chișinău` and `кишин` finds `Кишинёв`.
- **Ranking:** FTS5 `rank` is negative; `- importance * 10` lets a city
  outrank a bar with the same term. `web/map/search.js` uses
  `rank * (1 + 0.75 * importance)` — switch it to this expression in the same
  PR or native and web order results differently.
- **Read-only, off the runtime thread:** `open_with_flags(path,
  SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX)`, one connection per request
  dropped after, all inside `spawn_blocking`. No cache in v1 (~1 ms query);
  add a `Mutex<Connection>` keyed on (len, mtime) only if a large region
  profiles slow.

## Catalog and installer

`offline_install.rs` today downloads exactly two hardcoded Moldova artifacts
(PMTiles + gzip graph); `scripts/offline/` builds the search DB but never
releases it to the app. Add a third artifact:

- catalog `files[]`: `{"role":"search","transfer_name":"<id>-search.sqlite",
  "installed_name":"<id>-search.sqlite","compression":"identity"}` — schema 1
  already supports it;
- installed at `packs/<region>/<region>-search.sqlite`;
- Moldova becomes 145,281,673 B transfer (145 MB), 210,801,902 B on disk
  (211 MB).

Every two-file assumption must grow: `DownloadPlan`, `ArtifactSpec::search()`,
`expected_*_bytes`, `JobState`/`state_mut`, `run_job`, `finish_job`,
`settle_unstarted`, `artifact_path`, `valid_artifact_name`/`valid_transfer_name`,
`OfflineStatus` (+`search: FileStatus`), `MapConfig` (+`searchInstalled`), and
`DownloadPlan::fixture_gzip` callers (`tests/map_offline_e2e.rs`,
`offline_install` tests). Keep the single-segment, catalog-derived name rule.
**The SQLite file is never HTTP-served**: `/packs/` stays `.pmtiles`-only.

## `GET /map/search`

Handler in `src/offline_search.rs`, mounted beside `/map/route` in both
loopback servers, reusing the `Arc<OfflineInstaller>` extension:

```text
GET /map/search?region=moldova&q=balti&limit=20
200 {"region":"moldova","results":[
      {"name":"Bălți","kind":"city","lat":47.76,"lon":27.93,"address":null}]}
```

- `region` must match `[a-z0-9-]{1,64}` and resolve to an installed directory
  under `packs/`, else 400 `INVALID_REGION` / 404 `REGION_NOT_INSTALLED`.
- `q` is trimmed, capped at 128 chars and 8 tokens; empty or
  punctuation-only returns `results: []` without opening the DB (FTS5 rejects
  an empty MATCH).
- `limit` defaults to 20, parsed leniently, clamped to 1–50.
- Missing/corrupt index or absent `places`/`places_fts` → 503
  `SEARCH_UNAVAILABLE` (same shape as `ROUTING_UNAVAILABLE`), never a 500.
- `kind` is the raw OSM value (`city`, `bar`, `address`); `address` is
  nullable; `importance` stays server-side.
- **No SQL injection:** the MATCH string is built only from alphanumeric
  tokens (so `"`, `*`, `(`, `:`, `NEAR`, `OR` cannot appear) and bound as
  `?1`; the limit is an integer param. The path comes from a validated id,
  canonicalised under `packs/` — the query string cannot choose a file.

## UI (`web/map/index.html`)

A `<form role="search">` above the offline panel: labelled `type="search"`
input (`autocomplete="off"`, `spellcheck="false"`), a `<ul>` of
`<li><button type="button">` rows (name, `kind · address`), and a
`role="status" aria-live="polite"` line. Buttons give Tab/Enter/Space for
free; add ArrowDown/ArrowUp between rows and Escape to clear and refocus the
input — no ARIA combobox needed.

- Debounce 180 ms, minimum 2 characters, cancel the previous `fetch` with
  `AbortController`, ignore stale responses.
- Activate: `handle.getMap().flyTo({center:[lon,lat], zoom: ≥15})`, set
  `aria-current="true"`, keep the list open. A per-row **Route** button writes
  `lat, lon` into the existing route panel `to` input and focuses it.
- States: panel hidden unless `/map/config` reports `packInstalled` and
  `searchInstalled`; no hits → "No places match"; 503 → "Search is unavailable
  for this region" with results cleared. Offline is the normal case, so no
  banner. Never persist or log the query; CSP is unchanged.
- `window.__pinpointSearch = { input, results, search }` for the UI test,
  matching `__pinpointRoute`. No marker API in v1; `flyTo` is enough and
  cannot collide with friend pins.

## Multi-region

v1 searches only the selected region. The "no hits, then try a nearby installed
region" fallback is deferred: BM25 scores across databases are not comparable,
border towns legitimately exist in both packs, misses would double latency, and
the region picker already switches packs. v2: when the map center leaves the
active bbox and the selected region returns zero hits, query the nearest
installed region and badge each row with its region id. Also deferred: result
bounding boxes (not in the schema; needs a rebuild), reverse geocode,
connection caching, per-result markers.

## Tests

1. `src/offline_search.rs` unit, temp DB with 3 places (Chișinău 0.9, Bălți
   city 0.9, Bălți bar 0.1): `balti` returns the city first, `balt` prefix
   matches, `Bălți`/`Chisinau`/`кишин` fold, `limit=1`, empty and
   punctuation-only → `[]`, and `balti" OR 1=1 --`, `NEAR(`, `*` add nothing
   and never error.
2. HTTP test (mobile map server, temp `packs/moldova/`): 200 shape and order;
   `limit=999` → ≤50; `region=../../Cargo.toml` → 400; unknown → 404; index
   deleted → 503 `SEARCH_UNAVAILABLE`; JSON omits `importance`.
3. Installer: third artifact lands in `packs/<id>/`, `search_installed` flips
   true, a bad hash keeps the previous file and reports `failed`.
4. Browser `web/map/test/search-ui.mjs` (Puppeteer — the repo has no
   Playwright): type `balti`, two rows with the city first, ArrowDown+Enter
   centres `window.__pinpointPreview.map`, Escape clears, `zzz` shows
   "No places match", hidden without a pack, no console errors, no external
   requests. Keep the 2-place WASM smoke green.
5. Gate: `cargo test --locked` picks up 1–3; add `search-ui.mjs` to
   `scripts/v2/check.ps1`'s `$webTests`.

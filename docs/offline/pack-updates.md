# Multi-region packs: update, delete, rollback

**Status:** Proposed · 2026-11-17 · extends
[`2026-11-17-global-maps-design.md`](../superpowers/specs/2026-11-17-global-maps-design.md)
§5–6 and `docs/offline/moldova.md`. Rust only; no code here.

```text
packs/<id>/                  live: installed.json + installed files
packs/.previous/<id>-<ver>/  one retained version, the rollback source
packs/.staging/<id>/         in-flight build, never served
packs/.trash/<id>-<ver>/     a rolled-back version awaiting deletion
```

`packs/<id>/installed.json` (schema 1) is the only record of what is live:
`{schema, region, version, installed_at, source: "catalog"|"legacy",
files: [{role, name, bytes, sha256}]}`. Ids are `[a-z0-9-]{1,64}`, so dot
directories cannot collide with a region.

**Versioning is plain string equality** between `catalog.regions[].version` and
`installed.json.version` — no semver, no ordering. Equal, every file present at
`installed_bytes`, hash as written → installed, no update offered. Unequal →
update offered, even when the catalog is *older* than what is installed (a
re-published catalog is still "Update"; v1 has no downgrade logic). Files
without `installed.json` get `version: null`: unversioned, update offered, not
rollbackable. Update and reinstall are the same job — only the wording and the
retained previous version differ. Keeping exactly one previous version lets a
bad release be undone with no 139 MB re-download; the next update prunes it and
`can_rollback` becomes false.

## Update sequence (v1)

1. **Preflight** — region in the catalog, no job running,
   `free_space(packs_dir) >= Σ installed_bytes + 16 MiB`, else `507` with zero
   bytes fetched. `installed_bytes`, not `transfer_bytes`: the graph is 45 MB
   on the wire, 111 MB at rest.
2. **Stage** — each file to `.staging/<id>/<name>.part`, checking transfer
   length + transfer SHA-256 and installed length + installed SHA-256 (existing
   `offline_install.rs` logic), `fsync`, rename inside staging. Any failure
   deletes `.staging/<id>` and sets `error`. The live region is never touched,
   so *a graph failure after the basemap verified leaves the old pack fully
   usable*. Files are never renamed into the live dir one at a time — that
   would leave a PMTiles archive and graph from different versions.
3. **Seal** — write `.staging/<id>/installed.json` last; it marks the staged
   dir complete.
4. **Commit** — gate `/packs/<id>/*` (`503`, `Retry-After: 1`), rename live →
   `.previous/<id>-<oldver>`, rename `.staging/<id>` → live, invalidate the
   `LocalRouter` cache, re-read `/map/config`, ungate.
5. **Clean** — prune older `.previous/<id>-*`, best effort; Windows may refuse
   and the startup sweep gets it. Never fails the job.

A crash between the two commit renames leaves no live dir plus a `.previous`
backup; startup recovery renames the newest one back. **Rollback** is that same
primitive behind `POST /map/offline/rollback`: gate, live → `.trash`, newest
`.previous` → live, invalidate, ungate, delete the trash. It trusts the
retained `installed.json`, so no network and no re-hash. v1 never auto-updates,
so a bad release cannot reinstall itself.

## Delete

`POST /map/offline/delete {region}` → `409` while any job (download, migration,
delete, rollback) targets that region; those paths rename the same directory.
Otherwise gate serving, remove `packs/<id>` and its `.previous/*`, ungate.
Only that region goes: sibling ids, other `.staging` entries and the Monaco
fixture are untouched. The response reports declared `freed_bytes` from
`installed.json` plus measured `free_bytes` after. Deleting the active region
falls `/map/config` back to the next installed region, then the fixture.

**Legacy migration** runs before the first status/config and takes the same job
lock, so it cannot race a delete or update of `moldova`. It *moves* (never
copies) root-level `moldova.pmtiles` and `moldova.graph.bin` into
`packs/moldova/` and synthesizes `installed.json`, claiming the catalog version
only when both declared sizes match, else `version: null`. If `packs/moldova/`
already exists it merges nothing — it logs and leaves the root files, which a
later delete must not assume are gone. The moves are renames with the same
crash window, so startup recovery retries migration when `<id>/` is missing and
root files are present.

## Space, metering, resume

Preflight is the only space gate and uses declared `installed_bytes`; an update
needs the full new version on top of the retained old one. `free_space` is a
`platform.rs` helper (`statvfs` / `GetDiskFreeSpaceExW`) — no new crate.
Metered networks are UI-only in v1: `/map/catalog` carries `transfer_bytes` so
the page warns before `POST`; Android can add `isActiveNetworkMetered` later
through the existing Java bridge. Resume is not supported and must not be
faked: the graph is a gzip stream and hashes cover the whole file, so `.part`
files die on failure and at job start, and startup sweeps `.staging/*` and
pruned `.previous/*`. Only a sealed staging dir already matching the target
version skips the download.

## Concurrency and Windows

One job per process (`JobState.running`), any region; a second request is `409`
plus status. Other regions stay readable, and the target serves from the live
dir until commit — the gate is per-region and covers just the rename window.
The routing graph never blocks a rename: `LocalRouter` loads it into memory and
closes the handle, then reloads on length+mtime change, hence the explicit
invalidate after a swap (length+mtime can collide on coarse-timestamp
filesystems). The PMTiles archive is the opposite — `serve_pack_named` opens it
per request and streams it, and on Windows a **directory** rename fails while
any handle is open inside, even though a file rename usually survives
`FILE_SHARE_DELETE`. So: gate, retry the rename with short backoff
(~10 × 50 ms), and on final failure leave the live dir in place, keep the
sealed staging dir, return `region_in_use`; the next attempt re-verifies it and
swaps without re-downloading.

## API and status additions

```text
GET  /map/catalog          regions[] += {version, transfer_bytes, installed_bytes,
                            installed: {version, bytes, verified, can_rollback}|null,
                            update_available, fits}
POST /map/offline/download {region}   202 | 404 | 409 | 507
POST /map/offline/delete   {region}   200 {freed_bytes, free_bytes} | 409
POST /map/offline/rollback {region}   202 | 404 | 409
GET  /map/offline/status   += {region, version, installed_version, previous_version,
                            phase, free_bytes, error: {code, message}|null}

phase: idle | checking | downloading | verifying | swapping | cleaning
       | installed | failed
code:  insufficient_space | verify_failed | http | region_in_use | io
       | no_previous_version
```

`version` is the job target, `installed_version` what is live; they differ only
during a job or after a failed commit — what the UI needs for "update failed,
v1 still active". `FileStatus` keeps its shape with `region` added.

## Test matrix

| # | Case | Asserts |
|---|---|---|
| 1 | Fresh install | files + `installed.json`, `installed`, `installed_version == catalog.version` |
| 2 | Update v1→v2 | live bytes hash to v2, `.previous/<id>-v1` present, `previous_version` set |
| 3 | Graph hash fails after basemap verified | live dir byte-identical to v1, no `.staging`, `verify_failed` |
| 4 | Crash between commit renames | startup recovery restores v1, never a half pack |
| 5 | Rollback after a bad update | live is v1, v2 pruned, `can_rollback: false` after |
| 6 | Delete active region | `freed_bytes > 0`, `free_bytes` grows, config falls back, sibling intact |
| 7 | Delete/rollback/second download during a job | `409`; range during swap → `503`, then `206` |
| 8 | Free space below requirement | `507`, no `.staging`, zero bytes transferred |
| 9 | v2.3.0 legacy layout | no network, version only on size match, delete then works |
| 10 | Range response open during update (Windows) | retries, then success or `region_in_use` with live intact |

Not in v1: byte-range resume, delta updates, more than one retained version,
automatic updates, `parts` for assets over 2 GiB.

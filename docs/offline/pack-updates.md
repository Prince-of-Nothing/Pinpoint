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

**Versioning is plain string equality** between catalog `version` and
`installed.json.version` — no semver, no ordering. Equal, every file present at
`installed_bytes` with the hash written at install → installed, nothing
offered. Unequal → update offered, even when the catalog is *older* than what
is installed (a re-published catalog still reads "Update"; v1 has no downgrade
logic). No `installed.json` → `version: null`, unversioned: update offered, not
rollbackable. Update and reinstall share one job; only the wording and the
retained version differ. Exactly one previous version is kept, so a bad release
is undone without a 139 MB re-download; the next update prunes it and
`can_rollback` goes false.

## Update sequence (v1)

1. **Preflight** — catalog entry, no running job,
   `free_space >= Σ installed_bytes + 16 MiB`, else `507` and nothing fetched.
   `installed_bytes`, not `transfer_bytes` (graph: 45 MB wire, 111 MB at rest).
2. **Stage** — each file into `.staging/<id>/<name>.part` with transfer and
   installed length + SHA-256 checks (existing logic), `fsync`, rename inside
   staging. A failure deletes `.staging/<id>`; the live dir is untouched, so *a
   graph failure after the basemap verified leaves the old pack usable*. Never
   rename files into the live dir one by one — that leaves a PMTiles archive
   and graph from different versions.
3. **Seal** — `installed.json` written last; it marks staging complete.
4. **Commit** — gate `/packs/<id>/*` (`503`, `Retry-After: 1`), live →
   `.previous/<id>-<oldver>`, `.staging/<id>` → live, invalidate `LocalRouter`,
   re-read `/map/config`, ungate.
5. **Clean** — prune older `.previous`, best effort (Windows may refuse; the
   startup sweep gets it). Never fails the job.

A crash between the two renames leaves no live dir plus a backup; startup
recovery renames the newest `.previous` back. **Rollback** reuses that: gate,
live → `.trash`, newest `.previous` → live, invalidate, ungate, delete the
trash — no network, no re-hash. v1 never auto-updates, so a bad release cannot
reinstall itself.

## Delete

`POST /map/offline/delete {region}` → `409` while any job targets that region
(download, migration, delete, rollback) — they rename the same directory. Else
gate serving, remove `packs/<id>` and its `.previous/*`, ungate. Only that
region goes: siblings, other `.staging` entries and the Monaco fixture are
untouched. Returns declared `freed_bytes` plus measured `free_bytes`. Deleting
the active region falls `/map/config` back to the next installed region, then
the fixture.

**Migration** runs before the first status/config on the same job lock, so it
cannot race a delete or update of `moldova`. It *moves* the root-level
`moldova.pmtiles` and `moldova.graph.bin` into `packs/moldova/` and synthesizes
`installed.json`, claiming the catalog version only when both declared sizes
match, else `version: null`. If `packs/moldova/` exists it merges nothing — it
logs and leaves the root files, which a later delete must not assume are gone.
The moves are renames with the commit crash window, so startup recovery retries
when `<id>/` is missing and root files exist.

## Space, metering, resume

Preflight is the only space gate and uses declared `installed_bytes`; an update
needs the new version on top of the retained old one. `free_space` is a
`platform.rs` helper (`statvfs` / `GetDiskFreeSpaceExW`), no new crate. Metered
networks are UI-only in v1: `/map/catalog` carries `transfer_bytes` so the page
warns before `POST`; Android can add `isActiveNetworkMetered` later through the
Java bridge. No resume: gzip and whole-file hashes make `.part` bytes
worthless, so they die on failure and at job start, and startup sweeps
`.staging/*`, `.trash/*` and pruned `.previous/*`. Only a sealed staging dir
already matching the target version skips the download.

## Concurrency and Windows

One job per process, any region; a second request is `409` plus status. Other
regions stay readable, and the target serves from the live dir until commit —
the gate is per-region and covers just the rename window. The graph never
blocks a rename: `LocalRouter` loads it into memory, closes the handle and
reloads on length+mtime change, hence the explicit invalidate (length+mtime can
collide on coarse timestamps). PMTiles is the opposite — opened per request and
streamed, and on Windows a **directory** rename fails while a handle is open
inside it, though a file rename usually survives `FILE_SHARE_DELETE`. So: gate,
retry the rename (~10 × 50 ms), and on final failure leave the live dir, keep
the sealed staging dir and return `region_in_use`; the next attempt re-verifies
it and swaps without re-downloading.

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
v1 still active". `fits` is precomputed from `free_bytes` so the page needs no
disk API of its own. `FileStatus` keeps its shape with `region` added.

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

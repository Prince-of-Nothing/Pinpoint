# Catalog-driven pack installer — security threat model

**Status:** Proposed, v1 · **Date:** 2026-11-17
**Scope:** `src/region_catalog.rs` + multi-region `src/offline_install.rs`
(design `2026-11-17-global-maps-design.md`), both loopback map servers, the map
page. Out of scope: account/auth (`security-architecture.md`), OpenFreeMap.
**Assets:** `packs/` (what the map renders and routes on), disk, the process's
network position, user trust in the rendered map.
**Trust boundary:** `catalog.json` and every asset are attacker-reachable. Only
the bundled fallback catalog is trusted; trusting a hash *because the catalog
said so* is not trust (§3).

## 1. Untrusted remote catalog

| # | Threat | Mitigation (v1) | Test |
|---|---|---|---|
| C-01 | `url` at `http://127.0.0.1`, `169.254.169.254`, `10/8`, `::1` or a LAN host → SSRF from the app's network position | https only; host allowlisted (`github.com`, `objects.githubusercontent.com`, or an operator mirror); reject userinfo, fragments, non-443 ports, IP literals; resolve DNS, reject non-global addrs, connect to the checked addr | `catalog_rejects_loopback_private_and_ip_literals`, `resolved_address_is_rechecked` |
| C-02 | Redirect to another host, or https → http | `redirect::Policy::custom`: ≤3 hops, same allowlisted host, https only, final URL revalidated | `redirect_off_host_and_downgrade_refused` |
| C-03 | Oversized body, deeply nested JSON | Hard 1 MiB cap on content-length **and** running total; 64 regions / 32 files per region; parse only after the cap | `oversized_catalog_refused` |
| C-04 | Duplicate `id` or duplicate `(region, role)` shadowing a pinned entry | Reject the whole catalog on any duplicate id, role, or `installed_name`; never merge | `duplicate_ids_reject_the_catalog` |
| C-05 | `id`/names with `..`, `/`, `\`, `:` or NUL → write outside `packs/` | `id` is `^[a-z0-9-]{1,64}$`, names `^[a-z0-9][a-z0-9.-]{0,127}$` without `..`; then `join` + `starts_with(canonical packs_root)` | `catalog_names_cannot_escape_packs` |
| C-06 | Hash downgrade: empty/short/uppercase hash skips the check | Require 64 lowercase hex for **both** hashes and positive sizes; delete the `if !expected_transfer.is_empty()` escape (`offline_install.rs:585`) | `catalog_rejects_missing_or_malformed_hashes` |
| C-07 | Rollback/replay: `main` is mutable, an old catalog re-serves a withdrawn region | Cache the accepted catalog with its `updated` + own SHA-256; refuse an older `updated`; record per-region `version` and never auto-downgrade; revalidate the cache on every load | `older_catalog_refused`, `cached_catalog_revalidated` |

## 2. Installer

| # | Threat | Mitigation (v1) | Test |
|---|---|---|---|
| I-01 | Partial file installed after crash | Keep `.part` + `sync_all` + same-dir atomic rename (`:595`); discard stray `.part`, never resume | `leftover_part_discarded` |
| I-02 | Hash/size mismatch, truncation, extra bytes | Existing transfer+installed hash, length, gzip ISIZE and post-trailer checks, for every catalog artifact | `wrong_hash_keeps_old_file`, `short_artifact_rejected` |
| I-03 | Symlink/hardlink at `packs/<id>/` or the target → write-through, or serve foreign bytes | `symlink_metadata` refusal on region dir + target; temp opened `create_new`; recanonicalize parent before rename; add the same refusal next to `serve_pack_named`'s existing canonicalize | `installer_refuses_symlinked_paths` |
| I-04 | Concurrent installs (2nd POST, 2 processes, restart mid-job) | In-process `Mutex` covers one server; add `packs/.lock` (`create_new`, stale-pid check) so a second process fails closed with 409 | `second_process_cannot_start_a_job` |
| I-05 | Disk exhaustion from a giant declared size | Cap 2 GiB/file and per catalog; preflight free space ≥ install + 10%; `ENOSPC` → clean failure, `.part` removed, old pack intact | `giant_declared_size_refused`, `insufficient_space_fails_before_writing` |
| I-06 | gzip bomb / header flood | Already bounded: `write_installed` caps output at `expected_bytes`, header at 1 MiB — a bomb can only fill the *declared* size, so I-05 is the real bound. Also reject declared ratio > 200:1 | `gzip_bomb_stops_at_declared_size` |
| I-07 | Corrupt file counted as installed (status checks length only, `:411`) | Accept for v1: hash at install, write sidecar `{size, mtime, sha256}` so `config` detects replacement | deferred |

## 3. Host compromise

GitHub account or asset replacement defeats every catalog hash — catalog and
assets share one trust domain.

- **v1:** hashes in `catalog.json` (stops corruption, truncation, CDN mix-ups,
  and an asset swapped without repo write access); bundled fallback with the
  v2.3.0 hashes, so a fully compromised catalog cannot cause unverified bytes;
  release tag in the URL must match the region `version`; refuse
  `PINPOINT_OFFLINE_BASE_URL` outside debug builds or an explicit opt-in (it
  currently redirects catalog *and* assets).
- **Defer:** signed catalog (Ed25519 manifest over the whole catalog, key
  pinned in the app) or TUF/`cosign` with root+targets roles. That is the only
  real defense against repo compromise; accepted residual risk until then.
- **Not feasible:** TLS pinning to `github.com`/`objects.githubusercontent.com`
  (shared CDN, rotating certs, mobile trust stores). Do not attempt; hashes are
  the integrity control.

## 4. Page boundary

The page is untrusted for all installer inputs.

- `POST /map/offline/download` takes **only** `{ "region": "<id>" }`; the server
  looks the id up in the validated cache (404 on miss) and uses the catalog's
  URL, sizes and hashes. Reject unknown JSON fields — no URL, name, size, hash
  or path from the page, ever.
- `GET /map/catalog` strips asset URLs; returns `installed` flags only.
- `/packs/:region/:name` resolves through the catalog: `region` from the
  installed set, `name` from that region's `installed_name` list, never a
  filesystem listing.
- Keep page CSP `connect-src 'self'`; the OpenFreeMap toggle must not add a host
  the installer uses.

## 5. Legacy migration (`packs/moldova.pmtiles` → `packs/moldova/`)

| Threat | Mitigation | Test |
|---|---|---|
| Race with a running job or open Range read | Hold the `.lock` file for the whole move; skip while a job runs | `migration_skipped_while_job_runs` |
| `EXDEV`, or Windows rename while the pack is open | `rename`, else copy → `sync_all` → rename → delete; destination-exists = success, never overwrite; one delayed retry on Windows, then give up without failing the install | `migration_copy_fallback`, `migration_does_not_overwrite` |
| Permissions/umask on the new dir | `create_dir_all` + explicit `0o700` dir / `0o600` files on Unix; on failure keep serving the legacy path | `migrated_files_not_world_readable` |
| Half-migrated (pack moved, graph not) | Resolve both sources first, move both or roll back; idempotent re-run | `partial_legacy_layout_retries` |
| Adopting an attacker-planted legacy file | Verify size + SHA-256 against the bundled Moldova entry before adopting; else leave unadopted | `migration_rejects_tampered_legacy_file` |

## 6. Test checklist (Rust)

1. URLs: http, loopback, private/link-local, IP literal, custom port, userinfo,
   foreign host, rebinding address.
2. Redirects: off-host, https→http, >3 hops.
3. Catalog: >1 MiB, >64 regions, duplicate id/role/name, bad charset, `..`,
   missing/short/uppercase hash, zero and >2 GiB sizes.
4. Rollback: older `updated` refused; tampered cache detected.
5. Installer: wrong transfer hash, wrong installed hash, short body, extra
   bytes, truncated gzip, bomb to declared size, ENOSPC preflight.
6. Filesystem: symlinked dir/target, `.part` leftover, concurrent start
   (in-process + lock file), atomic replace keeps the old pack.
7. Page: unknown JSON field rejected, unknown region 404, `/packs` traversal
   (`..`, `%2e%2e`, `\`, `:`), region/name not installed.
8. Migration: EXDEV, destination exists, Windows open file, partial layout,
   tampered legacy file, permissions.

## 7. v1 vs deferred, and the top 5

**Ship:** C-01…C-07, I-01…I-06, §3 pinning + bundled fallback + debug-gated
override, §4 id-only API, all of §5.
**Defer:** signed catalog / TUF, I-07 sidecar, delta downloads, mirrors,
`parts`. **Rejected:** cert pinning.

**Must fix before v1 ships**

1. **C-01/C-02** — https + release-host allowlist + no off-host or downgrade
   redirects on catalog URLs (`resolve_base_url_with` and reqwest's default
   redirect policy are both too permissive today).
2. **C-06** — mandatory 64-hex hashes on both transfer and installed bytes;
   remove the empty-`transfer_sha256` bypass.
3. **C-04/C-05** — strict id/name regexes, unique ids, and
   `starts_with(packs_root)` on every joined path.
4. **I-05/I-06** — per-file and per-catalog size caps plus a free-space
   preflight before the first byte.
5. **§4** — download takes a region id only; everything else resolved
   server-side from the cached catalog.

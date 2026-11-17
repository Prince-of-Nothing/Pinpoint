# Mobile pack budget — country packs v1

Status: Proposed · 2026-11-17 · Android catalog v1
(`docs/superpowers/specs/2026-11-17-global-maps-design.md`). MiB = 2^20 B.
Moldova is the only measured region; the others extrapolate its ratios, so
re-measure before publishing.

## Measured baseline

| Artifact | Transfer | Installed |
|---|---:|---:|
| `moldova.pmtiles` (z0–14) | 88.9 | 88.9 |
| `moldova.graph.bin` | 43.2 (gz) | 105.7 |
| **Pack** | **132.1** (UI: 139 MB) | **194.6** (UI: 204 MB) |

PMTiles bytes per zoom, measured from the archive's directories: z0–13
**30.6** MiB (saves 58.2, 65%), z0–12 **14.8** MiB (saves 74.1, 84%).
z14 alone is 58.2 MiB, 65% of the file, so plan with `z13 ≈ 0.35 × z14` and
`z12 ≈ 0.17 × z14`; dense countries skew the top level up.

## Render cost (PMTiles, maxzoom)

MapLibre GL JS in the WebView range-reads the local pack, so archive size does
not set render memory: on a 1080p mid-range phone, WebView + MapLibre 120–200
MiB, tiles 10–40 MiB, GPU and glyph atlases 30–80 MiB — 200–320 MiB, the same
for z12 and z14 because only visible tiles are resident. Lower maxzoom buys
storage and transfer, not RAM, and costs overzoom quality (z18 overzoom from
z12 is six levels of blur). A 1 GB+ PMTiles renders fine, since range reads
stay small, but downloads badly: one serialized job, no resume, minutes on
20 Mbps, 2× update headroom — so >1 GiB installed is WiFi-only with explicit
confirmation.

## Graph load memory and time

`graph.bin` is fully resident (no mmap): 24 B/node + 24 B/edge + 4 B/node
offsets, plus ~29 B/node of A* arrays (149.1 peak − 105.7 resident on Moldova).
Load is 0.265 s desktop warm; a 3× phone factor covers slower flash and CRC-32,
and route is 0.13–0.36 s for 134 km on desktop, 3–4× on a phone.

| Region | PBF | Graph (1.09× PBF) | Peak with A* | Load (phone) | Route (phone) |
|---|---:|---:|---:|---:|---:|
| Moldova (measured) | 97 | 106 | 149 | 0.8 s | 0.13–0.36 s (desktop) |
| Romania | 318 | 347 | 480 | 2.6 s | 1–3 s |
| Ukraine | 844 | 920 | 1.3 GiB | 7 s | 3–10 s |
| Germany | 4659 | 5.0 GiB | ~7 GiB | 38 s | 15–60 s |

Romania is the practical single-pack ceiling on a 6 GB phone. Ukraine and
larger do not fit a 4 GB phone once A* runs; mmap would drop the resident copy
but not the A* arrays.

Measured 2026-11-17 (Romania z13, `data-romania-20261117-44139439624`):
PMTiles **218,306,730 B**, installed graph **463,119,610 B**, gzip graph
**187,335,727 B**, transfer **405,642,457 B**, installed **681,426,340 B**.
That is within the transfer budget but above the 500 MiB installed target;
verify a route on a 6 GB device before promoting it as tier B.

## Search SQLite (not in native v1)

6.46 MiB for 36,913 places (183 B/place); scaled by PBF: Romania ~22 MiB,
Ukraine ~59 MiB, Germany ~325 MiB. SQLite never loads the file — v1 opens a
read-only connection per query with a ~2 MiB page cache — so the index is a
storage cost, not a memory one. Keep it out of the v1 download.

## Download and storage budget

- Transfer ≈ PMTiles + graph/2.45; installed ≈ PMTiles + graph (Moldova's
  gzip expansion is 2.45× on the graph).
- **Free-space guard:** fresh install ≥ installed × 1.15; an **update needs
  ~2× installed** (old pack + new `.part` + new file before the atomic
  rename), so guard on 2.2× installed. Moldova update: ~430 MiB free. Fail
  before the first byte, never with ENOSPC mid-write.
- GitHub's 2 GiB per-asset cap must be enforced by the build: Germany's z14
  PMTiles (~4.2 GiB) and even its gzipped graph (~2.03 GiB) exceed it.

## Recommended caps (v1)

| Tier | Installed | Graph | maxzoom | Device floor | Examples |
|---|---:|---:|---|---|---|
| A | ≤ 220 MiB | ≤ 120 MiB | 14 | 4 GB | Moldova (195 MiB) |
| B | ≤ 500 MiB | ≤ 380 MiB | 13 | 6 GB, WiFi | Romania (~450 MiB est) |
| C | ≤ 1 GiB | none | 12 | 4 GB | Ukraine basemap only |
| D | parts | part ≤ 380 MiB | 12 country, 13–14 per part | 4 GB | Germany, France, US |

Publish single-pack routing only if the **built** graph is ≤ 380 MiB and the
4 GB-device route test passes; otherwise ship the z12 basemap alone or split.
Cap any region at ~1 GiB installed.

Large-country strategy: one z12 country basemap for browse; routing in parts
whose graph is ≤ 380 MiB (≈ ≤ 350 MiB PBF), chosen by measured graph size, not
admin borders (NRW at 875 MiB PBF and Bayern at 819 already exceed it). Parts
route independently; cross-part routing and a z8–10 macro-region browse pack
are later.

## What to measure on a real device (pass/fail)

4 GB RAM, 1080p, UFS 2.x, Android 13+; app + WebView combined.

| Metric | Method | Pass | Fail |
|---|---|---|---|
| Peak RSS (browse + route) | `dumpsys meminfo` after pan/zoom, then a route | ≤ 450 MiB | > 600 MiB or LMK kill |
| First render with pack | cold start → first styled frame | ≤ 3 s | > 5 s |
| Graph load | off-main-thread timer, no ANR | ≤ 2 s (≤ 6 s for 380 MiB) | > 10 s or ANR |
| Route Chișinău→Bălți | in-app timer, 10 runs | ≤ 1.5 s | > 3 s |
| Install | 139 MB on 20 Mbps + hash + rename | ≤ 3 min, progress ≤ 1 s | > 6 min, no progress |
| Update free space | peak bytes during swap | ≤ 2.2× installed | > 2.5× or old pack lost |

Sources: `docs/offline/moldova.md`, `docs/offline/native-routing.md`,
`tools/native-router/README.md`, `docs/offline/region-inventory.md`.

# ODbL and attribution for Pinpoint map packs

**Status:** normative for `pinpoint-maps` releases · 2026-11-17

Applies to every file in `dist/offline/<region>/` and every
`data-<region>-<YYYYMMDD>` release.

## Licence stack

| Material | Licence | Obligation |
|---|---|---|
| OSM data (`.osm.pbf`) | ODbL 1.0 | credit + ODbL notice; derivative databases stay ODbL |
| Geofabrik extract | ODbL 1.0, no extra terms | credit the extract; record URL, download date, SHA-256 |
| Planetiler (`planetiler-openmaptiles`) | Apache-2.0 | licence/NOTICE in docs only; no effect on data |
| OpenMapTiles schema/design | code BSD-3; cartography CC-BY 4.0 | visible `© OpenMapTiles` link on any browsable map; notice in `ATTRIBUTION.md` |
| Protomaps basemap/style (not used today) | code BSD-3, visual design CC0 | attribution only if we ever ship a Protomaps style |
| OpenFreeMap (online toggle only) | service MIT; data OSM + OMT schema | show `OpenFreeMap © OpenMapTiles Data from OpenStreetMap` |

Geofabrik adds no licence terms of its own; its download pages carry
"Data processed by Geofabrik GmbH and created by OpenStreetMap Contributors |
License: ODbL 1.0", so naming the extract is enough.

## Where the notice must appear

Canonical map credit, links included:

```text
© OpenStreetMap contributors · OpenMapTiles · ODbL 1.0
```

| Surface | Requirement |
|---|---|
| Map view (app, web) | corner credit, permanent, or collapsible only if reachable from an (i)/About control |
| About / credits screen | same credit plus ODbL link and full notice |
| `ATTRIBUTION.md` in the pack | full text: OSM, ODbL + URI, Geofabrik extract, OpenMapTiles, source date, derivative-DB notice |
| `manifest.json` | `attribution`, `license`, `license_url` (already present); add `source_date` |
| `catalog.json` | per-region attribution + licence URL (catalog is the discovery surface) |
| PMTiles archive metadata | `attribution` + ODbL URI inside the archive (set with Planetiler `--attribution`) |
| Release notes | credit line, ODbL link, source date, list of derivative databases |
| GPX / GeoJSON exports | already carry `© OpenStreetMap contributors (ODbL)`; keep it |

## Share-alike: what the packs are

`*.pmtiles`, `*-search.sqlite`, `*-valhalla.tar` and `*.graph.bin` are all
**Derivative Databases** (substantial extraction into a new database), not
Produced Works. Public use triggers ODbL §4.4; §4.6 requires offering the
database, or an alteration file, free over the internet. Publishing them as
public release assets satisfies §4.6. Therefore:

- never hand a pack to selected users while keeping the database closed;
- never merge non-ODbL data into a pack (§4.4d);
- don't delete a release while shipped clients may still fetch it (§4.6 offer);
- the rendered map is a Produced Work (§4.5b) — attribution only, no
  share-alike on app code, UI or style.

## Blockers

1. **Installed packs carry no licence.** `src/offline_install.rs` installs only
   `pmtiles` + `graph.bin`; `ATTRIBUTION.md` stays on the server. A recipient
   of the derivative database gets no §4.2(b) notice in the database or its
   documentation. Fix in the catalog/installer: add `ATTRIBUTION.md` to the
   region's `files` (installed under `packs/<region>/`) and keep the static
   credits screen in the app.
2. **PMTiles metadata is unverified.** If the archive lacks `attribution` and
   the ODbL URI, the only notice is server-side, which is not "in the
   database". Check one asset with `pmtiles show` before the first release.

## Release checklist (`pinpoint-maps`, `build-region.yml`)

- [ ] Build from the pinned PBF SHA-256 and pinned image digests; record extract
      URL, download date (UTC) and PBF SHA-256.
- [ ] `ATTRIBUTION.md` generated from `scripts/offline/ATTRIBUTION.md` plus
      Geofabrik, OpenMapTiles, source date and derivative-DB notice; `verify.ps1`
      passes (`OSM`, `ODbL`, licence URL present).
- [ ] `manifest.json` carries attribution, `ODbL-1.0`, licence URL, source date.
- [ ] PMTiles built with `--attribution "© OpenStreetMap contributors"`; metadata
      checked.
- [ ] All assets + `ATTRIBUTION.md` uploaded as **public** release assets (no
      login, no expiring links).
- [ ] Release body: credit line, ODbL link, source date, list of shipped
      derivative databases.
- [ ] `catalog.json` regenerated with attribution and licence URL per region.
- [ ] Do not build packs from OpenFreeMap tiles: its ToS caps offline downloads
      at 50 MB / 3,000 tiles and 4 parallel requests — far below a country pack.

## In-app credits checklist

- [ ] Map corner shows the canonical credit on every map, online and offline.
- [ ] About screen lists OSM/ODbL, OpenMapTiles, and Geofabrik per active pack.
- [ ] Pack row shows the source date and the ODbL link.
- [ ] Online toggle shows the OpenFreeMap line while it is active, and hides it
      when the local pack style is in use.

## Sources

- OSM copyright and ODbL: https://www.openstreetmap.org/copyright ·
  https://opendatacommons.org/licenses/odbl/1-0/
- OSMF attribution guidelines (map, database, search, routing):
  https://osmfoundation.org/wiki/Licence/Attribution_Guidelines
- Geofabrik download server: https://download.geofabrik.de/ ·
  https://download.geofabrik.de/technical.html
- OpenMapTiles licence: https://github.com/openmaptiles/openmaptiles/blob/master/LICENSE.md
- Planetiler licence: https://github.com/onthegomap/planetiler/blob/main/LICENSE
- Protomaps basemap licence and downloads:
  https://github.com/protomaps/basemaps/blob/main/LICENSE.md ·
  https://docs.protomaps.com/basemaps/downloads
- OpenFreeMap attribution and ToS: https://openfreemap.org/ ·
  https://openfreemap.org/tos/

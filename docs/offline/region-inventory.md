# Offline region inventory

Geofabrik extract sizing for Pinpoint offline packs. Sizes are `Content-Length`
from a live `HEAD` on 2026-11-17 and drift daily; re-measure before a release.
1 MiB = 2^20 B. GitHub caps one release asset at 2 GiB = 2048 MiB.

A pack is two assets: `<region>.pmtiles` (basemap) and `<region>.graph.bin`
(routing). Measured on Moldova, PMTiles is 0.92x the PBF and the graph 1.09x,
so the **largest asset is ~1.09x the PBF** and the whole pack ~2.2x. Every
`>` below means PBF MiB.

## Priority regions

| Region | Extract | PBF MiB | bbox W,S,E,N | Cap risk |
|---|---|---:|---|---|
| Moldova | `europe/moldova-latest.osm.pbf` | 97 | 26.61,45.46,30.19,48.49 | small |
| Bulgaria | `europe/bulgaria-latest.osm.pbf` | 169 | 22.35,41.23,29.19,44.22 | small |
| Serbia | `europe/serbia-latest.osm.pbf` | 231 | 18.81,42.23,23.01,46.19 | small |
| Hungary | `europe/hungary-latest.osm.pbf` | 315 | 16.11,45.73,22.91,48.59 | small |
| Romania | `europe/romania-latest.osm.pbf` | 318 | 20.24,43.61,30.28,48.27 | small |
| Greece | `europe/greece-latest.osm.pbf` | 327 | 18.97,34.59,29.66,41.75 | small |
| Slovakia | `europe/slovakia-latest.osm.pbf` | 332 | 16.83,47.73,22.57,49.62 | small |
| Turkey | `europe/turkey-latest.osm.pbf` | 624 | 25.52,35.72,44.86,43.07 | medium |
| Austria | `europe/austria-latest.osm.pbf` | 778 | 9.53,46.37,17.16,49.02 | medium |
| Ukraine | `europe/ukraine-latest.osm.pbf` | 844 | 22.13,44.01,40.24,52.39 | medium |
| Czechia | `europe/czech-republic-latest.osm.pbf` | 909 | 12.08,48.54,18.86,51.06 | medium |
| Spain | `europe/spain-latest.osm.pbf` | 1433 | -9.78,35.26,5.10,44.15 | large |
| Poland | `europe/poland-latest.osm.pbf` | 2022 | 13.99,48.99,24.16,55.23 | large |
| Italy | `europe/italy-latest.osm.pbf` | 2149 | 6.60,35.08,19.12,47.10 | large |
| United Kingdom | `europe/united-kingdom-latest.osm.pbf` | 2182 | -14.86,49.42,2.64,61.14 | large |
| Japan | `asia/japan-latest.osm.pbf` | 2456 | 122.56,20.08,154.47,45.82 | large |
| Germany | `europe/germany-latest.osm.pbf` | 4659 | 5.86,47.27,15.05,55.15 | large |
| France | `europe/france-latest.osm.pbf` | 4880 | -6.94,41.24,9.90,51.43 | large |
| Canada | `north-america/canada-latest.osm.pbf` | 6199 | -141.78,41.66,-44.18,85.04 | large |
| United States | `north-america/us-latest.osm.pbf` | 11704 | -180,15.92,180,72.99 | large |

All URLs are `https://download.geofabrik.de/` + the path shown. bboxes are the
index geometry envelope: France is mainland only (overseas are separate
features), Canada spans the Arctic, and the US hits -180/180 because Alaska
crosses the antimeridian, so clamp it for a web map.

Ukraine's extract is named "Ukraine (with Crimea)" and France's admin split is
still the pre-2016 one (27 regions, `alsace` separate from `champagne-ardenne`),
which matters if you key packs on ISO codes. Spain's index geometry is a single
polygon that omits the Canary Islands, so do not derive a Spain bbox from it.

## Getting the full list

```
https://download.geofabrik.de/index-v1.json
```

A GeoJSON `FeatureCollection`, 554 features on 2026-11-17, no `size` field:
`properties` is `id`, `parent`, `name`, `urls.pbf`, sometimes
`iso3166-1:alpha2`; `geometry` is a `MultiPolygon` you can reduce to a bbox.
There is no `index-v1.yaml` (404). Size comes from a `HEAD` on `urls.pbf`:

```bash
curl -sI https://download.geofabrik.de/europe/moldova-latest.osm.pbf | grep -i content-length
```

Roots (no `parent`) are continents: `africa` 56 children, `europe` 53,
`north-america` 62, `asia` 39, `australia-oceania` 23, `south-america` 12,
`central-america` 11, `russia` 10, `antarctica` 0. Depth is not uniform — the
US is the trap: states are 53 features with `parent: north-america` and
`id: us/california`, not `parent: us`. Group by `id` prefix or by the
`/us/` segment of `urls.pbf`; every other nested feature has `parent` set to the
country id (`bayern` -> `germany`). 53 features are nested overall.

Country splits available today: Germany 16 Länder, France 27 régions, Spain 18
(incl. Ceuta/Melilla), Poland 16 voivodeships, Italy 5 macro-regions, UK 4
(England, Scotland, Wales + overseas), Japan 8, Canada 13, US 53. Turkey,
Romania and Ukraine have no split. Continents exist whole (`europe-latest` is
33,829 MiB — never a pack).

Per extract there is also `<name>.osm.pbf.md5` and a `<name>-updates/` directory
of minutely/hourly/daily diffs, which is how you refresh a pack without a
re-download; pin the md5 in the manifest.

## Size classes and maxzoom

| Class | PBF MiB | Est. largest asset | maxzoom | Action |
|---|---|---|---|---|
| small | <= 500 | <= 545 | 14 | single pack, no risk |
| medium | 500 - 1300 | <= 1417 | 13 | single pack, measure before publishing |
| large | > 1300 | > 1417 | 12-13 | tight at 1300-1879; split past 1879 |

The cap is hit at ~1879 MiB of PBF, so 1300-1879 is tight-but-fits (headroom
under 30%) and only past 1879 is a split mandatory. Lowering maxzoom does not
fix a large region: it shrinks the PMTiles only, while `graph.bin` tracks
road-km and stays ~1.09x the PBF. Germany at z13 is a ~1.1 GiB basemap next to
a 5 GiB graph — the graph is what must be split.

One level of split is always enough: the largest sub-regions are England 1636,
California 1275, British Columbia 1192, Québec 1113, Nordrhein-Westfalen 875
and Bayern 819 MiB, all under 1879. Italy's macro-regions are 206-598,
Germany's smallest 96 (`berlin`), France's 33 (`corse`).

## Recommended v1

Ship the 7 small + 4 medium single-file packs — 4,944 MiB of PBF, ~10.6 GiB
across 22 assets, largest asset ~990 MiB:

- Moldova, Bulgaria, Serbia, Hungary, Romania, Greece, Slovakia (small, z14)
- Turkey, Austria, Ukraine, Czechia (medium, z13)

Trim to the first line only (1,789 MiB PBF, ~3.8 GiB, largest asset 362 MiB) if
v1 needs to be one sitting of uploads; the medium set is a clean v1.1.

Defer the rest to v1.2. Spain (1433) still fits whole and can ship with tight
headroom; the eight past 1879 must be split: Germany 16, France 27, Poland 16,
Italy 5, UK 4, Japan 8, Canada 13, US 53. Keep the country basemap at z12 and
split only the graph. Poland first — it is the cheapest split that is actually
over the line.

Not checked: no pack was built for any region above, so the 1.09x/0.92x
multipliers come from Moldova alone; sizes are point-in-time and exclude the
`-updates/` growth between releases.

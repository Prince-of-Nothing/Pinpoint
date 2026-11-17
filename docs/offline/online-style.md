# Pinpoint treatment for the OpenFreeMap online basemap

**Status:** visual spec for the opt-in online style · 2026-11-17 · Pairs with
`2026-11-17-global-maps-design.md` §6, `openfreemap.md`, `odbl-attribution.md`.
Layer facts checked live against `https://tiles.openfreemap.org/styles/liberty`.

## Theme being ported

Pinpoint's offline map is forest/amber: cream paper, green land, muted blue
water, amber major roads, white minor roads; dark mode is `#11161d` with
`#c77b3a` roads. Tokens live in `THEME_PAINT` (`web/map/map.js`), route colours
in `ROUTE_PAINT`, page chrome in `preview.css`.

`THEME_PAINT` targets **our** `style.json` ids. Only `background` and `water`
exist in Liberty; `landcover`, `landuse-green`, `landuse-urban`, `waterway`,
`boundary-country`, `boundary-region`, `rail`, `road-major*`, `road-medium`,
`road-minor`, `road-path` are ours and must be re-pointed, not reused.

## Liberty layer ids (live, 2026-11-17)

| Group | Ids |
|---|---|
| background | `background`, raster `natural_earth` (z0–6 hillshade) |
| water | `water`, `waterway_{river,other,tunnel}`, `water_name_*` |
| land | `park`, `park_outline`, `landcover_{wood,grass,ice,wetland,sand}`, `landuse_{residential,pitch,track,cemetery,hospital,school}` |
| roads | `road_{motorway,trunk_primary,secondary_tertiary,minor,service_track,link,motorway_link,path_pedestrian}`, `road_area_pattern`, `road_major_rail`, `road_transit_rail` (+`_hatching`), plus **`tunnel_*` / `bridge_*` twins of every road id** |
| casings | `*_casing` for every road/tunnel/bridge id |
| boundaries | `boundary_2`, `boundary_3`, `boundary_disputed` |
| labels | `label_*`, `highway-name-*`, `highway-shield-*`, `road_shield_us`, `poi_r1/r7/r20`, `poi_transit`, `airport`, `road_one_way_arrow*` |
| buildings | `building`, `building-3d` |

Discover at runtime, never trust a hardcoded list alone:

```js
map.getStyle().layers.map(({ id, type, 'source-layer': sl }) => ({ id, type, sl }));
// roads: /^(bridge|tunnel)_road_|^road_/ · casings: /_casing$/ · labels: /^(label_|poi_|airport$|highway-name-)/
```

Keep the existing `if (map.getLayer(id))` guard: a renamed id must skip, not
throw.

## Light / dark

One style, two paint tables. Do **not** switch to `/styles/dark` or `/fiord`:
different ids drop every override and the raster hillshade stays light.
`setTheme` stays `setPaintProperty` over the group table (same
`[property, value]` shape plus an id list), then `paintRoute()`. Reapply on
`style.load`, not `load`: `applyTheme()` bails while `!isStyleLoaded()`.

`setStyle` is only for the offline↔online toggle. It drops custom sources and
layers, so re-add them in `style.load` from the retained `pendingLocations` /
`pendingRoute`, then `applyTheme()`. Page backgrounds (`#f4f1ea` / `#11161d`
in `MAP_CSS`) already match the style `background`, so no white flash. Planet
tiles stop at z14 while `createMap` allows 18 — overzoom is expected.

## Typography and glyphs

Liberty uses `Noto Sans Regular`, `Noto Sans Bold`, `Noto Sans Italic` from
`https://tiles.openfreemap.org/fonts/<stack>/<range>.pbf` (all three return 200;
`Noto Sans Medium` and `Open Sans` 404). A missing stack drops labels silently
and emits a MapLibre `error` event — the module routes every `error` to
`fail()`, so glyph/sprite 404s must be filtered or the page flips to "error".

Map labels stay Noto Sans: the map is cartography, the chrome keeps its serif
title and UI sans. Liberty prefers `name_en`, then `name`; the 79-language UI
does not localise map labels. Do not override `text-field`/`text-font` (layout
changes relayout the map and couple us to upstream naming); only `text-color`
and `text-halo-color`. Keep the halo at 1 px, text ≥ 11 px, and in dark swap the
halo to `#11161d` or every label gets a white box.

## Attribution

One DOM node, `.map-attribution`, bottom-right (bottom-left under 820 px), dark
translucent chip in both themes: `#f3f6f8` on `rgb(17 22 29 / 78%)` ≈ 12:1,
links `#a7e8d6` ≈ 11:1, so it holds over cream and dark tiles. Keep
`attributionControl: false` and swap text with the style:

- online: `OpenFreeMap © OpenMapTiles Data from OpenStreetMap` — links to
  openfreemap.org, openmaptiles.org, openstreetmap.org/copyright;
- offline: `© OpenStreetMap contributors · OpenMapTiles · ODbL 1.0`.

The privacy note belongs in the toggle card, not the credit chip. On mobile the
chip must clear the route panel (`bottom: 14px` vs `58px`), may wrap to two
lines, and is never hidden or dimmed below 0.8 opacity.

## Minimal override set

Ten entries; each is one paint property applied to a matched id group (~40 ids,
including `tunnel_`/`bridge_` twins). Light → dark:

| # | Ids | Property | Light | Dark |
|---|---|---|---|---|
| 1 | `background` | `background-color` | `#f4f1ea` | `#11161d` |
| 2 | `water` | `fill-color` | `#a9d3e0` | `#16384a` |
| 3 | `waterway_*` | `line-color` | `#8fc1d4` | `#245a70` |
| 4 | `park`, `landcover_wood`, `landcover_grass` | `fill-color` | `#d7e7cf` | `#1e2c25` |
| 5 | `landuse_residential` | `fill-color` | `#e7e4de` | `#20272f` |
| 6 | `road_motorway`, `road_trunk_primary`, `road_link`, `road_motorway_link` + twins | `line-color` | `#f3b562` | `#c77b3a` |
| 7 | `road_secondary_tertiary` + twins | `line-color` | `#f7d9a8` | `#b08b54` |
| 8 | `road_minor`, `road_service_track`, `road_path_pedestrian` + twins | `line-color` | `#ffffff` | `#4a515b` |
| 9 | every `*_casing` | `line-color` | `#c9c0b2` | `#0b0f14` |
| 10 | `label_*`, `highway-name-*`, `water_name_*`, `poi_*`, `airport` | `text-color` (+ halo in dark) | `#1c241f` / halo `#f4f1ea` | `#f0eadc` / halo `#11161d` |

Enough for cream paper, green land, muted water, amber trunk roads and one halo
pass. Entry 8 does nothing in light but is required in dark. Add
`building`/`building-3d` `fill-color: #20272f` only if dark screenshots show
bright blocks; they are the first visible leftover.

**Cannot change without forking:** the `natural_earth` raster (opacity/filter
only — hide it in dark if it reads as grey haze); sprite art (POI icons,
shields, one-way arrows, `wetland_bg_11` / `pedestrian_polygon` patterns) needs
a custom sprite; layer order, filters and zoom ranges are style structure, not
paint; the three glyph stacks above are the only ones served.

## Test and screenshot plan

- Pin the live style as `web/map/test/fixtures/liberty-style.json` and assert
  every id in the override table still exists. Runs offline in CI; earliest
  warning for upstream renames.
- Browser row (`smoke.mjs` house style, Puppeteer): toggle online, wait for
  `idle` (not `load`), assert `getPaintProperty('water','fill-color')` and one
  label/halo value per theme, assert every request host is
  `tiles.openfreemap.org` or local, and assert no `error` event fired.
- Screenshots `{light, dark} × {world z1.5, city z12, city with route}`: fixed
  camera, DPR 1, 1280×720 and 390×844 (mobile chip); save
  `online-<theme>-<view>.png`. Dark must be ≥ 25 % darker in mean canvas
  luminance and contain far fewer pure-white pixels — that catches
  un-overridden bridge/tunnel roads.
- Attribution: contrast ≥ 4.5:1; chip box inside the map frame at both widths
  and intersecting neither `.map-meta` nor `.route-panel`; text matches the
  online string; links have names.
- Manual: flip theme with a route drawn (pins and route survive), then
  online → offline → online; pan across a country border at z5.

## Top unknowns

1. **Id stability.** Liberty is unpinned and rebuilt weekly; a retagged layer
   turns an override into a silent no-op. Only the fixture assertion catches it.
2. **Delegated click handlers across `setStyle`.** `map.on("click",
   LOCATION_LAYER, …)` plus the `locationHandlersInstalled` guard may leave pins
   unclickable after a style switch; one browser check decides whether the guard
   must reset in `style.load`.
3. **Dark leftovers.** `natural_earth` shading, sprite icons and `building`
   stay light and no paint override fixes them; decide between hiding the raster
   in dark and accepting the contrast before shipping the look.

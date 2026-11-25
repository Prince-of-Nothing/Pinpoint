# 0026. Opt-in Satellite Imagery, Terrain and 3D Buildings on the Web Map

Date: 2026-11-25

## Status

Accepted.

Extends [ADR-0021](./0021-global-map-catalog.md), which made the OpenFreeMap
basemap the only remote map host and kept it opt-in.

## Context

The web map offered two basemaps (offline PMTiles, OpenFreeMap "Streets") and
overlay toggles for friends, trail and notes. Users expect a map app's layer
menu to offer satellite imagery and a 3D view; Pinpoint had neither.

The page CSP admitted one remote host, `tiles.openfreemap.org`, and the
project had skipped satellite and terrain because the familiar providers
(Google, Mapbox, MapTiler, Esri) need an API key, an account or terms that
restrict redistribution in an open-source app. A key would also have to live
in the browser or behind a new server proxy.

Keyless options exist:

- **EOX Sentinel-2 cloudless 2016** (`tiles.maps.eox.at`): a global cloud-free
  mosaic, CC BY 4.0, about 10 m per pixel. Later yearly mosaics are
  CC BY-NC-SA 4.0, so only the 2016 layer is used. The host returns CORS
  headers, which WebGL needs.
- **Terrain Tiles on AWS Open Data** (`elevation-tiles-prod.s3.amazonaws.com`):
  the Mapzen/Tilezen terrarium DEM, served anonymously with
  `Access-Control-Allow-Origin: *`. Attribution follows the upstream sources
  (SRTM, GMTED2010 via USGS, and others).
- **3D buildings** need no new data: OpenFreeMap's Liberty style already ships
  a `building-3d` extrusion, and the offline OpenMapTiles schema carries
  `render_height` on its `building` layer.

## Decision

We will offer three opt-in layers in the web map's layer panel, all off by
default and stored per device:

- **Satellite** as a third map type next to Streets and Offline. It is the
  Liberty style with the EOX 2016 raster drawn under roads, buildings and
  labels (a hybrid view). Turning the online map off also turns satellite off.
- **Terrain**: hill shading plus raised relief (`setTerrain`) from the AWS
  terrarium DEM, using two DEM sources as MapLibre recommends.
- **3D buildings**: Liberty's `building-3d` follows the toggle; on the
  offline style a `fill-extrusion` layer is added on the PMTiles source.
  Turning terrain or 3D buildings on tilts the camera; turning both off
  flattens it.

The layer logic lives in `web/app/map-layers.js`, is idempotent and is
re-applied after every style switch. The CSP admits exactly the two new
origins in `img-src` and `connect-src`; a unit test fails on any other remote
source or a wildcard. The map attribution shows the EOX and terrain credits
while those layers are on, and the panel and the privacy page name every
provider and what it sees.

Rejected alternatives:

- **Esri World Imagery or a keyed provider**: sharper imagery, but needs an
  account or key and has redistribution terms an open-source app cannot meet
  by default.
- **Proxying the new tiles through the Pinpoint server**: would hide the
  user's IP from the providers, but turns the server into an open imagery
  relay with bandwidth cost and caching obligations. The opt-in model already
  used for OpenFreeMap is applied instead.
- **EOX 2018+ mosaics**: newer, but non-commercial licence.

## Consequences

- Users get satellite, terrain and 3D views without an account or key.
- Two more third parties can see a user's IP address and the tile coordinates
  of the area on screen, but only after the user turns that layer on. No
  account, friend, share or location data reaches them; the CSP enforces the
  host list.
- Satellite imagery is from 2016 at about 10 m per pixel: fields, forests and
  towns are clear, individual houses are not.
- Satellite and terrain need a connection; offline they fail quietly and the
  base map keeps working.
- If either provider changes its endpoint or terms, the layer stops loading
  and the CSP host list and `map-layers.js` change together.

# OpenStreetMap policy

Pinpoint uses OpenStreetMap data under the ODbL. This short policy is the
repository source of truth; the full guide is in the
[wiki](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Maps-Self-Hosted-Osm).

- **Never** use `tile.openstreetmap.org` for app traffic (OSMF tile policy).
- The backend proxy is the default: `/map/tiles/{z}/{x}/{y}` with
  `PINPOINT_TILE_URL` for a self-hosted raster source.
- Public Nominatim, OSRM and Overpass endpoints are demo defaults only. They
  receive user-initiated queries only, at most one request per second for
  Nominatim; production self-hosts them (`PINPOINT_NOMINATIM_URL`,
  `PINPOINT_OSRM_URL`, `PINPOINT_OVERPASS_URL`).
- Friend/shared coordinates are never sent to OSM services.
- Every map surface shows **© OpenStreetMap contributors** and the ODbL link.

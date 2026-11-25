// Satellite, terrain and 3D buildings must apply cleanly to both the online
// (OpenFreeMap Liberty) and the offline (OpenMapTiles PMTiles) style, survive
// being re-applied after a style switch, and only use hosts the CSP allows.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const layers = await import(pathToFileURL(path.resolve("web/app/map-layers.js")).href);
const {
  applyLayers,
  targetPitch,
  SATELLITE_LAYER,
  SATELLITE_SOURCE,
  TERRAIN_SOURCE,
  HILLSHADE_LAYER,
  HILLSHADE_SOURCE,
  BUILDINGS_LAYER,
  TILT_PITCH,
  SATELLITE_TILES,
  TERRAIN_TILES,
} = layers;

class FakeMap {
  constructor(style) {
    this.style = structuredClone(style);
    this.terrain = null;
  }
  getStyle() {
    return this.style;
  }
  getLayer(id) {
    return this.style.layers.find((layer) => layer.id === id);
  }
  getSource(id) {
    return this.style.sources[id];
  }
  addSource(id, source) {
    assert.ok(!this.style.sources[id], `source ${id} added twice`);
    this.style.sources[id] = source;
  }
  removeSource(id) {
    assert.ok(
      !this.style.layers.some((layer) => layer.source === id),
      `source ${id} removed while a layer still uses it`,
    );
    delete this.style.sources[id];
  }
  addLayer(layer, before) {
    assert.ok(!this.getLayer(layer.id), `layer ${layer.id} added twice`);
    if (before === undefined) {
      this.style.layers.push(layer);
      return;
    }
    const index = this.style.layers.findIndex((item) => item.id === before);
    assert.ok(index >= 0, `beforeId ${before} does not exist`);
    this.style.layers.splice(index, 0, layer);
  }
  removeLayer(id) {
    this.style.layers = this.style.layers.filter((layer) => layer.id !== id);
  }
  setLayoutProperty(id, key, value) {
    const layer = this.getLayer(id);
    layer.layout = { ...(layer.layout || {}), [key]: value };
  }
  setTerrain(terrain) {
    this.terrain = terrain;
  }
  ids() {
    return this.style.layers.map((layer) => layer.id);
  }
  visibility(id) {
    return this.getLayer(id)?.layout?.visibility ?? "visible";
  }
}

const liberty = {
  sources: { openmaptiles: { type: "vector" }, ne2_shaded: { type: "raster" } },
  layers: [
    { id: "background", type: "background" },
    { id: "landcover", type: "fill", source: "openmaptiles", "source-layer": "landcover" },
    { id: "building", type: "fill", source: "openmaptiles", "source-layer": "building" },
    { id: "road", type: "line", source: "openmaptiles", "source-layer": "transportation" },
    { id: "building-3d", type: "fill-extrusion", source: "openmaptiles", "source-layer": "building" },
    { id: "place-label", type: "symbol", source: "openmaptiles", "source-layer": "place" },
  ],
};

const offline = {
  sources: { basemap: { type: "vector" } },
  layers: [
    { id: "background", type: "background" },
    { id: "water", type: "fill", source: "basemap", "source-layer": "water" },
    { id: "roads", type: "line", source: "basemap", "source-layer": "transportation" },
  ],
};

// Satellite on the online style: imagery under buildings/roads/labels, flat
// footprints hidden, 3D buildings untouched; off restores everything.
{
  const map = new FakeMap(liberty);
  applyLayers(map, { satellite: true });
  assert.deepEqual(map.getSource(SATELLITE_SOURCE).tiles, [SATELLITE_TILES]);
  assert.deepEqual(map.ids().slice(0, 4), ["background", "landcover", SATELLITE_LAYER, "building"]);
  assert.equal(map.visibility("building"), "none", "flat footprints would cover the imagery");
  assert.equal(map.visibility("building-3d"), "none", "3D buildings follow their own toggle (off)");
  applyLayers(map, { satellite: true }); // idempotent after a re-apply
  applyLayers(map, { satellite: false });
  assert.ok(!map.getLayer(SATELLITE_LAYER) && !map.getSource(SATELLITE_SOURCE));
  assert.equal(map.visibility("building"), "visible");
}

// Terrain: two DEM sources, hillshade under the overlays, relief on; off clears it.
{
  const map = new FakeMap(liberty);
  applyLayers(map, { terrain: true });
  for (const id of [TERRAIN_SOURCE, HILLSHADE_SOURCE]) {
    assert.equal(map.getSource(id).type, "raster-dem");
    assert.equal(map.getSource(id).encoding, "terrarium");
    assert.deepEqual(map.getSource(id).tiles, [TERRAIN_TILES]);
  }
  assert.ok(map.ids().indexOf(HILLSHADE_LAYER) < map.ids().indexOf("building"));
  assert.equal(map.terrain.source, TERRAIN_SOURCE);
  assert.ok(map.terrain.exaggeration > 1);
  applyLayers(map, { terrain: true });
  applyLayers(map, { terrain: false });
  assert.equal(map.terrain, null);
  assert.ok(!map.getLayer(HILLSHADE_LAYER) && !map.getSource(TERRAIN_SOURCE) && !map.getSource(HILLSHADE_SOURCE));
}

// 3D buildings online: Liberty's own building-3d follows the toggle, no duplicate.
{
  const map = new FakeMap(liberty);
  applyLayers(map, { buildings: true });
  assert.equal(map.visibility("building-3d"), "visible");
  assert.ok(!map.getLayer(BUILDINGS_LAYER));
  applyLayers(map, { buildings: false });
  assert.equal(map.visibility("building-3d"), "none");
}

// 3D buildings offline: the PMTiles style has none, so one is added on its source.
{
  const map = new FakeMap(offline);
  applyLayers(map, { buildings: true, satellite: true, terrain: true });
  const extrusion = map.getLayer(BUILDINGS_LAYER);
  assert.equal(extrusion.type, "fill-extrusion");
  assert.equal(extrusion.source, "basemap");
  assert.equal(extrusion["source-layer"], "building");
  assert.ok(map.ids().indexOf(SATELLITE_LAYER) < map.ids().indexOf("roads"));
  applyLayers(map, { buildings: true, satellite: true, terrain: true });
  applyLayers(map, {});
  assert.deepEqual(map.ids(), ["background", "water", "roads"], "everything added must be removed");
  assert.deepEqual(Object.keys(map.style.sources), ["basemap"]);
}

// Camera: tilt for raised content, keep a user's tilt, flatten when nothing is raised.
assert.equal(targetPitch({ buildings: true }, 0), TILT_PITCH);
assert.equal(targetPitch({ terrain: true }, 30), 30);
assert.equal(targetPitch({}, 45), 0);

// Every remote tile host the module uses must be admitted by the server CSP.
const handlers = await readFile("src/server/web_handlers.rs", "utf8");
for (const url of [SATELLITE_TILES, TERRAIN_TILES]) {
  const origin = new URL(url.replace(/\{[a-z]\}/g, "0")).origin;
  assert.ok(handlers.includes(`"${origin}"`), `${origin} missing from OPT_IN_MAP_HOSTS`);
  assert.ok(handlers.includes(`connect-src 'self' https://tiles.openfreemap.org`), "connect-src changed shape");
  assert.match(handlers, new RegExp(origin.replace(/[.]/g, "\\.")), `${origin} missing from the CSP`);
}

// The panel offers the three map types and the 3D toggles.
const page = await readFile("web/app/index.html", "utf8");
for (const id of ["basemap-streets", "basemap-satellite", "basemap-offline", "layer-buildings", "layer-terrain", "map-attribution-satellite", "map-attribution-terrain"]) {
  assert.ok(page.includes(`id="${id}"`), `missing #${id}`);
}
assert.match(page, /Sentinel-2 cloudless/, "EOX credit missing");
assert.match(page, /Copernicus Sentinel data 2016/, "EOX credit must name the data year");

console.log("PASS  satellite, terrain and 3D buildings apply to both styles and stay within the CSP");

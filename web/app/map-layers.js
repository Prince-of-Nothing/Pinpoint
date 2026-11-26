// Optional map layers on top of whichever style is loaded (OpenFreeMap
// Liberty online, or the same-origin PMTiles style offline):
//
//  - satellite: Sentinel-2 cloudless 2016 imagery by EOX (CC BY 4.0), drawn
//    under roads, buildings and labels so the map reads as a hybrid;
//  - terrain:   Terrain Tiles on AWS (Mapzen/Tilezen terrarium DEM) for hill
//    shading and raised relief;
//  - buildings: 3D building extrusions from the vector tiles' `building` layer.
//
// Satellite and terrain are the only extra hosts the page may contact
// (ADR-0026). They see tile coordinates for the area on screen and nothing
// else: no account data, friend pins or locations. Both are off by default.
//
// `applyLayers` is idempotent and is called after every style load, because a
// style switch drops every source and layer that was added at runtime.

export const SATELLITE_TILES =
  "https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless_3857/default/g/{z}/{y}/{x}.jpg";
export const TERRAIN_TILES =
  "https://elevation-tiles-prod.s3.amazonaws.com/terrarium/{z}/{x}/{y}.png";

export const SATELLITE_SOURCE = "pinpoint-satellite";
export const SATELLITE_LAYER = "pinpoint-satellite";
export const TERRAIN_SOURCE = "pinpoint-terrain-dem";
export const HILLSHADE_SOURCE = "pinpoint-hillshade-dem";
export const HILLSHADE_LAYER = "pinpoint-hillshade";
export const BUILDINGS_LAYER = "pinpoint-buildings-3d";

export const TERRAIN_EXAGGERATION = 1.5;
export const TILT_PITCH = 55;

const DEM = {
  type: "raster-dem",
  tiles: [TERRAIN_TILES],
  encoding: "terrarium",
  tileSize: 256,
  maxzoom: 15,
};

/** The first layer that must stay above imagery and relief. */
export function firstOverlayLayer(map) {
  const layers = map.getStyle()?.layers || [];
  const hit = layers.find(
    (layer) =>
      layer.type === "symbol" ||
      layer["source-layer"] === "building" ||
      layer["source-layer"] === "transportation" ||
      layer.id === BUILDINGS_LAYER,
  );
  return hit?.id;
}

/** The vector source that carries OpenMapTiles layers (Liberty: `openmaptiles`, offline: `basemap`). */
export function vectorSource(map) {
  const style = map.getStyle() || {};
  const fromLayer = (style.layers || []).find(
    (layer) => layer["source-layer"] && style.sources?.[layer.source]?.type === "vector",
  );
  if (fromLayer) return fromLayer.source;
  return Object.keys(style.sources || {}).find((id) => style.sources[id]?.type === "vector");
}

function setVisible(map, id, visible) {
  if (map.getLayer(id)) map.setLayoutProperty(id, "visibility", visible ? "visible" : "none");
}

function removeLayer(map, id) {
  if (map.getLayer(id)) map.removeLayer(id);
}

function removeSource(map, id) {
  if (map.getSource(id)) map.removeSource(id);
}

export function applySatellite(map, on) {
  const footprints = (map.getStyle()?.layers || []).filter(
    (layer) => layer.type === "fill" && layer["source-layer"] === "building",
  );
  if (!on) {
    removeLayer(map, SATELLITE_LAYER);
    removeSource(map, SATELLITE_SOURCE);
    for (const layer of footprints) setVisible(map, layer.id, true);
    return;
  }
  if (!map.getSource(SATELLITE_SOURCE)) {
    map.addSource(SATELLITE_SOURCE, {
      type: "raster",
      tiles: [SATELLITE_TILES],
      tileSize: 256,
      maxzoom: 15,
    });
  }
  if (!map.getLayer(SATELLITE_LAYER)) {
    map.addLayer(
      { id: SATELLITE_LAYER, type: "raster", source: SATELLITE_SOURCE, paint: { "raster-fade-duration": 150 } },
      firstOverlayLayer(map),
    );
  }
  // Flat grey footprints would hide the imagery; 3D buildings stay.
  for (const layer of footprints) setVisible(map, layer.id, false);
}

export function applyTerrain(map, on) {
  if (!on) {
    map.setTerrain(null);
    removeLayer(map, HILLSHADE_LAYER);
    removeSource(map, HILLSHADE_SOURCE);
    removeSource(map, TERRAIN_SOURCE);
    return;
  }
  // MapLibre recommends separate DEM sources for relief and shading.
  if (!map.getSource(TERRAIN_SOURCE)) map.addSource(TERRAIN_SOURCE, { ...DEM });
  if (!map.getSource(HILLSHADE_SOURCE)) map.addSource(HILLSHADE_SOURCE, { ...DEM });
  if (!map.getLayer(HILLSHADE_LAYER)) {
    map.addLayer(
      {
        id: HILLSHADE_LAYER,
        type: "hillshade",
        source: HILLSHADE_SOURCE,
        paint: { "hillshade-exaggeration": 0.35, "hillshade-shadow-color": "#3d3424" },
      },
      firstOverlayLayer(map),
    );
  }
  map.setTerrain({ source: TERRAIN_SOURCE, exaggeration: TERRAIN_EXAGGERATION });
}

export function applyBuildings(map, on) {
  const styled = (map.getStyle()?.layers || []).filter(
    (layer) =>
      layer.type === "fill-extrusion" &&
      layer["source-layer"] === "building" &&
      layer.id !== BUILDINGS_LAYER,
  );
  if (styled.length) {
    // Liberty ships its own `building-3d`; follow the toggle instead of doubling it.
    for (const layer of styled) setVisible(map, layer.id, on);
    return;
  }
  if (!on) {
    removeLayer(map, BUILDINGS_LAYER);
    return;
  }
  const source = vectorSource(map);
  if (!source || map.getLayer(BUILDINGS_LAYER)) return;
  const labels = (map.getStyle()?.layers || []).find((layer) => layer.type === "symbol");
  map.addLayer(
    {
      id: BUILDINGS_LAYER,
      type: "fill-extrusion",
      source,
      "source-layer": "building",
      minzoom: 14,
      paint: {
        "fill-extrusion-color": "hsl(35, 8%, 82%)",
        "fill-extrusion-height": ["coalesce", ["get", "render_height"], 6],
        "fill-extrusion-base": ["coalesce", ["get", "render_min_height"], 0],
        "fill-extrusion-opacity": 0.85,
      },
    },
    labels?.id,
  );
}

/**
 * Bring the loaded style in line with the chosen layers.
 *
 * @param {object} map MapLibre map (or anything with the same style API).
 * @param {{satellite?: boolean, terrain?: boolean, buildings?: boolean}} layers
 */
export function applyLayers(map, { satellite = false, terrain = false, buildings = false } = {}) {
  applySatellite(map, satellite);
  applyTerrain(map, terrain);
  applyBuildings(map, buildings);
}

/**
 * Run a change that adds sources or layers. While a basemap switch is still
 * loading, MapLibre throws "Style is not done loading"; that call is skipped
 * (returns false) because the next `style.load` re-applies everything. Any
 * other error is a real bug and is rethrown.
 *
 * @param {() => void} run
 * @returns {boolean} whether the change ran
 */
export function whenStyleReady(run) {
  try {
    run();
    return true;
  } catch (error) {
    if (/not done loading/i.test(String(error?.message))) return false;
    throw error;
  }
}

/** Pitch the camera for 3D content, flatten it when nothing is raised. */
export function targetPitch({ terrain = false, buildings = false } = {}, current = 0) {
  if (terrain || buildings) return current > 0 ? current : TILT_PITCH;
  return 0;
}

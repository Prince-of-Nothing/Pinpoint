/**
 * Pinpoint shared map module.
 *
 * A small, dependency-light wrapper around MapLibre GL JS + PMTiles. It is the
 * foundation shared by the web app and the Android/desktop WebView map pages.
 * It has no imports from the rest of `web/app/` and makes no network requests
 * other than the local style, the local PMTiles fixture/pack and MapLibre's own
 * worker file.
 *
 * The caller must load `../vendor/maplibre-gl.css` in the page. The module
 * loads `../vendor/pmtiles.js` itself on first use.
 */

import * as maplibregl from "../vendor/maplibre-gl.js";

const PMTILES_SCRIPT_URL = new URL("../vendor/pmtiles.js", import.meta.url).href;
const LOCATION_SOURCE = "pinpoint-locations";
const LOCATION_LAYER = "pinpoint-locations-circles";
const ROUTE_SOURCE = "pinpoint-route";
const ROUTE_CASING_LAYER = "pinpoint-route-casing";
const ROUTE_LINE_LAYER = "pinpoint-route-line";
const STYLE_ELEMENT_ID = "pinpoint-map-styles";

const PRECISION_COLORS = {
  exact: "#34d399",
  approximate: "#facc15",
  city_only: "#f87171",
  unknown: "#94a3b8",
};

const PRECISION_LABELS = {
  exact: "Exact location",
  approximate: "Approximate location",
  city_only: "City only",
  unknown: "Location",
};

/**
 * Route line colours for the casing and the line layer.
 */
const ROUTE_PAINT = {
  light: { casing: "#ffffff", line: "#a8641f" },
  dark: { casing: "#0b0f14", line: "#e3a458" },
};

/**
 * Paint overrides used by `setTheme`. Layer ids match `style.json`; unknown
 * ids are skipped so a replacement style can omit layers safely.
 */
const THEME_PAINT = {
  light: {
    background: ["background-color", "#f4f1ea"],
    landcover: ["fill-color", "#dbe9d4"],
    "landuse-green": ["fill-color", "#d7e7cf"],
    "landuse-urban": ["fill-color", "#e7e4de"],
    water: ["fill-color", "#a9d3e0"],
    waterway: ["line-color", "#8fc1d4"],
    "boundary-country": ["line-color", "#8b7aa8"],
    "boundary-region": ["line-color", "#a393bd"],
    rail: ["line-color", "#a9a39a"],
    "road-major-casing": ["line-color", "#c9c0b2"],
    "road-major": ["line-color", "#f3b562"],
    "road-medium": ["line-color", "#f7d9a8"],
    "road-minor": ["line-color", "#ffffff"],
    "road-path": ["line-color", "#c9b99f"],
  },
  dark: {
    background: ["background-color", "#11161d"],
    landcover: ["fill-color", "#1b2a24"],
    "landuse-green": ["fill-color", "#1e2c25"],
    "landuse-urban": ["fill-color", "#20272f"],
    water: ["fill-color", "#16384a"],
    waterway: ["line-color", "#245a70"],
    "boundary-country": ["line-color", "#7f6ba8"],
    "boundary-region": ["line-color", "#6d5b91"],
    rail: ["line-color", "#5a6069"],
    "road-major-casing": ["line-color", "#0b0f14"],
    "road-major": ["line-color", "#c77b3a"],
    "road-medium": ["line-color", "#b08b54"],
    "road-minor": ["line-color", "#4a515b"],
    "road-path": ["line-color", "#5b5145"],
  },
};

const MAP_CSS = `
.pinpoint-map {
  position: relative;
  background: #f4f1ea;
  --pinpoint-popup-bg: #ffffff;
  --pinpoint-popup-fg: #111827;
  --pinpoint-popup-muted: #667085;
}
.pinpoint-map--dark {
  background: #11161d;
  --pinpoint-popup-bg: #1b232e;
  --pinpoint-popup-fg: #f3f4f6;
  --pinpoint-popup-muted: #a6b0bf;
}
.pinpoint-map .maplibregl-canvas { outline: none; }
.pinpoint-map .maplibregl-popup-content {
  background: var(--pinpoint-popup-bg);
  color: var(--pinpoint-popup-fg);
  border-radius: 10px;
  padding: 10px 12px;
  font: 13px/1.4 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
  box-shadow: 0 10px 30px rgb(15 23 42 / 22%);
}
.pinpoint-map .maplibregl-popup-tip { border-top-color: var(--pinpoint-popup-bg); }
.pinpoint-map .maplibregl-popup-close-button { color: var(--pinpoint-popup-muted); }
.pinpoint-popup-content { display: flex; flex-direction: column; gap: 2px; }
.pinpoint-popup-content strong { font-size: 13px; font-weight: 650; }
.pinpoint-popup-content span { color: var(--pinpoint-popup-muted); font-size: 11px; }
`;

let pmtilesPromise = null;
let pmtilesProtocolReady = false;

/**
 * Load the vendored PMTiles browser build from this module's own directory.
 * `pmtiles.js` is an IIFE build that publishes `globalThis.pmtiles`; importing
 * it as an ES module would not expose that global, so a local script tag is
 * the reliable path and keeps the module usable from any static origin.
 */
function loadPmtilesApi() {
  if (globalThis.pmtiles?.Protocol) return Promise.resolve(globalThis.pmtiles);
  if (pmtilesPromise) return pmtilesPromise;

  pmtilesPromise = new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = PMTILES_SCRIPT_URL;
    script.async = true;
    script.onload = () => {
      if (globalThis.pmtiles?.Protocol) {
        resolve(globalThis.pmtiles);
      } else {
        pmtilesPromise = null;
        reject(new Error("pmtiles.js loaded but did not expose globalThis.pmtiles"));
      }
    };
    script.onerror = () => {
      pmtilesPromise = null;
      reject(new Error(`Failed to load PMTiles from ${PMTILES_SCRIPT_URL}`));
    };
    document.head.appendChild(script);
  });

  return pmtilesPromise;
}

function ensurePmtilesProtocol(pmtilesApi) {
  if (pmtilesProtocolReady) return;
  const protocol = new pmtilesApi.Protocol();
  maplibregl.addProtocol("pmtiles", protocol.tile);
  pmtilesProtocolReady = true;
}

/** Convert a caller-supplied PMTiles path into a `pmtiles://` source URL. */
function toProtocolUrl(url) {
  const raw = String(url ?? "").trim();
  if (!raw) throw new Error("createMap: pmtilesUrl is required");
  if (raw.startsWith("pmtiles://")) return raw;
  return `pmtiles://${raw.replace(/^\.\//, "")}`;
}

/**
 * Fetch the style and bind its `pmtiles://` vector source(s) to `pmtilesUrl`.
 * The style is fetched relatively, so the same module works at `/map/`, at a
 * loopback WebView origin, or under an app sub-path.
 */
async function loadStyle(styleUrl, pmtilesUrl) {
  const response = await fetch(styleUrl);
  if (!response.ok) {
    throw new Error(`Failed to load map style ${styleUrl}: HTTP ${response.status}`);
  }

  const style = await response.json();
  const protocolUrl = toProtocolUrl(pmtilesUrl);
  let bound = 0;

  for (const source of Object.values(style.sources ?? {})) {
    if (source && typeof source.url === "string" && source.url.startsWith("pmtiles://")) {
      source.url = protocolUrl;
      bound += 1;
    }
  }

  if (bound === 0) {
    throw new Error("Map style has no pmtiles:// vector source to bind");
  }

  return style;
}

function injectStyles() {
  if (document.getElementById(STYLE_ELEMENT_ID)) return;
  const style = document.createElement("style");
  style.id = STYLE_ELEMENT_ID;
  style.textContent = MAP_CSS;
  document.head.appendChild(style);
}

function toGeoJSON(locations) {
  const features = [];

  for (const location of Array.isArray(locations) ? locations : []) {
    const latitude = Number(location?.latitude);
    const longitude = Number(location?.longitude);
    if (
      !Number.isFinite(latitude) ||
      !Number.isFinite(longitude) ||
      Math.abs(latitude) > 90 ||
      Math.abs(longitude) > 180
    ) {
      continue;
    }

    const precision = String(location?.precision_level ?? "unknown");

    features.push({
      type: "Feature",
      geometry: { type: "Point", coordinates: [longitude, latitude] },
      properties: {
        user_id: location?.user_id ?? null,
        user_name: String(location?.user_name ?? "Friend"),
        precision_level: precision,
        latitude,
        longitude,
      },
    });
  }

  return { type: "FeatureCollection", features };
}

/**
 * Convert route `[lat, lon]` points into a GeoJSON LineString feature.
 * Invalid or missing points are skipped; fewer than two valid points means no
 * drawable route.
 */
function toRouteGeoJSON(routeOrPoints) {
  const points = Array.isArray(routeOrPoints)
    ? routeOrPoints
    : Array.isArray(routeOrPoints?.points)
      ? routeOrPoints.points
      : [];
  const coordinates = [];

  for (const point of points) {
    if (!Array.isArray(point) || point.length < 2) continue;
    const lat = Number(point[0]);
    const lon = Number(point[1]);
    if (
      !Number.isFinite(lat) ||
      !Number.isFinite(lon) ||
      Math.abs(lat) > 90 ||
      Math.abs(lon) > 180
    ) {
      continue;
    }
    coordinates.push([lon, lat]);
  }

  if (coordinates.length < 2) return null;
  return {
    type: "Feature",
    properties: {},
    geometry: { type: "LineString", coordinates },
  };
}

function buildPopupContent(properties) {
  const root = document.createElement("div");
  root.className = "pinpoint-popup-content";

  const name = document.createElement("strong");
  name.textContent = properties.user_name || "Friend";

  const precision = String(properties.precision_level || "unknown");
  const latitude = Number(properties.latitude);
  const longitude = Number(properties.longitude);
  const meta = document.createElement("span");
  meta.textContent = `${PRECISION_LABELS[precision] ?? PRECISION_LABELS.unknown} · ${latitude.toFixed(4)}, ${longitude.toFixed(4)}`;

  root.append(name, meta);
  return root;
}

/**
 * Create a MapLibre map bound to a local PMTiles pack.
 *
 * @param {object} options
 * @param {HTMLElement|string} options.container Element or selector. It must
 *   have a non-zero height before the map is created.
 * @param {string} options.styleUrl Local MapLibre style URL. The style must
 *   contain at least one vector source whose `url` starts with `pmtiles://`.
 * @param {string} options.pmtilesUrl Local PMTiles path (absolute,
 *   root-relative or page-relative). The module derives the `pmtiles://` URL.
 * @param {[number, number]} [options.center] `[longitude, latitude]`.
 * @param {number} [options.zoom]
 * @param {(handle: PinpointMap) => void} [options.onLoad] Called after the
 *   MapLibre `load` event and the first render.
 * @param {(error: Error) => void} [options.onError] Called for style/tile
 *   failures, including MapLibre's own `error` events.
 * @returns {PinpointMap}
 */
export function createMap({
  container,
  styleUrl,
  pmtilesUrl,
  center = [0, 0],
  zoom = 2,
  onLoad,
  onError,
} = {}) {
  if (!styleUrl) throw new Error("createMap: styleUrl is required");
  if (!pmtilesUrl) throw new Error("createMap: pmtilesUrl is required");

  const element =
    typeof container === "string" ? document.querySelector(container) : container;
  if (!element) throw new Error("createMap: container was not found");

  injectStyles();
  element.classList.add("pinpoint-map");

  const clickListeners = new Set();
  let map = null;
  let theme = "light";
  let loaded = false;
  let destroyed = false;
  let locationsRequested = false;
  let pendingLocations = toGeoJSON([]);
  let pendingRoute = null;
  let popup = null;
  let locationHandlersInstalled = false;

  const handle = {
    setLocations,
    onClick,
    setTheme,
    drawRoute,
    clearRoute,
    destroy,
    resize,
    getMap,
  };

  element.dataset.theme = theme;

  function fail(error) {
    if (destroyed) return;
    if (typeof onError === "function") onError(error);
    else console.error("[pinpoint-map]", error);
  }

  function applyTheme() {
    if (!map || !map.isStyleLoaded()) return;
    for (const [layerId, [property, value]] of Object.entries(THEME_PAINT[theme])) {
      if (map.getLayer(layerId)) map.setPaintProperty(layerId, property, value);
    }
    paintRoute();
  }

  function installLocationHandlers() {
    if (!map || locationHandlersInstalled) return;
    locationHandlersInstalled = true;

    map.on("click", LOCATION_LAYER, (event) => {
      const feature = event.features?.[0];
      if (!feature) return;

      popup?.remove();
      popup = new maplibregl.Popup({ offset: 12, closeButton: true })
        .setLngLat(feature.geometry.coordinates)
        .setDOMContent(buildPopupContent(feature.properties ?? {}))
        .addTo(map);
    });

    map.on("mouseenter", LOCATION_LAYER, () => {
      map.getCanvas().style.cursor = "pointer";
    });
    map.on("mouseleave", LOCATION_LAYER, () => {
      map.getCanvas().style.cursor = "";
    });
  }

  function applyLocations() {
    if (!map || !loaded) return;

    if (map.getSource(LOCATION_SOURCE)) {
      map.getSource(LOCATION_SOURCE).setData(pendingLocations);
      return;
    }

    map.addSource(LOCATION_SOURCE, {
      type: "geojson",
      data: pendingLocations,
    });

    map.addLayer({
      id: LOCATION_LAYER,
      type: "circle",
      source: LOCATION_SOURCE,
      paint: {
        "circle-radius": ["interpolate", ["linear"], ["zoom"], 4, 4, 12, 7, 18, 9],
        "circle-color": [
          "match",
          ["get", "precision_level"],
          "exact",
          PRECISION_COLORS.exact,
          "approximate",
          PRECISION_COLORS.approximate,
          "city_only",
          PRECISION_COLORS.city_only,
          PRECISION_COLORS.unknown,
        ],
        "circle-stroke-color": "#ffffff",
        "circle-stroke-width": 2,
        "circle-opacity": 0.95,
      },
    });

    installLocationHandlers();
  }

  function paintRoute() {
    if (!map || !map.isStyleLoaded()) return;
    const colors = ROUTE_PAINT[theme];
    if (map.getLayer(ROUTE_CASING_LAYER)) {
      map.setPaintProperty(ROUTE_CASING_LAYER, "line-color", colors.casing);
    }
    if (map.getLayer(ROUTE_LINE_LAYER)) {
      map.setPaintProperty(ROUTE_LINE_LAYER, "line-color", colors.line);
    }
  }

  function applyRoute() {
    if (!map || !loaded || !pendingRoute) return;

    if (map.getSource(ROUTE_SOURCE)) {
      map.getSource(ROUTE_SOURCE).setData(pendingRoute);
    } else {
      map.addSource(ROUTE_SOURCE, { type: "geojson", data: pendingRoute });
      map.addLayer({
        id: ROUTE_CASING_LAYER,
        type: "line",
        source: ROUTE_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": ROUTE_PAINT[theme].casing,
          "line-width": ["interpolate", ["linear"], ["zoom"], 6, 7, 14, 11],
          "line-opacity": 0.9,
        },
      });
      map.addLayer({
        id: ROUTE_LINE_LAYER,
        type: "line",
        source: ROUTE_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": ROUTE_PAINT[theme].line,
          "line-width": ["interpolate", ["linear"], ["zoom"], 6, 4, 14, 6.5],
        },
      });
    }

    paintRoute();

    const bounds = new maplibregl.LngLatBounds();
    for (const coordinate of pendingRoute.geometry.coordinates) {
      bounds.extend(coordinate);
    }
    map.fitBounds(bounds, { padding: 48, maxZoom: 14, duration: 600 });
  }

  /**
   * Draw a local route line. Accepts the route UI shape (`points: [[lat, lon]]`)
   * or a plain list of points. Safe before the style has loaded: the latest
   * call is drawn on load. Returns `false` when there is no usable geometry.
   *
   * @param {object|Array<[number, number]>} routeOrPoints
   * @returns {boolean}
   */
  function drawRoute(routeOrPoints) {
    const feature = toRouteGeoJSON(routeOrPoints);
    if (!feature) return false;
    pendingRoute = feature;
    applyRoute();
    return true;
  }

  /** Remove the route line, casing and source. Safe to call repeatedly. */
  function clearRoute() {
    pendingRoute = null;
    if (!map || !loaded) return;
    for (const layer of [ROUTE_LINE_LAYER, ROUTE_CASING_LAYER]) {
      if (map.getLayer(layer)) map.removeLayer(layer);
    }
    if (map.getSource(ROUTE_SOURCE)) map.removeSource(ROUTE_SOURCE);
  }

  /**
   * Replace the friend markers. Invalid coordinates are skipped. Safe to call
   * before the style has loaded; the latest call wins.
   *
   * @param {Array<{user_id: number|string, user_name: string, latitude: number,
   *   longitude: number, precision_level: "exact"|"approximate"|"city_only"|string}>} locations
   * @returns {number} number of accepted locations
   */
  function setLocations(locations) {
    pendingLocations = toGeoJSON(locations);
    locationsRequested = true;
    applyLocations();
    return pendingLocations.features.length;
  }

  /**
   * Register a general map-click handler. Marker clicks also emit this event.
   *
   * @param {(position: {lat: number, lon: number}) => void} callback
   * @returns {() => void} unsubscribe function
   */
  function onClick(callback) {
    if (typeof callback !== "function") {
      throw new TypeError("onClick: callback must be a function");
    }
    clickListeners.add(callback);
    return () => clickListeners.delete(callback);
  }

  /**
   * Switch the map and page-level theme. Dark mode also recolours the known
   * style layers; a custom style whose layer ids differ keeps its own colours.
   *
   * @param {"light"|"dark"} next
   */
  function setTheme(next) {
    theme = next === "dark" ? "dark" : "light";
    element.dataset.theme = theme;
    element.classList.toggle("pinpoint-map--dark", theme === "dark");
    applyTheme();
  }

  function resize() {
    map?.resize();
  }

  function getMap() {
    return map;
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    clickListeners.clear();
    pendingRoute = null;
    popup?.remove();
    popup = null;
    map?.remove();
    map = null;
    loaded = false;
    element.classList.remove("pinpoint-map", "pinpoint-map--dark");
    delete element.dataset.theme;
  }

  async function initialize() {
    try {
      const pmtilesApi = await loadPmtilesApi();
      if (destroyed) return;

      ensurePmtilesProtocol(pmtilesApi);
      const style = await loadStyle(styleUrl, pmtilesUrl);
      if (destroyed) return;

      map = new maplibregl.Map({
        container: element,
        style,
        center,
        zoom,
        minZoom: 0,
        maxZoom: 18,
        attributionControl: false,
      });

      map.on("load", () => {
        if (destroyed || !map) return;
        loaded = true;
        applyTheme();
        if (locationsRequested) applyLocations();
        if (pendingRoute) applyRoute();
        if (typeof onLoad === "function") onLoad(handle);
      });

      map.on("click", (event) => {
        if (destroyed) return;
        const position = { lat: event.lngLat.lat, lon: event.lngLat.lng };
        for (const callback of clickListeners) callback(position);
      });

      map.on("error", (event) => {
        fail(event?.error instanceof Error ? event.error : new Error(String(event?.error ?? event)));
      });
    } catch (error) {
      fail(error instanceof Error ? error : new Error(String(error)));
    }
  }

  setTheme("light");
  initialize();

  return handle;
}

/** @typedef {ReturnType<typeof createMap>} PinpointMap */

// Map adapter: the app's stable map API on top of the shared MapLibre GL JS +
// PMTiles foundation (`web/map/map.js`). The default basemap is the same-origin
// PMTiles style; the opt-in online basemap loads OpenFreeMap's Liberty style and
// its tiles. The adapter sends no app data or auth headers to that host.
//
// `app.js`/`nav.js` keep calling the same functions they always have. The
// returned `map` facade mirrors the small view surface they use
// (`getCenter`, `getZoom`, `flyTo`, `panTo`, `invalidateSize`) and translates
// it to MapLibre, whose coordinates are `[longitude, latitude]`.

import { createMap as createFoundationMap } from "/map-assets/map.js";
import * as maplibregl from "/vendor/maplibre-gl.js";

const DEFAULT_STYLE_URL = "/map-assets/style.json";
const DEFAULT_PMTILES_URL = "/map-assets/fixtures/monaco.pmtiles";
const ONLINE_STORAGE_KEY = "pinpoint.onlineMapWeb";
const ONLINE_TIMEOUT_MS = 10000;

const FRIEND_LAYER = "pinpoint-locations-circles";
const ROUTE_SOURCE = "pinpoint-route";
const ROUTE_CASING_LAYER = "pinpoint-route-casing";
const ROUTE_LINE_LAYER = "pinpoint-route-line";
const SELF_SOURCE = "pinpoint-self-accuracy";
const SELF_FILL_LAYER = "pinpoint-self-accuracy-fill";
const SELF_LINE_LAYER = "pinpoint-self-accuracy-line";
const TRAIL_SOURCE = "pinpoint-trail";
const TRAIL_LAYER = "pinpoint-trail-line";
const MEASURE_SOURCE = "pinpoint-measure";
const MEASURE_LAYER = "pinpoint-measure-line";

const EMPTY_FEATURES = { type: "FeatureCollection", features: [] };

/**
 * Build a map backed by the shared MapLibre/PMTiles module.
 *
 * `center` keeps the app's `[latitude, longitude]` order. `pmtilesUrl` and
 * `styleUrl` may be supplied explicitly, on the container as
 * `data-pmtiles-url` / `data-style-url`, through `globalThis.PINPOINT_MAP_CONFIG`
 * or through the page's `pinpoint-map-data` / `pinpoint-map-style` meta tags.
 * The default is the committed Monaco fixture served under `/map-assets/`.
 *
 * @param {object} options
 * @param {HTMLElement|string} options.element
 * @param {[number, number]} [options.center] `[latitude, longitude]`.
 * @param {number} [options.zoom]
 * @param {(point: {lat: number, lon: number}) => void} [options.onClick]
 * @param {(point: {lat: number, lon: number}) => void} [options.onLongPress]
 * @param {(point: {lat: number, lon: number, x: number, y: number}) => void} [options.onContextMenu]
 * @param {"light"|"dark"} [options.theme] Defaults to dark, matching the shell.
 * @param {string} [options.pmtilesUrl]
 * @param {string} [options.styleUrl]
 * @param {(api: object) => void} [options.onLoad]
 * @param {(error: Error) => void} [options.onError]
 */
export function createMap({
  element,
  center = [0, 0],
  zoom = 2,
  onClick,
  onLongPress,
  onContextMenu,
  theme = "dark",
  pmtilesUrl,
  styleUrl,
  onLoad,
  onError,
} = {}) {
  const container =
    typeof element === "string" ? document.querySelector(element) : element;
  if (!container) throw new Error("createMap: element was not found");

  const initialLat = Number(center?.[0]);
  const initialLon = Number(center?.[1]);
  const initialCenter = [
    Number.isFinite(initialLat) ? initialLat : 0,
    Number.isFinite(initialLon) ? initialLon : 0,
  ];
  const initialZoom = Number.isFinite(Number(zoom)) ? Number(zoom) : 2;

  const localStyleUrl =
    styleUrl ||
    container.dataset?.styleUrl ||
    readConfig("styleUrl", "pinpoint-map-style") ||
    DEFAULT_STYLE_URL;
  const localPmtilesUrl =
    pmtilesUrl ||
    container.dataset?.pmtilesUrl ||
    readConfig("pmtilesUrl", "pinpoint-map-data") ||
    DEFAULT_PMTILES_URL;
  const onlineStyleUrl = readConfig("onlineStyleUrl", "pinpoint-online-style");

  const onlineToggle = document.getElementById("online-map-toggle");
  const onlineError = document.getElementById("online-map-error");
  const attributionOffline = document.getElementById("map-attribution-offline");
  const attributionOnline = document.getElementById("map-attribution-online");

  let mapReady = false;
  let destroyed = false;
  let currentTheme = theme === "dark" ? "dark" : "light";
  let pendingView = null;
  let selected = null;
  let self = null;
  let route = null;
  let pendingFit = null;
  let selectedMarker = null;
  let selfMarker = null;
  let routeMarkers = [];
  let trail = null;
  let noteMarkers = [];
  let measurePoints = [];
  let measureMarkers = [];
  let navigationControl = null;
  let scaleControl = null;
  let fullscreenControl = null;
  let online = false;
  let onlineBusy = false;
  let onlineSeq = 0;

  const base = createFoundationMap({
    container,
    styleUrl: localStyleUrl,
    pmtilesUrl: localPmtilesUrl,
    center: [initialCenter[1], initialCenter[0]],
    zoom: initialZoom,
    onLongPress,
    onLoad() {
      if (destroyed) return;
      mapReady = true;
      const map = base.getMap();
      if (map) {
        if (!navigationControl) {
          navigationControl = new maplibregl.NavigationControl({
            showCompass: true,
            visualizePitch: true,
          });
          map.addControl(navigationControl, "bottom-right");
        }
        if (!scaleControl) {
          scaleControl = new maplibregl.ScaleControl({
            maxWidth: 110,
            unit: "metric",
          });
          map.addControl(scaleControl, "bottom-right");
        }
        if (!fullscreenControl) {
          fullscreenControl = new maplibregl.FullscreenControl();
          map.addControl(fullscreenControl, "bottom-right");
        }
        // A style switch drops the adapter's accuracy layers; re-add them.
        map.on("style.load", () => {
          if (!destroyed) applySelfLocation();
          if (!destroyed) applyTrail();
          if (!destroyed) applyMeasure();
        });
      }
      applyView();
      applySelectedMarker();
      applySelfLocation();
      applyRoute();
      applyTrail();
      applyMeasure();
      applyFit();
      if (readOnlinePreference()) void setOnline(true, { persist: false });
      if (typeof onLoad === "function") onLoad(api);
    },
    onError(error) {
      if (typeof onError === "function") {
        onError(error);
      } else {
        console.error("[pinpoint-map]", error);
      }
    },
  });

  const unsubscribeClick = base.onClick((point) => {
    if (destroyed) return;
    if (typeof onClick === "function") onClick(point);
  });

  // Right-click (desktop) opens the map actions menu at the pointer.
  function handleContextMenu(event) {
    if (destroyed) return;
    event.preventDefault();
    const live = base.getMap();
    if (!live) return;
    const rect = live.getCanvas().getBoundingClientRect();
    const point = live.unproject([event.clientX - rect.left, event.clientY - rect.top]);
    if (typeof onContextMenu === "function") {
      onContextMenu({
        lat: point.lat,
        lon: point.lng,
        x: event.clientX,
        y: event.clientY,
      });
    }
  }
  container.addEventListener("contextmenu", handleContextMenu);

  base.setTheme(currentTheme);

  function onOnlineToggle() {
    void setOnline(Boolean(onlineToggle?.checked));
  }

  onlineToggle?.addEventListener("change", onOnlineToggle);
  if (onlineToggle) onlineToggle.checked = readOnlinePreference();
  renderOnlineUi();

  /** View facade with latitude/longitude-array semantics for `nav.js`. */
  const map = {
    getCenter() {
      const live = base.getMap()?.getCenter();
      return live
        ? { lat: live.lat, lng: live.lng }
        : { lat: initialCenter[0], lng: initialCenter[1] };
    },
    getZoom() {
      return base.getMap()?.getZoom() ?? initialZoom;
    },
    flyTo(latLng, targetZoom, options = {}) {
      const lat = Number(latLng?.[0]);
      const lon = Number(latLng?.[1]);
      if (!Number.isFinite(lat) || !Number.isFinite(lon)) return;
      const nextZoom = Number(targetZoom);
      const duration = Number(options?.duration);
      const view = {
        center: [lon, lat],
        zoom: Number.isFinite(nextZoom) ? nextZoom : map.getZoom(),
        duration: Number.isFinite(duration)
          ? Math.max(0, Math.round(duration * 1000))
          : 500,
      };
      const live = base.getMap();
      if (!live) {
        pendingView = { type: "fly", view };
        return;
      }
      live.flyTo(view);
    },
    panTo(latLng, options = {}) {
      const lat = Number(latLng?.[0]);
      const lon = Number(latLng?.[1]);
      if (!Number.isFinite(lat) || !Number.isFinite(lon)) return;
      const view = {
        center: [lon, lat],
        animate: options?.animate !== false,
      };
      const live = base.getMap();
      if (!live) {
        pendingView = { type: "pan", view };
        return;
      }
      live.panTo(view.center, { animate: view.animate });
    },
    invalidateSize() {
      base.resize();
    },
    resize() {
      base.resize();
    },
    getMap() {
      return base.getMap();
    },
  };

  const api = {
    map,
    setFriendMarkers,
    setSelectedPlace,
    setSelfLocation,
    clearSelfLocation,
    drawRoute,
    clearRoute,
    fitRoute,
    drawTrail,
    clearTrail,
    setMeasurePoints,
    clearMeasure,
    setNoteMarkers,
    locate,
    setTheme,
    getTheme,
    setOnline,
    resize,
    destroy,
    getMap() {
      return base.getMap();
    },
  };

  // -------------------------------------------------------------------------
  // Friend pins
  // -------------------------------------------------------------------------

  /** Replace friend pins with the foundation's precision-coloured circles. */
  function setFriendMarkers(locations) {
    if (destroyed) return 0;
    return base.setLocations(locations);
  }

  // -------------------------------------------------------------------------
  // Selected place / current position
  // -------------------------------------------------------------------------

  function setSelectedPlace(place) {
    if (destroyed) return;
    if (!place) {
      selected = null;
      applySelectedMarker();
      return;
    }
    const lat = Number(place.latitude ?? place.lat);
    const lon = Number(place.longitude ?? place.lon);
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) {
      selected = null;
      applySelectedMarker();
      return;
    }
    selected = {
      lat,
      lon,
      label: String(place.short_name || place.display_name || "Selected"),
    };
    applySelectedMarker();
  }

  function applySelectedMarker() {
    if (!mapReady) return;
    const live = base.getMap();
    if (!live) return;
    selectedMarker?.remove();
    selectedMarker = null;
    if (!selected) return;
    const element = document.createElement("div");
    element.className = "pin-selected";
    const dot = document.createElement("span");
    dot.className = "pin-dot";
    const label = document.createElement("span");
    label.className = "pin-label";
    label.textContent = selected.label;
    element.append(dot, label);
    selectedMarker = new maplibregl.Marker({ element, anchor: "center" })
      .setLngLat([selected.lon, selected.lat])
      .addTo(live);
  }

  function setSelfLocation(fix, { fly = false } = {}) {
    if (destroyed) return;
    const lat = Number(fix?.lat ?? fix?.latitude);
    const lon = Number(fix?.lon ?? fix?.longitude);
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) return;
    const accuracy = Number(fix?.accuracy ?? fix?.accuracy_m ?? 0);
    self = {
      lat,
      lon,
      accuracy: Number.isFinite(accuracy) && accuracy > 0 ? accuracy : 0,
    };
    applySelfLocation();
    if (fly) {
      map.flyTo([lat, lon], Math.max(map.getZoom(), 15), { duration: 0.6 });
    }
  }

  function clearSelfLocation() {
    if (destroyed) return;
    self = null;
    applySelfLocation();
  }

  function applySelfLocation() {
    if (!mapReady) return;
    const live = base.getMap();
    if (!live) return;

    selfMarker?.remove();
    selfMarker = null;

    const data =
      self && self.accuracy > 0 && self.accuracy < 5000
        ? {
            type: "FeatureCollection",
            features: [accuracyPolygon(self.lat, self.lon, self.accuracy)],
          }
        : EMPTY_FEATURES;

    if (!live.getSource(SELF_SOURCE)) {
      live.addSource(SELF_SOURCE, { type: "geojson", data });
      addLayerBelowPins(live, {
        id: SELF_FILL_LAYER,
        type: "fill",
        source: SELF_SOURCE,
        paint: {
          "fill-color": "#34d399",
          "fill-opacity": 0.08,
        },
      });
      addLayerBelowPins(live, {
        id: SELF_LINE_LAYER,
        type: "line",
        source: SELF_SOURCE,
        paint: {
          "line-color": "#34d399",
          "line-width": 1,
          "line-opacity": 0.5,
          "line-dasharray": [4, 4],
        },
      });
    } else {
      live.getSource(SELF_SOURCE).setData(data);
    }

    if (!self) return;
    const element = document.createElement("div");
    element.className = "pin-self";
    const dot = document.createElement("span");
    dot.className = "pin-dot";
    const label = document.createElement("span");
    label.className = "pin-label";
    label.textContent = "You";
    element.append(dot, label);
    selfMarker = new maplibregl.Marker({ element, anchor: "center" })
      .setLngLat([self.lon, self.lat])
      .addTo(live);
  }

  function locate() {
    return new Promise((resolve, reject) => {
      if (!navigator.geolocation) {
        reject(new Error("Geolocation is not available in this browser."));
        return;
      }
      navigator.geolocation.getCurrentPosition(
        (position) => {
          const fix = {
            lat: position.coords.latitude,
            lon: position.coords.longitude,
            accuracy: position.coords.accuracy,
          };
          setSelfLocation(fix, { fly: true });
          resolve(fix);
        },
        (error) => {
          const messages = {
            1: "Location permission is blocked. Allow it in your browser's site settings, then try again.",
            2: "Location is unavailable. Turn on device Location and try again.",
            3: "Location request timed out. Try again.",
          };
          reject(new Error(messages[error?.code] ?? "Location unavailable - try again."));
        },
        { enableHighAccuracy: true, timeout: 8000, maximumAge: 30000 },
      );
    });
  }

  // -------------------------------------------------------------------------
  // Route
  // -------------------------------------------------------------------------

  function drawRoute(nextRoute, stops = []) {
    if (destroyed) return;
    const points = routePoints(nextRoute);
    if (points.length < 2) {
      route = null;
      applyRoute();
      return;
    }
    const via = (Array.isArray(stops) ? stops : [])
      .map((stop) => [Number(stop?.lat), Number(stop?.lon)])
      .filter(([lat, lon]) => Number.isFinite(lat) && Number.isFinite(lon));
    route = { points, stops: via };
    applyRoute();
  }

  function fitRoute(nextRoute) {
    if (destroyed) return;
    const points = routePoints(nextRoute);
    if (points.length < 2) return;
    pendingFit = points;
    applyFit();
  }

  function applyFit() {
    if (!mapReady || !pendingFit) return;
    const live = base.getMap();
    if (!live) return;
    const points = pendingFit;
    pendingFit = null;
    const lats = points.map(([lat]) => lat);
    const lons = points.map(([, lon]) => lon);
    live.fitBounds(
      [
        [Math.min(...lons), Math.min(...lats)],
        [Math.max(...lons), Math.max(...lats)],
      ],
      { padding: 48, maxZoom: 16, duration: 0 },
    );
  }

  function clearRoute() {
    if (destroyed) return;
    route = null;
    pendingFit = null;
    applyRoute();
  }

  // -------------------------------------------------------------------------
  // Daily trail and private notes
  // -------------------------------------------------------------------------

  function drawTrail(points) {
    trail = (Array.isArray(points) ? points : [])
      .map((point) => [Number(point.latitude ?? point.lat), Number(point.longitude ?? point.lon)])
      .filter(([lat, lon]) => Number.isFinite(lat) && Number.isFinite(lon));
    applyTrail();
  }

  function clearTrail() {
    trail = null;
    applyTrail();
  }

  function applyTrail() {
    if (!mapReady) return;
    const live = base.getMap();
    if (!live) return;
    if (live.getLayer(TRAIL_LAYER)) live.removeLayer(TRAIL_LAYER);
    if (live.getSource(TRAIL_SOURCE)) live.removeSource(TRAIL_SOURCE);
    if (!trail || trail.length < 2) return;
    live.addSource(TRAIL_SOURCE, {
      type: "geojson",
      data: {
        type: "Feature",
        properties: {},
        geometry: {
          type: "LineString",
          coordinates: trail.map(([lat, lon]) => [lon, lat]),
        },
      },
    });
    addLayerBelowPins(live, {
      id: TRAIL_LAYER,
      type: "line",
      source: TRAIL_SOURCE,
      layout: { "line-cap": "round", "line-join": "round" },
      paint: { "line-color": "#e3a458", "line-width": 4, "line-opacity": 0.85 },
    });
  }

  function setNoteMarkers(notes, onOpen) {
    if (destroyed) return;
    noteMarkers.forEach((marker) => marker.remove());
    noteMarkers = [];
    const live = base.getMap();
    if (!live || !Array.isArray(notes)) return;
    for (const note of notes) {
      const lat = Number(note.latitude);
      const lon = Number(note.longitude);
      if (!Number.isFinite(lat) || !Number.isFinite(lon)) continue;
      const element = document.createElement("button");
      element.type = "button";
      element.className = "note-marker";
      element.title = note.label || "Private note";
      element.setAttribute("aria-label", `Open note: ${note.label || note.message}`);
      if (
        typeof note.photo === "string" &&
        /^data:image\/(jpeg|png);base64,/.test(note.photo)
      ) {
        const photo = document.createElement("img");
        photo.className = "note-marker-photo";
        photo.src = note.photo;
        photo.alt = "";
        element.appendChild(photo);
      } else {
        element.textContent = "✎";
      }
      element.addEventListener("click", (event) => {
        event.preventDefault();
        onOpen?.(note);
      });
      const marker = new maplibregl.Marker({ element, anchor: "bottom" })
        .setLngLat([lon, lat])
        .addTo(live);
      noteMarkers.push(marker);
    }
  }

  // -------------------------------------------------------------------------
  // Distance measurement
  // -------------------------------------------------------------------------

  function setMeasurePoints(points) {
    measurePoints = (Array.isArray(points) ? points : [])
      .map(([lat, lon]) => [Number(lat), Number(lon)])
      .filter(([lat, lon]) => Number.isFinite(lat) && Number.isFinite(lon));
    applyMeasure();
  }

  function clearMeasure() {
    measurePoints = [];
    measureMarkers.forEach((marker) => marker.remove());
    measureMarkers = [];
    const live = base.getMap();
    if (!live) return;
    if (live.getLayer(MEASURE_LAYER)) live.removeLayer(MEASURE_LAYER);
    if (live.getSource(MEASURE_SOURCE)) live.removeSource(MEASURE_SOURCE);
  }

  function applyMeasure() {
    if (!mapReady) return;
    const live = base.getMap();
    if (!live) return;
    measureMarkers.forEach((marker) => marker.remove());
    measureMarkers = [];
    if (!measurePoints.length) {
      if (live.getLayer(MEASURE_LAYER)) live.removeLayer(MEASURE_LAYER);
      if (live.getSource(MEASURE_SOURCE)) live.removeSource(MEASURE_SOURCE);
      return;
    }
    const line =
      measurePoints.length >= 2
        ? {
            type: "Feature",
            properties: {},
            geometry: {
              type: "LineString",
              coordinates: measurePoints.map(([lat, lon]) => [lon, lat]),
            },
          }
        : EMPTY_FEATURES;
    if (!live.getSource(MEASURE_SOURCE)) {
      live.addSource(MEASURE_SOURCE, { type: "geojson", data: line });
      addLayerBelowPins(live, {
        id: MEASURE_LAYER,
        type: "line",
        source: MEASURE_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": "#f59e0b",
          "line-width": 4,
          "line-dasharray": [2, 1.5],
        },
      });
    } else {
      live.getSource(MEASURE_SOURCE).setData(line);
    }
    measurePoints.forEach(([lat, lon], index) => {
      const element = document.createElement("div");
      element.className = "measure-dot";
      element.textContent = String(index + 1);
      measureMarkers.push(
        new maplibregl.Marker({ element, anchor: "center" })
          .setLngLat([lon, lat])
          .addTo(live),
      );
    });
  }

  function applyRoute() {
    if (!mapReady) return;
    const live = base.getMap();
    if (!live) return;

    removeRoute(live);
    if (!route || route.points.length < 2) return;

    const data = {
      type: "Feature",
      properties: {},
      geometry: {
        type: "LineString",
        coordinates: route.points.map(([lat, lon]) => [lon, lat]),
      },
    };

    live.addSource(ROUTE_SOURCE, { type: "geojson", data });
    addLayerBelowPins(live, {
      id: ROUTE_CASING_LAYER,
      type: "line",
      source: ROUTE_SOURCE,
      layout: { "line-cap": "round", "line-join": "round" },
      paint: { "line-color": "#020617", "line-width": 9, "line-opacity": 0.85 },
    });
    addLayerBelowPins(live, {
      id: ROUTE_LINE_LAYER,
      type: "line",
      source: ROUTE_SOURCE,
      layout: { "line-cap": "round", "line-join": "round" },
      paint: { "line-color": "#34d399", "line-width": 5 },
    });

    const first = route.points[0];
    const last = route.points[route.points.length - 1];
    routeMarkers.push(addRouteMarker(live, "A", "pin-a", first));
    routeMarkers.push(addRouteMarker(live, "B", "pin-b", last));
    route.stops.forEach((point, index) => {
      routeMarkers.push(
        addRouteMarker(live, String(index + 1), "pin-stop", point),
      );
    });
  }

  function addRouteMarker(live, text, dotClass, [lat, lon]) {
    const element = document.createElement("div");
    element.className = "pin-route-end";
    const dot = document.createElement("span");
    dot.className = `pin-dot ${dotClass}`;
    dot.textContent = text;
    element.appendChild(dot);
    return new maplibregl.Marker({ element, anchor: "center" })
      .setLngLat([lon, lat])
      .addTo(live);
  }

  function removeRoute(live) {
    for (const marker of routeMarkers) marker.remove();
    routeMarkers = [];
    for (const layer of [ROUTE_LINE_LAYER, ROUTE_CASING_LAYER]) {
      if (live.getLayer(layer)) live.removeLayer(layer);
    }
    if (live.getSource(ROUTE_SOURCE)) live.removeSource(ROUTE_SOURCE);
  }

  // -------------------------------------------------------------------------
  // View / lifecycle / theme
  // -------------------------------------------------------------------------

  function applyView() {
    if (!mapReady || !pendingView) return;
    const live = base.getMap();
    if (!live) return;
    const { type, view } = pendingView;
    pendingView = null;
    if (type === "fly") live.flyTo(view);
    else live.panTo(view.center, { animate: view.animate });
  }

  function setTheme(next) {
    if (destroyed) return currentTheme;
    currentTheme = next === "dark" ? "dark" : "light";
    base.setTheme(currentTheme);
    return currentTheme;
  }

  function getTheme() {
    return currentTheme;
  }

  // -------------------------------------------------------------------------
  // Online basemap (opt-in)
  // -------------------------------------------------------------------------

  /**
   * Opt in to the worldwide OpenFreeMap Liberty style, or restore the local
   * PMTiles style. The preference is stored on this device; a failed online
   * switch reverts to the local style and shows an inline error.
   *
   * @param {boolean} enabled
   * @param {{persist?: boolean}} [options]
   * @returns {Promise<boolean>} whether the online style is active
   */
  async function setOnline(enabled, { persist = true } = {}) {
    if (destroyed) return false;
    const next = Boolean(enabled);
    if (persist) saveOnlinePreference(next);
    if (next && !onlineStyleUrl) {
      online = false;
      saveOnlinePreference(false);
      if (onlineToggle) onlineToggle.checked = false;
      renderOnlineUi();
      showOnlineError("Online map is not configured.");
      return false;
    }

    // Before the first load there is no style to swap; onLoad reads the
    // stored preference and applies it.
    if (!mapReady) {
      if (onlineToggle) onlineToggle.checked = next;
      return next;
    }
    if (next === online && !onlineBusy) {
      if (onlineToggle) onlineToggle.checked = next;
      return online;
    }
    if (next && typeof navigator !== "undefined" && navigator.onLine === false) {
      online = false;
      saveOnlinePreference(false);
      if (onlineToggle) onlineToggle.checked = false;
      renderOnlineUi();
      showOnlineError(
        "This device is offline — the online map needs a connection.",
      );
      return false;
    }

    const seq = ++onlineSeq;
    onlineBusy = true;
    showOnlineError("");
    renderOnlineUi();

    try {
      if (next) {
        await withTimeout(base.setStyle(onlineStyleUrl), ONLINE_TIMEOUT_MS);
        if (destroyed || seq !== onlineSeq) return online;
        online = true;
      } else {
        await base.setStyle(localStyleUrl, localPmtilesUrl);
        if (destroyed || seq !== onlineSeq) return online;
        online = false;
      }
    } catch {
      if (destroyed || seq !== onlineSeq) return online;
      if (next) {
        online = false;
        saveOnlinePreference(false);
        try {
          await base.setStyle(localStyleUrl, localPmtilesUrl);
        } catch {
          // base.onError reports a local style failure too.
        }
        if (destroyed || seq !== onlineSeq) return online;
        showOnlineError("Online map unavailable — using the offline map.");
      } else {
        online = true;
        saveOnlinePreference(true);
        showOnlineError("Could not restore the offline map.");
      }
    } finally {
      if (seq === onlineSeq) {
        onlineBusy = false;
        if (onlineToggle) onlineToggle.checked = online;
        renderOnlineUi();
      }
    }
    return online;
  }

  function renderOnlineUi() {
    if (onlineToggle) {
      onlineToggle.disabled = onlineBusy;
      onlineToggle.setAttribute("aria-busy", String(onlineBusy));
    }
    attributionOffline?.classList.toggle("hidden", online);
    attributionOnline?.classList.toggle("hidden", !online);
  }

  function showOnlineError(message) {
    if (!onlineError) return;
    onlineError.textContent = message || "";
    onlineError.classList.toggle("hidden", !message);
  }

  function resize() {
    if (destroyed) return;
    base.resize();
    if (mapReady) {
      applySelectedMarker();
      applySelfLocation();
    }
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    onlineSeq += 1;
    onlineBusy = false;
    onlineToggle?.removeEventListener("change", onOnlineToggle);
    unsubscribeClick();
    container.removeEventListener("contextmenu", handleContextMenu);
    selectedMarker?.remove();
    selectedMarker = null;
    selfMarker?.remove();
    selfMarker = null;
    routeMarkers.forEach((marker) => marker.remove());
    routeMarkers = [];
    measureMarkers.forEach((marker) => marker.remove());
    measureMarkers = [];
    const live = base.getMap();
    if (live) {
      navigationControl?.remove?.();
      scaleControl?.remove?.();
      fullscreenControl?.remove?.();
      removeRoute(live);
      if (live.getLayer(SELF_FILL_LAYER)) live.removeLayer(SELF_FILL_LAYER);
      if (live.getLayer(SELF_LINE_LAYER)) live.removeLayer(SELF_LINE_LAYER);
      if (live.getSource(SELF_SOURCE)) live.removeSource(SELF_SOURCE);
    }
    navigationControl = null;
    scaleControl = null;
    fullscreenControl = null;
    base.destroy();
  }

  return api;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function readConfig(key, metaName) {
  const config = globalThis.PINPOINT_MAP_CONFIG;
  const configured = config && typeof config === "object" ? config[key] : null;
  if (typeof configured === "string" && configured.trim()) return configured.trim();
  const meta = document.querySelector(`meta[name="${metaName}"]`);
  const content = meta?.getAttribute("content");
  return content && content.trim() ? content.trim() : null;
}

function readOnlinePreference() {
  try {
    return localStorage.getItem(ONLINE_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function saveOnlinePreference(enabled) {
  try {
    localStorage.setItem(ONLINE_STORAGE_KEY, enabled ? "1" : "0");
  } catch {
    // Private browsing: the choice lasts this page.
  }
}

/** Reject if a style switch hangs, so the toggle cannot stay disabled. */
function withTimeout(promise, timeoutMs) {
  let timer = null;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`Map style did not load within ${timeoutMs} ms`)),
      timeoutMs,
    );
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}

function routePoints(route) {
  return (route?.points || [])
    .map(([lat, lon]) => [Number(lat), Number(lon)])
    .filter(([lat, lon]) => Number.isFinite(lat) && Number.isFinite(lon));
}

/** Insert a layer below the foundation's friend circles when they exist. */
function addLayerBelowPins(map, layer) {
  const before = map.getLayer(FRIEND_LAYER) ? FRIEND_LAYER : undefined;
  map.addLayer(layer, before);
}

/** Approximate a geodesic circle as a GeoJSON polygon. */
function accuracyPolygon(lat, lon, radiusMeters) {
  const ring = [];
  for (let step = 0; step <= 64; step += 1) {
    ring.push(destination(lat, lon, (step * 360) / 64, radiusMeters));
  }
  return {
    type: "Feature",
    properties: {},
    geometry: { type: "Polygon", coordinates: [ring] },
  };
}

function destination(lat, lon, bearingDeg, distanceMeters) {
  const earthRadius = 6371000;
  const angular = distanceMeters / earthRadius;
  const bearing = (bearingDeg * Math.PI) / 180;
  const lat1 = (lat * Math.PI) / 180;
  const lon1 = (lon * Math.PI) / 180;
  const lat2 = Math.asin(
    Math.sin(lat1) * Math.cos(angular) +
      Math.cos(lat1) * Math.sin(angular) * Math.cos(bearing),
  );
  const lon2 =
    lon1 +
    Math.atan2(
      Math.sin(bearing) * Math.sin(angular) * Math.cos(lat1),
      Math.cos(angular) - Math.sin(lat1) * Math.sin(lat2),
    );
  return [
    (((lon2 * 180) / Math.PI + 540) % 360) - 180,
    (lat2 * 180) / Math.PI,
  ];
}

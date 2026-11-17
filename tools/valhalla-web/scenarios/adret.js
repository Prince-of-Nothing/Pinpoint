import { loadValhalla, MemoryTileSource, tilesInBounds } from "/node_modules/@adret-maps/valhalla-wasm/lib/index.js";
import { parseTar } from "/lib/tar.mjs";
import { memorySnapshot, serializeError, summarizeRoute } from "/lib/metrics.mjs";

const TAR_URL = "/data/moldova-valhalla.tar";
const ORIGIN = { lat: 47.01, lon: 28.86 };
const DESTINATION = { lat: 47.76, lon: 27.93 };
const params = new URLSearchParams(location.search);
const subset = params.get("subset");
// A generous corridor around Chișinău–Bălți. The engine needs every tile its
// search visits, so a production client must widen this per route and profile.
const CORRIDOR = { west: 27.8, south: 47.0, east: 28.95, north: 47.8 };

export async function run() {
  const started = performance.now();
  const result = {
    scenario: "adret",
    package: "@adret-maps/valhalla-wasm",
    tarUrl: TAR_URL,
    timings: {},
    tiles: {},
    routes: {},
  };
  try {
    const fetchStart = performance.now();
    const response = await fetch(TAR_URL);
    if (!response.ok) throw new Error(`tar fetch ${response.status}`);
    const buffer = await response.arrayBuffer();
    result.timings.fetchTarMs = performance.now() - fetchStart;
    result.tarBytes = buffer.byteLength;

    const parseStart = performance.now();
    const parsed = parseTar(buffer);
    result.timings.parseTarMs = performance.now() - parseStart;
    result.tiles.count = parsed.size;
    result.tiles.bytes = [...parsed.values()].reduce((sum, tile) => sum + tile.byteLength, 0);
    result.tiles.paths = [...parsed.keys()].slice(0, 12);

    const wanted = subset === "corridor" ? new Set(tilesInBounds(CORRIDOR).map((tile) => tile.path)) : null;
    result.tiles.requested = wanted ? wanted.size : parsed.size;
    const tiles = new MemoryTileSource();
    for (const [tilePath, bytes] of parsed) {
      if (!wanted || wanted.has(tilePath)) tiles.set(tilePath, bytes);
    }
    result.tiles.loaded = tiles.size;
    result.tiles.loadedBytes = tiles.bytes;
    result.tiles.subset = subset ?? "all";

    const loadStart = performance.now();
    const valhalla = await loadValhalla({ tiles });
    result.timings.loadMs = performance.now() - loadStart;
    result.wasmMemoryBytesAfterLoad = valhalla.memoryBytes;

    for (const costing of ["auto", "pedestrian", "bicycle"]) {
      const routeStart = performance.now();
      try {
        const route = valhalla.route({
          locations: [ORIGIN, DESTINATION],
          costing,
          language: "en-US",
          // "maneuvers" returns geometry but leaves instruction text empty;
          // "instructions" still returns maneuvers and fills the text.
          directions_type: "instructions",
        });
        result.routes[costing] = {
          ok: true,
          ms: performance.now() - routeStart,
          ...summarizeRoute(route),
          response: route,
          wasmMemoryBytes: valhalla.memoryBytes,
        };
      } catch (error) {
        result.routes[costing] = { ok: false, ms: performance.now() - routeStart, error: serializeError(error) };
      }
    }
    result.timings.totalMs = performance.now() - started;
    result.ok = Object.values(result.routes).some((route) => route.ok);
    result.memory = await memorySnapshot();
  } catch (error) {
    result.ok = false;
    result.error = serializeError(error);
    result.memory = await memorySnapshot();
  }
  return result;
}

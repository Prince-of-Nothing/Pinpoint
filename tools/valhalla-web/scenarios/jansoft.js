import { createRouter } from "/node_modules/@jansoft/mbujkanji-valhalla-wasm/dist/index.js";
import { memorySnapshot, serializeError, summarizeRoute } from "/lib/metrics.mjs";

const TAR_URL = "/data/moldova-valhalla.tar";
const PACKAGE_BASE = "/node_modules/@jansoft/mbujkanji-valhalla-wasm/dist/";
const ORIGIN = { lat: 47.01, lon: 28.86 };
const DESTINATION = { lat: 47.76, lon: 27.93 };

export async function run() {
  const started = performance.now();
  const result = {
    scenario: "jansoft",
    package: "@jansoft/mbujkanji-valhalla-wasm",
    tarUrl: TAR_URL,
    timings: {},
    routes: {},
  };
  const router = createRouter({ verbose: true });
  try {
    const initStart = performance.now();
    await router.init({
      wasmPath: PACKAGE_BASE + "valhalla.wasm",
      jsGluePath: PACKAGE_BASE + "valhalla.js",
      onProgress: (progress) => console.info("init-progress", JSON.stringify(progress)),
    });
    result.timings.initMs = performance.now() - initStart;
    result.afterInit = router.getStatus();

    const fetchStart = performance.now();
    const response = await fetch(TAR_URL);
    if (!response.ok) throw new Error(`tar fetch ${response.status}`);
    const buffer = await response.arrayBuffer();
    result.timings.fetchTarMs = performance.now() - fetchStart;
    result.tarBytes = buffer.byteLength;

    const tilesStart = performance.now();
    try {
      await router.loadTiles(buffer, {
        regionId: "moldova",
        onProgress: (progress) => console.info("tile-progress", JSON.stringify(progress)),
      });
      result.timings.loadTilesMs = performance.now() - tilesStart;
      result.loadTiles = { ok: true };
      result.afterLoad = router.getStatus();
    } catch (error) {
      result.timings.loadTilesMs = performance.now() - tilesStart;
      result.loadTiles = { ok: false, error: serializeError(error) };
    }

    if (result.loadTiles.ok) {
      for (const costing of ["auto", "pedestrian"]) {
        const routeStart = performance.now();
        try {
          const route = await router.route({
            locations: [ORIGIN, DESTINATION],
            costing,
            language: "en-US",
            directions_type: "maneuvers",
          });
          result.routes[costing] = { ok: true, ms: performance.now() - routeStart, ...summarizeRoute(route), response: route };
        } catch (error) {
          result.routes[costing] = { ok: false, ms: performance.now() - routeStart, error: serializeError(error) };
        }
      }
    }
    result.timings.totalMs = performance.now() - started;
    result.ok = Object.values(result.routes).some((route) => route.ok);
    result.memory = await memorySnapshot();
  } catch (error) {
    result.ok = false;
    result.error = serializeError(error);
    result.memory = await memorySnapshot();
  } finally {
    try {
      router.dispose();
    } catch {
      // ignore dispose failures after a load failure
    }
  }
  return result;
}

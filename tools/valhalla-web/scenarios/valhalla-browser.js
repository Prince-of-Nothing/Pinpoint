import { createRouter } from "/node_modules/valhalla-browser/dist/index.js";
import { memorySnapshot, serializeError } from "/lib/metrics.mjs";

const ORIGIN = { lat: 47.01, lon: 28.86 };
const DESTINATION = { lat: 47.76, lon: 27.93 };
const params = new URLSearchParams(location.search);
const manifestUrl = params.get("manifest") || "/datasets/moldova-20261116/manifest.json";
const transport = params.get("transport") || undefined;

function routeSummary(native) {
  const leg = native?.trip?.legs?.[0];
  return {
    summary: native?.trip?.summary ?? null,
    status: native?.trip?.status,
    status_message: native?.trip?.status_message,
    language: native?.trip?.language,
    legs: native?.trip?.legs?.length ?? 0,
    maneuvers: leg?.maneuvers?.length ?? 0,
    firstManeuvers: leg?.maneuvers?.slice(0, 5) ?? [],
    shapeChars: leg?.shape?.length ?? 0,
  };
}

export async function run() {
  const started = performance.now();
  const result = {
    scenario: "valhalla-browser",
    package: "valhalla-browser",
    manifestUrl,
    timings: {},
    routes: {},
  };
  let router;
  try {
    const createStart = performance.now();
    router = await createRouter({ manifestUrl, ...(transport ? { transport } : {}) });
    result.timings.createRouterMs = performance.now() - createStart;
    result.startup = router.startup;

    for (const costing of ["auto", "pedestrian"]) {
      const routeStart = performance.now();
      try {
        const route = await router.route({
          origin: ORIGIN,
          destination: DESTINATION,
          costing,
        });
        result.routes[costing] = {
          ok: true,
          ms: performance.now() - routeStart,
          ...routeSummary(route.native),
          response: route.native,
          diagnostics: route.diagnostics,
          dataset: route.dataset,
        };
      } catch (error) {
        result.routes[costing] = { ok: false, ms: performance.now() - routeStart, error: serializeError(error) };
      }
    }
    try {
      result.diagnostics = await router.diagnostics();
    } catch (error) {
      result.diagnosticsError = serializeError(error);
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
      await router?.dispose();
    } catch {
      // ignore dispose failures after a load failure
    }
  }
  return result;
}

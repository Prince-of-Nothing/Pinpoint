/** Browser-side memory snapshot. UA-specific memory needs COOP/COEP. */
export async function memorySnapshot() {
  const out = {
    crossOriginIsolated: globalThis.crossOriginIsolated,
    userAgent: navigator.userAgent,
  };
  if (performance.memory) {
    out.jsHeap = {
      used: performance.memory.usedJSHeapSize,
      total: performance.memory.totalJSHeapSize,
      limit: performance.memory.jsHeapSizeLimit,
    };
  }
  try {
    out.uaSpecific = await performance.measureUserAgentSpecificMemory();
  } catch (error) {
    out.uaSpecificError = String(error?.message ?? error);
  }
  return out;
}

/** JSON-safe error, preserving the routing packages' typed fields. */
export function serializeError(error) {
  const out = {
    name: error?.name,
    message: error?.message,
    code: error?.code,
    status: error?.status,
    statusText: error?.statusText,
    nativeCode: error?.nativeCode,
    stack: error?.stack,
  };
  return Object.fromEntries(Object.entries(out).filter(([, value]) => value !== undefined));
}

export function summarizeRoute(response) {
  const trip = response?.trip;
  const leg = trip?.legs?.[0];
  return {
    summary: trip?.summary ?? null,
    status: trip?.status,
    status_message: trip?.status_message,
    units: trip?.units,
    language: trip?.language,
    legs: trip?.legs?.length ?? 0,
    maneuvers: leg?.maneuvers?.length ?? 0,
    firstManeuvers: leg?.maneuvers?.slice(0, 5) ?? [],
    shapeChars: leg?.shape?.length ?? 0,
  };
}

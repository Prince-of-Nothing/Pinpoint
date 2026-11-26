// Map services for the web client, as interchangeable providers.
//
// Place search asks providers in order and falls through when one is missing
// or fails: the installed offline pack, then the Pinpoint server's /map/search
// proxy, then on-device data (saved and recent places, and names in the map
// tiles already loaded). No single provider is required, so search keeps
// working offline or when the server is down, with whatever is available.
//
// Every provider returns places in the shape nav.js renders:
// { short_name, display_name, latitude, longitude, distance_m, category, source }.

const EARTH_RADIUS_M = 6_371_008.8;
const MAP_LAYERS = ["place", "poi", "transportation_name", "water_name", "mountain_peak", "aerodrome_label", "park"];

/** Case- and accent-insensitive form for matching ("Chișinău" ~ "chisinau"). */
export function normalizeText(value) {
  return String(value ?? "")
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .trim();
}

/** 3 = exact, 2 = starts with, 1 = a word starts with, 0 = no match. */
export function matchScore(name, query) {
  const n = normalizeText(name);
  const q = normalizeText(query);
  if (!n || !q) return 0;
  if (n === q) return 3;
  if (n.startsWith(q)) return 2;
  if (n.split(/[\s,.'’()-]+/).some((word) => word.startsWith(q))) return 1;
  return 0;
}

export function distanceMeters(a, b) {
  const rad = Math.PI / 180;
  const dLat = (b.lat - a.lat) * rad;
  const dLon = (b.lon - a.lon) * rad;
  const h =
    Math.sin(dLat / 2) ** 2 + Math.cos(a.lat * rad) * Math.cos(b.lat * rad) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(h)));
}

function withDistance(place, center) {
  if (!center) return place;
  const at = { lat: Number(place.latitude), lon: Number(place.longitude) };
  return { ...place, distance_m: distanceMeters(center, at) };
}

/** Saved places first, then recents, matched by name. */
export function savedPlacesProvider(getLists) {
  return {
    id: "saved",
    label: "Saved and recent places",
    async search(query, { center, limit = 8 } = {}) {
      const { saved = [], recents = [] } = getLists() || {};
      const seen = new Set();
      return [...saved.map((p) => [p, "Saved place"]), ...recents.map((p) => [p, "Recent place"])]
        .map(([place, kind]) => ({ place, kind, score: matchScore(place.short_name || place.display_name, query) }))
        .filter(({ score, place }) => {
          const key = `${place.display_name}|${Number(place.latitude).toFixed(5)}|${Number(place.longitude).toFixed(5)}`;
          if (!score || seen.has(key)) return false;
          seen.add(key);
          return true;
        })
        .sort((a, b) => b.score - a.score)
        .slice(0, limit)
        .map(({ place, kind }) =>
          withDistance(
            { ...place, category: place.category || kind, source: "saved", source_label: kind },
            center,
          ),
        );
    },
  };
}

function featurePoint(geometry) {
  if (!geometry) return null;
  const { type, coordinates } = geometry;
  if (type === "Point") return coordinates;
  if (type === "MultiPoint" || type === "LineString") return coordinates[Math.floor(coordinates.length / 2)];
  if (type === "MultiLineString") return featurePoint({ type: "LineString", coordinates: coordinates[0] });
  if (type === "Polygon") {
    const ring = coordinates[0] || [];
    if (!ring.length) return null;
    const sum = ring.reduce((acc, [x, y]) => [acc[0] + x, acc[1] + y], [0, 0]);
    return [sum[0] / ring.length, sum[1] / ring.length];
  }
  if (type === "MultiPolygon") return featurePoint({ type: "Polygon", coordinates: coordinates[0] });
  return null;
}

function vectorSourceIds(map) {
  const sources = map.getStyle?.()?.sources || {};
  return Object.keys(sources).filter((id) => sources[id]?.type === "vector");
}

/** Names in the vector tiles already loaded on the device (works offline). */
export function mapFeaturesProvider(getMap) {
  return {
    id: "map",
    label: "Names on the map",
    async search(query, { center, limit = 8 } = {}) {
      const map = getMap?.();
      if (!map?.querySourceFeatures) return [];
      const found = new Map();
      for (const source of vectorSourceIds(map)) {
        for (const sourceLayer of MAP_LAYERS) {
          let features = [];
          try {
            features = map.querySourceFeatures(source, { sourceLayer });
          } catch {
            continue; // the layer is not in this source's schema
          }
          for (const feature of features) {
            const props = feature.properties || {};
            const name = props.name || props["name:latin"] || props.name_en || props.name_int;
            const score = matchScore(name, query);
            if (!score) continue;
            const point = featurePoint(feature.geometry);
            if (!point || !point.every(Number.isFinite)) continue;
            const kind = String(props.subclass || props.class || sourceLayer).replace(/_/g, " ");
            const key = `${normalizeText(name)}|${kind}|${point[1].toFixed(3)}|${point[0].toFixed(3)}`;
            if (found.has(key)) continue;
            found.set(
              key,
              withDistance(
                {
                  short_name: String(name),
                  display_name: `${name}, ${kind}`,
                  latitude: point[1],
                  longitude: point[0],
                  category: kind,
                  source: "map",
                  source_label: "On the map",
                  score,
                },
                center,
              ),
            );
          }
        }
      }
      // Streets come as many segments: keep the nearest of each same-named
      // feature of one kind within a kilometre.
      const kept = [];
      const sorted = [...found.values()].sort(
        (a, b) => b.score - a.score || (a.distance_m ?? 0) - (b.distance_m ?? 0),
      );
      for (const place of sorted) {
        const duplicate = kept.some(
          (k) =>
            k.category === place.category &&
            normalizeText(k.short_name) === normalizeText(place.short_name) &&
            distanceMeters(
              { lat: k.latitude, lon: k.longitude },
              { lat: place.latitude, lon: place.longitude },
            ) < 1000,
        );
        if (!duplicate) kept.push(place);
        if (kept.length >= limit) break;
      }
      return kept.map(({ score, ...place }) => place);
    },
  };
}

/** The Pinpoint server's /map/search proxy. Throws on network or HTTP errors. */
export function serverSearchProvider(fetchImpl = (...args) => fetch(...args)) {
  return {
    id: "server",
    label: "Pinpoint server",
    async search(query, { center, limit = 8, signal } = {}) {
      const params = new URLSearchParams({ q: query, limit: String(limit) });
      if (center) {
        params.set("lat", center.lat.toFixed(5));
        params.set("lon", center.lon.toFixed(5));
      }
      const res = await fetchImpl(`/map/search?${params}`, { signal, credentials: "include" });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body?.error?.message || `Search failed (HTTP ${res.status}).`);
      return (Array.isArray(body.results) ? body.results : []).map((place) => ({ ...place, source: "server" }));
    },
  };
}

/** The installed offline pack's search index, or nothing when none is installed. */
export function offlinePackProvider(getOfflineSearch, toSearchPlace) {
  return {
    id: "offline",
    label: "Offline map",
    async available() {
      return Boolean(await getOfflineSearch());
    },
    async search(query, { center, limit = 8 } = {}) {
      const engine = await getOfflineSearch();
      if (!engine) return [];
      return engine
        .search(query, limit)
        .map((result) => ({ ...toSearchPlace(result, center), source: "offline" }));
    },
  };
}

/** Several providers answering as one (e.g. saved places plus map names). */
export function combinedProvider(id, label, providers) {
  return {
    id,
    label,
    async search(query, context = {}) {
      const lists = await Promise.all(providers.map((p) => p.search(query, context).catch(() => [])));
      return lists.flat().slice(0, context.limit ?? 8);
    },
  };
}

/**
 * Ask the providers in order. The first non-empty answer wins; a failing
 * provider is skipped and remembered. Returns which provider answered and the
 * last error, so the UI can say "offline results" or "server unreachable".
 *
 * @returns {Promise<{results: object[], source: string|null, error: Error|null}>}
 */
export async function searchPlaces(providers, query, context = {}) {
  let error = null;
  for (const provider of providers) {
    if (context.signal?.aborted) break;
    try {
      const results = await provider.search(query, context);
      if (results.length) return { results, source: provider.id, error };
    } catch (err) {
      if (err?.name === "AbortError") throw err;
      error = err;
    }
  }
  return { results: [], source: null, error };
}

/**
 * What the web client can reach right now, for the Account "Services" card.
 * Reads only same-origin endpoints and local state; it never probes the
 * third-party geocoder or router (they are rate-limited public services).
 */
export async function collectServiceStatus({
  fetchImpl = (...args) => fetch(...args),
  listPacks = async () => [],
  onlineMap = false,
  online = true,
} = {}) {
  const rows = [];
  let info = null;
  try {
    const res = await fetchImpl("/api/v1/services", { credentials: "include", cache: "no-store" });
    if (res.ok) info = await res.json();
  } catch {
    info = null;
  }
  let packs = [];
  try {
    packs = (await listPacks()) || [];
  } catch {
    packs = [];
  }
  const pack = packs[0];
  const host = (url) => {
    try {
      return new URL(url, "https://local.invalid").host || url;
    } catch {
      return String(url || "");
    }
  };
  rows.push(
    info
      ? { id: "server", name: "Pinpoint server", state: "ok", detail: `Connected, version ${info.version}, ${info.database} storage` }
      : { id: "server", name: "Pinpoint server", state: "down", detail: online ? "Not reachable right now" : "This device is offline" },
  );
  const searchVia = [pack ? "the offline map" : null, info ? host(info.integrations?.nominatim) : null, "this device"].filter(Boolean);
  rows.push({ id: "search", name: "Place search", state: "ok", detail: `Searches ${searchVia.join(", then ")}` });
  rows.push(
    info
      ? { id: "routing", name: "Directions", state: "ok", detail: `Via ${host(info.integrations?.osrm)}${info.integrations?.osrm_foot ? ", with walking" : ""}${info.integrations?.osrm_bike ? " and cycling" : ""}` }
      : { id: "routing", name: "Directions", state: "down", detail: "Needs the Pinpoint server" },
  );
  rows.push(
    pack
      ? { id: "tiles", name: "Map", state: "ok", detail: `Offline map installed (${pack.name || pack.id})${onlineMap ? ", online map on" : ""}` }
      : onlineMap
        ? { id: "tiles", name: "Map", state: online ? "ok" : "down", detail: online ? "Online map (OpenFreeMap)" : "Online map needs a connection" }
        : { id: "tiles", name: "Map", state: "off", detail: "No offline map installed and the online map is off" },
  );
  return rows;
}

// Offline search provider for the installed Moldova pack.
//
// The service worker serves `/offline-packs/<id>/<file>` from OPFS. When no
// pack is installed, or the service worker is not active, the SQLite fetch
// fails and the app keeps its existing same-origin `/map/search` behavior.
//
// The SQLite index is loaded once per page. A failed load is cached for the
// session: retrying a 6.8 MB fetch on every keystroke would be worse than
// falling back until the next reload.

const PACK_ID = "moldova";
const SQLITE_NAME = "moldova-search.sqlite";
const EARTH_RADIUS_M = 6_371_008.8;

let engine = null;
let opening = null;
let unavailable = false;

/**
 * Open the offline search index, or return `null` when no installed pack is
 * available. The result is memoised.
 *
 * @returns {Promise<{search: (query: string, limit?: number) => Array<object>, close: () => void}|null>}
 */
export async function getOfflineSearch() {
  if (engine) return engine;
  if (unavailable) return null;
  if (opening) return opening;

  opening = (async () => {
    try {
      const { openSearch } = await import("/map-assets/search.js");
      engine = await openSearch({
        sqliteUrl: `/offline-packs/${PACK_ID}/${SQLITE_NAME}`,
      });
      return engine;
    } catch (error) {
      unavailable = true;
      // Informational only: the server search fallback is expected when the
      // user has not installed the pack yet.
      console.info(
        "[pinpoint] offline search unavailable; using server search",
        error?.message || String(error),
      );
      return null;
    } finally {
      opening = null;
    }
  })();

  return opening;
}

/** Close the index and allow a later call to reopen it (for example after a pack install). */
export function resetOfflineSearch() {
  if (engine?.close) {
    try {
      engine.close();
    } catch (_) {
      // closing an already-closed handle is not an error worth surfacing
    }
  }
  engine = null;
  opening = null;
  unavailable = false;
}

/**
 * Convert an offline `{name, kind, lat, lon, address, importance}` result into
 * the place shape used by `nav.js`.
 *
 * @param {object} result
 * @param {{lat: number, lon: number}} [center]
 * @returns {object}
 */
export function toSearchPlace(result, center) {
  const lat = Number(result?.lat);
  const lon = Number(result?.lon);
  const name = String(result?.name || "Place");
  const address = result?.address ? String(result.address) : "";
  const display = address ? `${name}, ${address}` : name;
  const distance = Number.isFinite(center?.lat) && Number.isFinite(center?.lon)
    ? distanceMeters(center.lat, center.lon, lat, lon)
    : undefined;

  return {
    display_name: display,
    short_name: name,
    latitude: lat,
    longitude: lon,
    category: String(result?.kind || "place"),
    ...(Number.isFinite(distance) ? { distance_m: distance } : {}),
  };
}

/** Great-circle distance in metres. */
export function distanceMeters(lat1, lon1, lat2, lon2) {
  const toRad = (value) => (value * Math.PI) / 180;
  const dLat = toRad(lat2 - lat1);
  const dLon = toRad(lon2 - lon1);
  const a =
    Math.sin(dLat / 2) ** 2 +
    Math.cos(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.sqrt(a));
}

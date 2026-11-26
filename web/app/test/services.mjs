// Map services as interchangeable providers (web/app/services.js): matching,
// each provider on its own, the fall-through chain, and the Account status.
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const s = await import(pathToFileURL(path.resolve("web/app/services.js")).href);

// Matching is accent- and case-insensitive, and ranks exact > prefix > word.
assert.equal(s.normalizeText("Chișinău"), "chisinau");
assert.equal(s.matchScore("Chișinău", "chisinau"), 3);
assert.equal(s.matchScore("Orheiul Vechi", "orhei"), 2);
assert.equal(s.matchScore("Mall Dova", "dova"), 1);
assert.equal(s.matchScore("Mereni", "xyz"), 0);
assert.equal(s.matchScore("", "a"), 0);
assert.ok(Math.abs(s.distanceMeters({ lat: 47, lon: 28.8 }, { lat: 47.01, lon: 28.8 }) - 1112) < 5);

const center = { lat: 47.0, lon: 28.8 };

// Saved and recent places: matched by name, saved first, deduplicated.
{
  const home = { short_name: "Home", display_name: "Home, Botanica", latitude: 47.0, longitude: 28.85 };
  const p = s.savedPlacesProvider(() => ({ saved: [home], recents: [home, { short_name: "Gym", display_name: "Gym", latitude: 47, longitude: 28 }] }));
  const found = await p.search("home", { center });
  assert.equal(found.length, 1, "a place both saved and recent appears once");
  assert.equal(found[0].source, "saved");
  assert.equal(found[0].category, "Saved place");
  assert.ok(found[0].distance_m > 0);
  assert.deepEqual(await p.search("zzz", { center }), []);
}

// Names in loaded vector tiles: every vector source and known layer, any geometry.
{
  const asked = [];
  const map = {
    getStyle: () => ({ sources: { basemap: { type: "vector" }, sat: { type: "raster" } } }),
    querySourceFeatures(source, { sourceLayer }) {
      asked.push(`${source}/${sourceLayer}`);
      if (sourceLayer === "aerodrome_label") throw new Error("no such layer");
      if (sourceLayer === "place") return [
        { properties: { name: "Orhei", class: "town" }, geometry: { type: "Point", coordinates: [28.82, 47.38] } },
        { properties: { name: "Orhei", class: "town" }, geometry: { type: "Point", coordinates: [28.82, 47.38] } },
      ];
      if (sourceLayer === "transportation_name") return [
        { properties: { "name:latin": "Strada Orheiului", class: "secondary" }, geometry: { type: "LineString", coordinates: [[28.8, 47], [28.81, 47.01], [28.82, 47.02]] } },
      ];
      return [];
    },
  };
  const found = await s.mapFeaturesProvider(() => map).search("orhei", { center });
  assert.ok(asked.every((a) => a.startsWith("basemap/")), "raster sources are not queried");
  assert.deepEqual(found.map((f) => f.short_name), ["Orhei", "Strada Orheiului"], "exact first, duplicates merged");
  assert.equal(found[1].latitude, 47.01, "a line uses its middle point");
  assert.equal(found[0].source, "map");
  assert.deepEqual(await s.mapFeaturesProvider(() => null).search("x"), [], "no map yet is not an error");

  // A street split into segments is one result (the nearest segment).
  const segments = {
    getStyle: () => ({ sources: { openmaptiles: { type: "vector" } } }),
    querySourceFeatures: (_s, { sourceLayer }) =>
      sourceLayer === "transportation_name"
        ? [
            { properties: { name: "Bulevardul Ștefan cel Mare", class: "primary" }, geometry: { type: "Point", coordinates: [28.806, 47.003] } },
            { properties: { name: "Bulevardul Ștefan cel Mare", class: "primary" }, geometry: { type: "Point", coordinates: [28.802, 47.0] } },
            { properties: { name: "Bulevardul Ștefan cel Mare", class: "primary" }, geometry: { type: "Point", coordinates: [29.5, 47.5] } },
          ]
        : [],
  };
  const streets = await s.mapFeaturesProvider(() => segments).search("stefan", { center });
  assert.equal(streets.length, 2, "segments within a kilometre merge; a far one stays separate");
  assert.equal(streets[0].longitude, 28.802, "the nearest segment is kept");
}

// The Pinpoint server: query and centre in the URL, HTTP errors throw.
{
  let url;
  const ok = s.serverSearchProvider(async (u) => {
    url = u;
    return { ok: true, status: 200, json: async () => ({ results: [{ short_name: "Albina" }] }) };
  });
  const found = await ok.search("Albina", { center, limit: 5 });
  assert.equal(found[0].source, "server");
  assert.match(url, /^\/map\/search\?q=Albina&limit=5&lat=47\.00000&lon=28\.80000$/);
  const bad = s.serverSearchProvider(async () => ({ ok: false, status: 503, json: async () => ({}) }));
  await assert.rejects(bad.search("x", {}), /HTTP 503/);
}

// The chain: first non-empty answer wins; failures are skipped and reported.
{
  const empty = { id: "a", search: async () => [] };
  const down = { id: "b", search: async () => { throw new TypeError("Failed to fetch"); } };
  const device = { id: "device", search: async () => [{ short_name: "Home" }] };
  const r1 = await s.searchPlaces([empty, down, device], "home");
  assert.equal(r1.source, "device");
  assert.match(String(r1.error), /Failed to fetch/);
  const r2 = await s.searchPlaces([empty, down], "home");
  assert.deepEqual(r2.results, []);
  assert.equal(r2.source, null);
  const abort = { id: "c", search: async () => { const e = new Error("stop"); e.name = "AbortError"; throw e; } };
  await assert.rejects(s.searchPlaces([abort, device], "x"), /stop/, "aborts are not swallowed");
  const combined = s.combinedProvider("device", "x", [device, down, { search: async () => [{ short_name: "Map" }] }]);
  assert.deepEqual((await combined.search("h")).map((p) => p.short_name), ["Home", "Map"], "one failing part does not sink the rest");
}

// Account status: same-origin only, honest about what works offline.
{
  const services = {
    version: "3.0.0",
    database: "in-memory",
    integrations: { nominatim: "https://nominatim.openstreetmap.org", osrm: "https://router.project-osrm.org", osrm_foot: "http://localhost:5001" },
  };
  const fetched = [];
  const up = await s.collectServiceStatus({
    fetchImpl: async (u) => {
      fetched.push(u);
      return { ok: true, json: async () => services };
    },
    listPacks: async () => [{ id: "moldova", name: "Moldova" }],
    onlineMap: true,
  });
  assert.deepEqual(fetched, ["/api/v1/services"], "only the same-origin status endpoint is read");
  const byId = Object.fromEntries(up.map((r) => [r.id, r]));
  assert.equal(byId.server.state, "ok");
  assert.match(byId.server.detail, /version 3\.0\.0, in-memory storage/);
  assert.equal(byId.search.detail, "Searches the offline map, then nominatim.openstreetmap.org, then this device");
  assert.equal(byId.routing.detail, "Via router.project-osrm.org, with walking");
  assert.match(byId.tiles.detail, /Offline map installed \(Moldova\), online map on/);

  const down = await s.collectServiceStatus({
    fetchImpl: async () => { throw new TypeError("Failed to fetch"); },
    online: false,
  });
  const d = Object.fromEntries(down.map((r) => [r.id, r]));
  assert.deepEqual([d.server.state, d.server.detail], ["down", "This device is offline"]);
  assert.equal(d.search.state, "ok", "search still works from the device");
  assert.equal(d.search.detail, "Searches this device");
  assert.equal(d.routing.state, "down");
  assert.equal(d.tiles.state, "off");
}

console.log("PASS  services: matching, providers, fall-through chain and status");

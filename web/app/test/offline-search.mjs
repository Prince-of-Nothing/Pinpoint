// Pure mapping checks for the offline search provider. The full OPFS/service
// worker flow is covered by the offline-pack smoke test.
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const moduleUrl = pathToFileURL(
  path.resolve("web/app/offline-search.js"),
).href;
const { toSearchPlace, distanceMeters } = await import(moduleUrl);

let passed = 0;
function check(name, fn) {
  try {
    fn();
    console.log(`PASS  ${name}`);
    passed += 1;
  } catch (error) {
    console.error(`FAIL  ${name}: ${error.message}`);
    process.exitCode = 1;
  }
}

check("maps the offline result into nav.js place fields", () => {
  const place = toSearchPlace(
    {
      name: "Bălți",
      kind: "city",
      lat: 47.76,
      lon: 27.93,
      address: "Municipiul Bălți",
      importance: 0.9,
    },
    { lat: 47.01, lon: 28.86 },
  );
  assert.equal(place.short_name, "Bălți");
  assert.equal(place.display_name, "Bălți, Municipiul Bălți");
  assert.equal(place.latitude, 47.76);
  assert.equal(place.longitude, 27.93);
  assert.equal(place.category, "city");
  assert.ok(place.distance_m > 100_000 && place.distance_m < 120_000);
});

check("falls back to the name when there is no address", () => {
  const place = toSearchPlace({ name: "Orhei", kind: "town", lat: 47.38, lon: 28.82 });
  assert.equal(place.display_name, "Orhei");
  assert.equal(place.short_name, "Orhei");
  assert.equal(place.distance_m, undefined);
});

check("uses a generic label when the name is missing", () => {
  const place = toSearchPlace({ kind: "address", lat: 47, lon: 28 });
  assert.equal(place.short_name, "Place");
  assert.equal(place.category, "address");
});

check("haversine matches the known Chișinău–Bălți straight-line distance", () => {
  const metres = distanceMeters(47.01, 28.86, 47.76, 27.93);
  assert.ok(metres > 100_000 && metres < 120_000, `got ${metres}`);
});

check("haversine is zero for the same point", () => {
  assert.equal(distanceMeters(47.01, 28.86, 47.01, 28.86), 0);
});

console.log(`offline search mapping checks passed: ${passed}`);

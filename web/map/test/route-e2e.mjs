/**
 * Browser E2E for the local turn-by-turn routing panel on the shared map page.
 *
 * Started by `tests/map_route_e2e.rs` against the real desktop loopback map
 * server. It fills From/To, presses Route, and asserts the local `/map/route`
 * response, the MapLibre casing/line layers, the summary and maneuvers, then
 * clears the route. Any external request, page error or console error fails.
 *
 * Usage (normally through the ignored Rust test):
 *   node web/map/test/route-e2e.mjs http://127.0.0.1:PORT/map/index.html
 *
 * Env:
 *   NODE_PATH            must contain puppeteer
 *   PINPOINT_SCREENSHOT  optional screenshot path
 */

import { mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";

const require = createRequire(import.meta.url);
const indexUrl = process.argv[2];
const screenshot =
  process.env.PINPOINT_SCREENSHOT || path.join(tmpdir(), "pinpoint-map-route.png");

if (!indexUrl) {
  console.error("usage: node web/map/test/route-e2e.mjs <map index URL>");
  process.exit(2);
}

const origin = new URL(indexUrl).origin;
const FROM = "47.0105, 28.8638";
const TO = "47.76, 27.93";

let failures = 0;
function check(condition, message, detail = "") {
  if (condition) {
    console.log(`PASS  ${message}`);
  } else {
    failures += 1;
    console.error(`FAIL  ${message}${detail ? ` — ${detail}` : ""}`);
  }
}

function formatDistance(meters) {
  return meters < 1000 ? `${Math.round(meters)} m` : `${(meters / 1000).toFixed(1)} km`;
}

function formatDuration(seconds) {
  if (seconds < 3600) return `${Math.max(1, Math.round(seconds / 60))} min`;
  const hours = Math.floor(seconds / 3600);
  return `${hours} h ${Math.round((seconds % 3600) / 60)} min`;
}

let puppeteer;
try {
  puppeteer = require("puppeteer");
} catch {
  console.error(
    "FAIL  puppeteer is not installed; set NODE_PATH to a node_modules that contains it",
  );
  process.exit(1);
}

mkdirSync(path.dirname(screenshot), { recursive: true });

const browser = await puppeteer.launch({
  headless: true,
  args: [
    "--no-sandbox",
    "--disable-setuid-sandbox",
    "--disable-dev-shm-usage",
    "--enable-unsafe-swiftshader",
    "--use-gl=angle",
    "--use-angle=swiftshader",
  ],
});

try {
  const page = await browser.newPage();
  await page.setViewport({ width: 1280, height: 1000, deviceScaleFactor: 1 });
  page.setDefaultTimeout(20000);

  const pageErrors = [];
  const consoleErrors = [];
  const failedRequests = [];
  const externalRequests = [];
  const routeRequests = [];

  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("request", (request) => {
    const url = request.url();
    if (!url.startsWith("http://") && !url.startsWith("https://")) return;
    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      return;
    }
    if (parsed.origin !== origin) {
      externalRequests.push(url);
      return;
    }
    if (parsed.pathname === "/map/route") routeRequests.push(url);
  });
  page.on("requestfailed", (request) => {
    const errorText = request.failure()?.errorText ?? "unknown";
    // MapLibre cancels in-flight tile reads when fitBounds moves the view;
    // an intentional cancellation is not a failed route request.
    if (errorText.includes("ERR_ABORTED")) return;
    failedRequests.push(`${request.url()} (${errorText})`);
  });

  await page.goto(indexUrl, { waitUntil: "load", timeout: 20000 });
  await page.waitForFunction(() => window.__pinpointMapReady === true);
  check(true, "MapLibre map loaded");

  await page.waitForFunction(() => window.__pinpointRoute?.ui?.element?.isConnected === true);
  check(true, "route panel mounted");

  const initial = await page.evaluate(() => ({
    from: document.querySelector('#route-panel input[name="from"]')?.value ?? "",
    to: document.querySelector('#route-panel input[name="to"]')?.value ?? "",
    autoDisabled:
      document.querySelector('#route-panel [data-profile="auto"]')?.disabled ?? null,
    walkDisabled:
      document.querySelector('#route-panel [data-profile="pedestrian"]')?.disabled ?? null,
    bikeDisabled:
      document.querySelector('#route-panel [data-profile="bicycle"]')?.disabled ?? null,
    attribution: document.querySelector(".map-attribution")?.textContent ?? "",
  }));
  check(initial.from === FROM, "From defaults to Chișinău", initial.from);
  check(initial.to === TO, "To defaults to Bălți", initial.to);
  check(initial.autoDisabled === false, "Auto profile is enabled");
  check(initial.walkDisabled === true, "Pedestrian profile is disabled honestly");
  check(initial.bikeDisabled === true, "Bicycle profile is disabled honestly");
  check(
    initial.attribution.includes("OpenStreetMap contributors") &&
      initial.attribution.includes("ODbL 1.0"),
    "visible OSM/ODbL attribution is kept",
  );

  async function fill(selector, value) {
    await page.$eval(
      selector,
      (element, nextValue) => {
        element.value = nextValue;
        element.dispatchEvent(new Event("input", { bubbles: true }));
        element.dispatchEvent(new Event("change", { bubbles: true }));
      },
      value,
    );
  }

  await fill('#route-panel input[name="from"]', FROM);
  await fill('#route-panel input[name="to"]', TO);
  await page.click('#route-panel [data-profile="auto"]');

  const responsePromise = page.waitForResponse(
    (response) => {
      const url = new URL(response.url());
      return url.origin === origin && url.pathname === "/map/route";
    },
    { timeout: 30000 },
  );
  await page.click('#route-panel button[data-action="route"]');

  let response;
  try {
    response = await responsePromise;
  } catch (error) {
    const state = await page.evaluate(() => ({
      rootState: document.querySelector(".pinpoint-route")?.dataset.state ?? "",
      error: document.querySelector('[data-role="error"]')?.textContent ?? "",
      errorHidden: document.querySelector('[data-role="error"]')?.hidden ?? null,
      summary: document.querySelector('[data-role="summary"]')?.textContent ?? "",
      from: document.querySelector('#route-panel input[name="from"]')?.value ?? "",
      to: document.querySelector('#route-panel input[name="to"]')?.value ?? "",
    }));
    console.error("DIAG route wait failed", JSON.stringify(state, null, 2));
    console.error("DIAG route requests", JSON.stringify(routeRequests, null, 2));
    console.error("DIAG page errors", pageErrors.join(" | "));
    console.error("DIAG console errors", consoleErrors.join(" | "));
    console.error("DIAG failed requests", failedRequests.join(" | "));
    throw error;
  }
  const body = await response.json();

  check(response.status() === 200, "local /map/route returned 200", String(response.status()));
  check(routeRequests.length === 1, "exactly one local route request", String(routeRequests.length));
  check(
    routeRequests[0]?.includes("profile=auto") &&
      routeRequests[0]?.includes("from_lat=47.0105") &&
      routeRequests[0]?.includes("to_lat=47.76"),
    "route request carries the parsed coordinates and Auto profile",
    routeRequests[0] ?? "missing",
  );
  check(Number(body.distance_m) > 0, "response has a positive distance", String(body.distance_m));
  check(Number(body.duration_s) > 0, "response has a positive duration", String(body.duration_s));
  check(Array.isArray(body.points) && body.points.length > 1, "response has route geometry");
  check(
    Array.isArray(body.maneuvers) && body.maneuvers.length > 0,
    "response has turn-by-turn maneuvers",
  );

  await page.waitForFunction(
    () => document.querySelector(".pinpoint-route")?.dataset.state === "ready",
  );

  const summary = await page.$eval('[data-role="summary"]', (element) =>
    element.textContent.replace(/\s+/g, " ").trim(),
  );
  check(summary.includes(formatDistance(body.distance_m)), "summary shows the distance", summary);
  check(summary.includes(formatDuration(body.duration_s)), "summary shows the duration", summary);
  check(summary.includes("Auto"), "summary shows the profile", summary);

  const steps = await page.$$eval('[data-role="steps"] li', (items) =>
    items.map((item) => item.textContent.replace(/\s+/g, " ").trim()),
  );
  check(
    steps.length === body.maneuvers.length,
    "directions list matches the maneuver count",
    `${steps.length} vs ${body.maneuvers.length}`,
  );
  check(
    steps[0]?.includes(body.maneuvers[0].instruction),
    "first maneuver is rendered",
    steps[0] ?? "missing",
  );

  await new Promise((resolve) => setTimeout(resolve, 900));
  const mapState = await page.evaluate(() => {
    const map = window.__pinpointPreview?.map;
    if (!map) return null;
    const source = map.getSource("pinpoint-route");
    const serialized = source?.serialize?.();
    return {
      source: Boolean(source),
      casing: Boolean(map.getLayer("pinpoint-route-casing")),
      line: Boolean(map.getLayer("pinpoint-route-line")),
      coordinates:
        source?._data?.geometry?.coordinates?.length ??
        serialized?.data?.geometry?.coordinates?.length ??
        0,
      panelPoints: window.__pinpointRoute?.ui?.getRoute()?.points?.length ?? 0,
      fitsRoute:
        map.getBounds().contains([28.8638, 47.0105]) &&
        map.getBounds().contains([27.93, 47.76]),
    };
  });
  check(mapState?.source === true, "GeoJSON route source exists");
  check(mapState?.casing === true, "route casing layer exists");
  check(mapState?.line === true, "route line layer exists");
  check(
    mapState?.coordinates === body.points.length || mapState?.panelPoints === body.points.length,
    "the drawn route uses the returned points",
    `${mapState?.coordinates} / ${mapState?.panelPoints} vs ${body.points.length}`,
  );
  check(mapState?.fitsRoute === true, "map fitted the route bounds");

  await page.screenshot({ path: screenshot });
  check(true, `screenshot saved (${screenshot})`);

  // Stop ends guidance but keeps the route; Clear removes it.
  await page.click('#route-panel button[data-action="nav"]');
  await page.waitForFunction(
    () => document.querySelector(".pinpoint-route")?.dataset.state === "navigating",
  );
  check(true, "Start enters guidance");
  await page.click('#route-panel button[data-action="nav"]');
  await page.waitForFunction(
    () => document.querySelector(".pinpoint-route")?.dataset.state === "ready",
  );
  const afterStop = await page.evaluate(() => ({
    source: Boolean(window.__pinpointPreview?.map?.getSource("pinpoint-route")),
    line: Boolean(window.__pinpointPreview?.map?.getLayer("pinpoint-route-line")),
  }));
  check(afterStop.source === true && afterStop.line === true, "Stop keeps the route line");

  await page.click('#route-panel button[data-action="clear"]');
  await page.waitForFunction(
    () => document.querySelector(".pinpoint-route")?.dataset.state === "idle",
  );
  const cleared = await page.evaluate(() => {
    const map = window.__pinpointPreview?.map;
    return {
      source: Boolean(map?.getSource("pinpoint-route")),
      casing: Boolean(map?.getLayer("pinpoint-route-casing")),
      line: Boolean(map?.getLayer("pinpoint-route-line")),
      steps: document.querySelectorAll('[data-role="steps"] li').length,
      summary: document.querySelector('[data-role="summary"]')?.textContent ?? "",
    };
  });
  check(cleared.source === false, "Clear removes the route source");
  check(cleared.casing === false, "Clear removes the route casing");
  check(cleared.line === false, "Clear removes the route line");
  check(cleared.steps === 0, "Clear empties the directions");
  check(cleared.summary === "", "Clear empties the summary");

  check(externalRequests.length === 0, "no external network requests", externalRequests.join(", "));
  check(failedRequests.length === 0, "no failed network requests", failedRequests.join(", "));
  check(pageErrors.length === 0, "no page errors", pageErrors.join(" | "));
  check(consoleErrors.length === 0, "no console errors", consoleErrors.join(" | "));

  console.log(
    `ROUTE_E2E ${JSON.stringify({
      status: response.status(),
      distance_m: body.distance_m,
      duration_s: body.duration_s,
      points: body.points.length,
      maneuvers: body.maneuvers.length,
      first: body.maneuvers[0]?.instruction ?? "",
      last: body.maneuvers.at(-1)?.instruction ?? "",
    })}`,
  );
} finally {
  await browser.close();
}

if (failures > 0) {
  console.error(`Map route E2E failed: ${failures} check(s)`);
  process.exit(1);
}
console.log("Map route E2E passed.");

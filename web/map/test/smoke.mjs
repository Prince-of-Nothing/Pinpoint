/**
 * Smoke test for the Pinpoint map foundation.
 *
 * Static checks always run. When Puppeteer is available (or `NODE_PATH`
 * points at a node_modules that has it), the test also opens the preview in
 * headless Chromium, waits for the MapLibre `load` event, verifies the canvas,
 * markers and local-only network traffic, and saves a screenshot.
 *
 * Usage (PowerShell):
 *   $env:NODE_PATH="C:\path\to\node_modules"
 *   node web/map/test/smoke.mjs
 *
 * Static-only fallback:
 *   node web/map/test/smoke.mjs --static-only
 *
 * Screenshot path: $env:PINPOINT_SCREENSHOT, otherwise
 * <system temp>/web-foundation.png.
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createStaticServer } from "../serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(HERE, "../..");
const FIXTURE = path.join(WEB_ROOT, "map", "fixtures", "monaco.pmtiles");
const FIXTURE_BYTES = 94389;
const FIXTURE_SHA256 = "a600dbfd9ddb25b82661f01d228a39d2b386dd13e401d2081b3ce7fd9c6de82f";
// Built at runtime so this guard file itself never contains the mojibake text.
const BROKEN_AC_ATTRIBUTION = "Ac" + " OpenStreetMap";
const SCREENSHOT = process.env.PINPOINT_SCREENSHOT || path.join(tmpdir(), "web-foundation.png");
const STATIC_ONLY = process.argv.includes("--static-only");
const require = createRequire(import.meta.url);

let failures = 0;
let browserRan = false;

function check(condition, message, detail = "") {
  if (condition) {
    console.log(`PASS  ${message}`);
  } else {
    failures += 1;
    console.error(`FAIL  ${message}${detail ? ` — ${detail}` : ""}`);
  }
}

function respondJson(request, body, status = 200) {
  return request.respond({
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "access-control-allow-origin": "*",
      "cache-control": "no-store",
    },
    body: JSON.stringify(body),
  });
}

const ONLINE_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";

const LIBERTY_FIXTURE = {
  version: 8,
  name: "OpenFreeMap Liberty (test)",
  sources: {
    land: {
      type: "geojson",
      data: { type: "FeatureCollection", features: [] },
    },
  },
  layers: [
    { id: "background", type: "background", paint: { "background-color": "#eef2f0" } },
  ],
};

const EMPTY_FILE_STATUS = {
  installed: false,
  bytes: 0,
  expected_bytes: 0,
  download_bytes: 0,
  downloaded_bytes: 0,
  progress_bytes: 0,
  phase: "idle",
  error: null,
};

/**
 * Second browser pass: mocked catalog/config/status proves the region manager,
 * the `{region}` download body, and the opt-in OpenFreeMap style switch. The
 * static server has no catalog, so the fallback pass checks the legacy panel.
 */
async function runMapsPanelChecks(browser) {
  const page = await browser.newPage();
  await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
  page.setDefaultTimeout(30000);
  await page.evaluateOnNewDocument(() => {
    try {
      localStorage.removeItem("pinpoint.onlineMap");
    } catch {
      // Storage disabled: the page still defaults to the offline style.
    }
  });

  const catalog = {
    schema: 1,
    updated: "2026-11-17T12:00:00Z",
    regions: [
      {
        id: "moldova",
        name: "Moldova",
        bbox: [7.4, 43.72, 7.45, 43.76],
        maxzoom: 14,
        version: "2026-11-17",
        installed: true,
        installed_version: "2026-11-17",
        files: [
          { role: "basemap", transfer_bytes: 93172399, installed_bytes: 93172399, installed: true },
        ],
      },
      {
        id: "romania",
        name: "Romania",
        bbox: [20.2, 43.6, 29.7, 48.3],
        maxzoom: 12,
        version: "2026-11-10",
        installed: false,
        files: [
          {
            role: "basemap",
            transfer_bytes: 550000000,
            installed_bytes: 600000000,
            installed: false,
          },
        ],
      },
    ],
  };
  const config = {
    pmtilesUrl: "/map/fixtures/monaco.pmtiles",
    graphInstalled: true,
    packInstalled: true,
    regions: [
      {
        id: "moldova",
        name: "Moldova",
        bbox: [7.4, 43.72, 7.45, 43.76],
        version: "2026-11-17",
        pmtilesUrl: "/map/fixtures/monaco.pmtiles",
        graphUrl: "/packs/moldova/moldova.graph.bin",
        installed: true,
      },
    ],
  };
  const idleStatus = {
    region: null,
    pack: { ...EMPTY_FILE_STATUS },
    graph: { ...EMPTY_FILE_STATUS },
    expected_download_bytes: 0,
  };
  const runningStatus = {
    region: "romania",
    pack: {
      ...EMPTY_FILE_STATUS,
      phase: "downloading",
      expected_bytes: 93172399,
      download_bytes: 93172399,
      downloaded_bytes: 12000000,
    },
    graph: {
      ...EMPTY_FILE_STATUS,
      expected_bytes: 110858815,
      download_bytes: 45338586,
    },
    expected_download_bytes: 138510985,
  };

  const state = {
    catalog,
    catalogStatus: 200,
    config,
    status: idleStatus,
    styleStatus: 200,
    posts: [],
    statusRequests: 0,
    styleRequests: 0,
  };

  const pageErrors = [];
  const consoleErrors = [];
  const externalRequests = [];
  const httpFailures = [];

  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("response", (response) => {
    if (response.status() < 400) return;
    try {
      httpFailures.push({ status: response.status(), url: new URL(response.url()).href });
    } catch {
      httpFailures.push({ status: response.status(), url: response.url() });
    }
  });
  page.on("request", (request) => {
    const url = request.url();
    if (url.startsWith("https://tiles.openfreemap.org/")) {
      state.styleRequests += 1;
      return;
    }
    if (!url.startsWith("http://") && !url.startsWith("https://")) return;
    if (new URL(url).origin !== base) externalRequests.push(url);
  });

  await page.setRequestInterception(true);
  page.on("request", (request) => {
    const url = request.url();
    if (url === ONLINE_STYLE_URL) {
      if (state.styleStatus !== 200) {
        return request.respond({
          status: state.styleStatus,
          contentType: "text/plain; charset=utf-8",
          headers: { "Access-Control-Allow-Origin": "*" },
          body: "unavailable",
        });
      }
      return respondJson(request, LIBERTY_FIXTURE);
    }

    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      return request.continue();
    }
    if (parsed.origin !== base) return request.continue();

    switch (parsed.pathname) {
      case "/map/catalog":
        return respondJson(request, state.catalog, state.catalogStatus);
      case "/map/config":
        return respondJson(request, state.config);
      case "/map/offline/status":
        state.statusRequests += 1;
        return respondJson(request, state.status);
      case "/map/offline/download":
        state.posts.push(request.postData());
        state.status = runningStatus;
        return respondJson(request, runningStatus, 202);
      default:
        return request.continue();
    }
  });

  await page.goto(`${base}/map/index.html`, { waitUntil: "load", timeout: 30000 });
  await page.waitForFunction(
    () =>
      window.__pinpointMapReady === true ||
      typeof window.__pinpointMapError === "string",
  );
  await page.waitForFunction(
    () => document.querySelectorAll("#region-list li").length === 2,
  );

  const rows = await page.$$eval("#region-list li", (items) =>
    items.map((item) => ({
      id: item.dataset.region,
      state: item.dataset.state,
      text: item.textContent.replace(/\s+/g, " ").trim(),
      buttons: [...item.querySelectorAll("button")].map((button) => ({
        action: button.dataset.action,
        label: button.getAttribute("aria-label"),
      })),
    })),
  );
  const moldova = rows.find((row) => row.id === "moldova");
  const romania = rows.find((row) => row.id === "romania");

  check(rows.length === 2, "catalog renders one row per region", JSON.stringify(rows));
  check(
    moldova?.text.includes("Moldova") && moldova?.text.includes("93 MB"),
    "Moldova row shows its transfer size",
    moldova?.text ?? "missing",
  );
  check(
    romania?.text.includes("Romania") && romania?.text.includes("550 MB"),
    "Romania row shows its transfer size",
    romania?.text ?? "missing",
  );
  check(
    moldova?.state === "installed" && moldova.buttons.some((b) => b.action === "show"),
    "installed region shows Installed and Show",
    JSON.stringify(moldova),
  );
  check(
    romania?.state === "idle" && romania.buttons.some((b) => b.action === "download"),
    "missing region shows Download",
    JSON.stringify(romania),
  );
  check(
    rows.every((row) => row.buttons.every((button) => Boolean(button.label))),
    "region buttons have accessible labels",
    JSON.stringify(rows),
  );

  const coverage = await page.evaluate(() => {
    const coverageAt = window.__pinpointMaps.coverageAt;
    const read = () => {
      const panel = document.querySelector("#maps-coverage");
      const button = panel?.querySelector("button");
      return {
        hidden: panel?.hidden ?? null,
        text: panel?.textContent.replace(/\s+/g, " ").trim() ?? "",
        action: button?.dataset.action ?? null,
        label: button?.getAttribute("aria-label") ?? null,
      };
    };
    coverageAt(24.9, 45.9);
    const romania = read();
    coverageAt(7.42, 43.74);
    const moldova = read();
    coverageAt(0, 0);
    const empty = read();
    coverageAt(-100, 40);
    const ocean = read();
    return { romania, moldova, empty, ocean };
  });

  check(
    coverage.romania.hidden === false &&
      coverage.romania.text.includes("Romania") &&
      coverage.romania.text.includes("550 MB") &&
      coverage.romania.action === "download" &&
      coverage.romania.label?.includes("Romania"),
    "coverage hint offers the uncovered region with its size",
    JSON.stringify(coverage.romania),
  );
  check(
    coverage.moldova.hidden === true,
    "coverage hint hides for the active source",
    JSON.stringify(coverage.moldova),
  );
  check(
    coverage.empty.hidden === true,
    "coverage hint hides when no region covers the centre",
    JSON.stringify(coverage.empty),
  );
  check(
    coverage.ocean.hidden === true,
    "coverage hint hides over open water",
    JSON.stringify(coverage.ocean),
  );

  const initial = await page.evaluate(() => ({
    sourceId: window.__pinpointPreview?.source?.id ?? null,
    meta: document.querySelector("#map-meta")?.textContent ?? "",
    mapsHidden: document.querySelector("#maps-panel")?.hidden ?? null,
    offlineHidden: document.querySelector("#offline-panel")?.hidden ?? null,
  }));
  check(
    initial.sourceId === "moldova",
    "defaults to the installed region from /map/config",
    JSON.stringify(initial),
  );
  check(initial.meta.includes("Moldova"), "map meta names the installed region", initial.meta);
  check(
    initial.mapsHidden === false && initial.offlineHidden === true,
    "catalog mode shows the Maps panel and hides the legacy card",
    JSON.stringify(initial),
  );

  await page.click('#region-list li[data-region="romania"] [data-action="download"]');
  await page.waitForFunction(() => !document.querySelector("#maps-progress")?.hidden);

  const post = state.posts.length ? JSON.parse(state.posts[0]) : null;
  check(
    post?.region === "romania",
    "download POST body is { region }",
    state.posts[0] ?? "missing",
  );

  const progress = await page.evaluate(() => ({
    text: document.querySelector("#maps-progress-text")?.textContent ?? "",
    live: document.querySelector("#maps-progress")?.getAttribute("aria-live") ?? "",
    value: Number(document.querySelector("#maps-progress-bar")?.value ?? 0),
    max: Number(document.querySelector("#maps-progress-bar")?.max ?? 0),
  }));
  check(
    progress.text.includes("Downloading") &&
      progress.text.includes("Romania") &&
      progress.text.includes("MB"),
    "progress row names the phase, region and bytes",
    progress.text,
  );
  check(progress.live === "polite", "progress row is an aria-live region", progress.live);
  check(
    progress.max === 138510985 && progress.value === 12000000,
    "progress bar uses the status transfer total",
    JSON.stringify(progress),
  );

  await new Promise((resolve) => setTimeout(resolve, 1200));
  check(
    state.statusRequests >= 2,
    "status polls while the download runs",
    String(state.statusRequests),
  );

  state.status = { ...idleStatus, region: "romania" };
  await new Promise((resolve) => setTimeout(resolve, 800));
  const stoppedAt = state.statusRequests;
  await new Promise((resolve) => setTimeout(resolve, 700));
  check(
    state.statusRequests === stoppedAt,
    "status polling stops when no download runs",
    `${stoppedAt} -> ${state.statusRequests}`,
  );
  check(
    await page.$eval("#maps-progress", (element) => element.hidden),
    "progress row hides when the download is idle",
  );

  await page.evaluate(() =>
    window.__pinpointPreview?.handle?.drawRoute({
      points: [
        [47.0105, 28.8638],
        [47.76, 27.93],
      ],
    }),
  );
  await page.waitForFunction(() =>
    Boolean(window.__pinpointPreview?.map?.getLayer("pinpoint-route-line")),
  );

  await page.click("#online-map");
  await page.waitForFunction(
    () =>
      window.__pinpointPreview?.online === true &&
      window.__pinpointPreview?.map?.getStyle()?.name === "OpenFreeMap Liberty (test)",
  );

  const onlineState = await page.evaluate(() => ({
    online: window.__pinpointPreview?.online ?? null,
    styleName: window.__pinpointPreview?.map?.getStyle()?.name ?? "",
    creditHidden: document.querySelector("#online-credit")?.hidden ?? null,
    credit: document.querySelector("#online-credit")?.textContent.replace(/\s+/g, " ").trim() ?? "",
    privacy: document.querySelector(".online-credit__privacy")?.textContent?.trim() ?? "",
    markers: Boolean(window.__pinpointPreview?.map?.getLayer("pinpoint-locations-circles")),
    route: Boolean(
      window.__pinpointPreview?.map?.getLayer("pinpoint-route-line") &&
        window.__pinpointPreview?.map?.getLayer("pinpoint-route-casing"),
    ),
    stored: localStorage.getItem("pinpoint.onlineMap"),
    toggle: document.querySelector("#online-map")?.checked ?? null,
  }));
  check(
    onlineState.online === true && onlineState.styleName === "OpenFreeMap Liberty (test)",
    "online toggle switches MapLibre to the fetched style",
    JSON.stringify(onlineState),
  );
  check(
    onlineState.creditHidden === false &&
      onlineState.credit.includes("tiles by OpenFreeMap"),
    "OpenFreeMap credit is visible only while online",
    onlineState.credit,
  );
  check(
    onlineState.privacy.includes("OpenFreeMap") &&
      onlineState.privacy.includes("reveal the viewed area"),
    "online privacy note is shown",
    onlineState.privacy,
  );
  check(
    onlineState.markers && onlineState.route,
    "pins, route casing and route line survive style.load",
    JSON.stringify({ markers: onlineState.markers, route: onlineState.route }),
  );
  check(
    onlineState.stored === "1" && onlineState.toggle === true,
    "online choice persists in pinpoint.onlineMap",
    JSON.stringify({ stored: onlineState.stored, toggle: onlineState.toggle }),
  );
  check(state.styleRequests === 1, "online style fetched once", String(state.styleRequests));

  await page.click("#online-map");
  await page.waitForFunction(
    () =>
      window.__pinpointPreview?.online === false &&
      window.__pinpointPreview?.map?.getStyle()?.name === "Pinpoint Offline Basemap",
  );
  const offlineState = await page.evaluate(() => ({
    stored: localStorage.getItem("pinpoint.onlineMap"),
    creditHidden: document.querySelector("#online-credit")?.hidden ?? null,
    markers: Boolean(window.__pinpointPreview?.map?.getLayer("pinpoint-locations-circles")),
    route: Boolean(window.__pinpointPreview?.map?.getLayer("pinpoint-route-line")),
  }));
  check(
    offlineState.stored === "0" && offlineState.creditHidden === true,
    "disabling online restores the local style and hides the credit",
    JSON.stringify(offlineState),
  );
  check(
    offlineState.markers && offlineState.route,
    "pins and route survive the offline style restore",
    JSON.stringify(offlineState),
  );

  // A failed style fetch keeps the local map and reports inline, with no retry.
  state.styleStatus = 500;
  const styleRequestsBefore = state.styleRequests;
  await page.click("#online-map");
  await page.waitForFunction(
    () => document.querySelector("#online-error")?.hidden === false,
  );
  await new Promise((resolve) => setTimeout(resolve, 700));
  const failedState = await page.evaluate(() => ({
    online: window.__pinpointPreview?.online ?? null,
    styleName: window.__pinpointPreview?.map?.getStyle()?.name ?? "",
    errorHidden: document.querySelector("#online-error")?.hidden ?? null,
    error: document.querySelector("#online-error")?.textContent ?? "",
    toggle: document.querySelector("#online-map")?.checked ?? null,
  }));
  check(
    failedState.online === false &&
      failedState.styleName === "Pinpoint Offline Basemap" &&
      failedState.toggle === false,
    "a failed online fetch keeps the local style rendered",
    JSON.stringify(failedState),
  );
  check(
    failedState.errorHidden === false &&
      failedState.error.includes("Online map unavailable"),
    "a failed online fetch shows the inline message",
    failedState.error,
  );
  check(
    state.styleRequests === styleRequestsBefore + 1,
    "a failed online fetch is not retried automatically",
    `${styleRequestsBefore} -> ${state.styleRequests}`,
  );

  check(pageErrors.length === 0, "maps panel has no page errors", pageErrors.join(" | "));
  const unexpectedHttpFailures = httpFailures.filter(
    ({ status, url }) => !(status === 500 && url === ONLINE_STYLE_URL),
  );
  check(
    unexpectedHttpFailures.length === 0,
    "only the deliberate online-style 500 failed",
    JSON.stringify(httpFailures),
  );
  const unexpectedConsoleErrors = consoleErrors.filter((text) => {
    if (!text.includes("status of 500")) return true;
    return unexpectedHttpFailures.length > 0;
  });
  check(
    unexpectedConsoleErrors.length === 0,
    "maps panel has no console errors other than the deliberate style failure",
    unexpectedConsoleErrors.join(" | "),
  );
  check(
    externalRequests.length === 0,
    "maps panel sends no requests except the opted-in style",
    externalRequests.join(", "),
  );

  await page.close();

  // Fallback: a v2.3.0 backend has no /map/catalog and must keep the old card.
  const legacyPage = await browser.newPage();
  legacyPage.setDefaultTimeout(30000);
  await legacyPage.evaluateOnNewDocument(() => {
    try {
      localStorage.removeItem("pinpoint.onlineMap");
    } catch {
      // Storage disabled: the page still defaults to the offline style.
    }
  });

  const legacyStatus = {
    pack: {
      ...EMPTY_FILE_STATUS,
      expected_bytes: 93172399,
      download_bytes: 93172399,
    },
    graph: {
      ...EMPTY_FILE_STATUS,
      expected_bytes: 110858815,
      download_bytes: 45338586,
    },
    expected_download_bytes: 138510985,
  };
  const legacyConsoleErrors = [];
  const legacyHttpFailures = [];

  legacyPage.on("console", (message) => {
    if (message.type() === "error") legacyConsoleErrors.push(message.text());
  });
  legacyPage.on("response", (response) => {
    if (response.status() < 400) return;
    try {
      legacyHttpFailures.push({
        status: response.status(),
        pathname: new URL(response.url()).pathname,
      });
    } catch {
      legacyHttpFailures.push({ status: response.status(), pathname: response.url() });
    }
  });
  await legacyPage.setRequestInterception(true);
  legacyPage.on("request", (request) => {
    let parsed;
    try {
      parsed = new URL(request.url());
    } catch {
      return request.continue();
    }
    if (parsed.origin !== base) return request.continue();
    switch (parsed.pathname) {
      case "/map/catalog":
        return request.respond({ status: 404, body: "Not Found" });
      case "/map/config":
        return respondJson(request, {
          pmtilesUrl: "./fixtures/monaco.pmtiles",
          graphInstalled: false,
          packInstalled: false,
        });
      case "/map/offline/status":
        return respondJson(request, legacyStatus);
      default:
        return request.continue();
    }
  });

  await legacyPage.goto(`${base}/map/index.html`, { waitUntil: "load", timeout: 30000 });
  await legacyPage.waitForFunction(
    () =>
      window.__pinpointMapReady === true &&
      document.querySelector("#offline-panel")?.hidden === false,
  );
  const fallback = await legacyPage.evaluate(() => ({
    mapsHidden: document.querySelector("#maps-panel")?.hidden ?? null,
    offlineHidden: document.querySelector("#offline-panel")?.hidden ?? null,
    badge: document.querySelector("#offline-badge")?.textContent ?? "",
    buttonHidden: document.querySelector("#offline-download")?.hidden ?? null,
    buttonText: document.querySelector("#offline-download")?.textContent ?? "",
  }));
  check(
    fallback.mapsHidden === true && fallback.offlineHidden === false,
    "catalog 404 falls back to the legacy offline card",
    JSON.stringify(fallback),
  );
  check(
    fallback.badge === "Not installed" &&
      fallback.buttonHidden === false &&
      fallback.buttonText.includes("Download Moldova map") &&
      fallback.buttonText.includes("139 MB"),
    "fallback keeps the Moldova download behavior and computed size",
    JSON.stringify(fallback),
  );
  const unexpectedLegacyFailures = legacyHttpFailures.filter(
    ({ status, pathname }) => !(status === 404 && pathname === "/map/catalog"),
  );
  check(
    unexpectedLegacyFailures.length === 0,
    "only the deliberate catalog 404 failed in fallback mode",
    JSON.stringify(legacyHttpFailures),
  );
  const unexpectedLegacyErrors = legacyConsoleErrors.filter((text) => {
    if (!text.includes("status of 404")) return true;
    return unexpectedLegacyFailures.length > 0;
  });
  check(
    unexpectedLegacyErrors.length === 0,
    "fallback has no console errors other than the deliberate catalog 404",
    unexpectedLegacyErrors.join(" | "),
  );
  await legacyPage.close();
}

async function fetchText(base, pathname) {
  const response = await fetch(base + pathname);
  return { response, text: await response.text() };
}

const server = createStaticServer({ root: WEB_ROOT });
const pmtilesResponses = [];

server.on("request", (request, response) => {
  if ((request.url ?? "").includes("monaco.pmtiles")) {
    response.on("finish", () => {
      pmtilesResponses.push({
        status: response.statusCode,
        range: request.headers.range ?? null,
      });
    });
  }
});

await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});

const address = server.address();
if (!address || typeof address === "string") {
  console.error("FAIL  could not determine the test server port");
  process.exit(1);
}
const base = `http://127.0.0.1:${address.port}`;
console.log(`Serving ${WEB_ROOT} at ${base}`);

try {
  // -------------------------------------------------------------------------
  // Static assets
  // -------------------------------------------------------------------------
  const assets = [
    ["/map/index.html", "text/html"],
    ["/map/style.json", "application/json"],
    ["/map/map.js", "text/javascript"],
    ["/map/preview.css", "text/css"],
    ["/map/fixtures/monaco.pmtiles", "application/octet-stream"],
    ["/map/fixtures/README.md", "text/plain"],
    ["/map/README.md", "text/plain"],
    ["/vendor/maplibre-gl.js", "text/javascript"],
    ["/vendor/maplibre-gl-worker.mjs", "text/javascript"],
    ["/vendor/maplibre-gl.css", "text/css"],
    ["/vendor/pmtiles.js", "text/javascript"],
    ["/vendor/maplibre-gl.LICENSE.txt", "text/plain"],
    ["/vendor/pmtiles.LICENSE.txt", "text/plain"],
  ];

  for (const [pathname, expectedType] of assets) {
    const response = await fetch(base + pathname);
    const type = response.headers.get("content-type") ?? "";
    check(response.status === 200, `HTTP 200 for ${pathname}`, `got ${response.status}`);
    check(type.startsWith(expectedType), `content type for ${pathname}`, `got ${type}`);
    await response.arrayBuffer();
  }

  const rangeResponse = await fetch(base + "/map/fixtures/monaco.pmtiles", {
    headers: { Range: "bytes=0-126" },
  });
  const rangeBody = Buffer.from(await rangeResponse.arrayBuffer());
  check(rangeResponse.status === 206, "Range request returns HTTP 206", `got ${rangeResponse.status}`);
  check(
    rangeResponse.headers.get("content-range") === `bytes 0-126/${FIXTURE_BYTES}`,
    "Range response advertises the correct Content-Range",
    rangeResponse.headers.get("content-range") ?? "missing",
  );
  check(
    rangeBody.length === 127 && rangeBody.subarray(0, 7).toString("latin1") === "PMTiles",
    "Range response starts with the PMTiles magic bytes",
  );

  // -------------------------------------------------------------------------
  // Fixture and style contract
  // -------------------------------------------------------------------------
  const fixture = readFileSync(FIXTURE);
  const fixtureHash = createHash("sha256").update(fixture).digest("hex");
  check(fixture.length === FIXTURE_BYTES, `fixture is ${FIXTURE_BYTES} bytes`, `got ${fixture.length}`);
  check(fixtureHash === FIXTURE_SHA256, "fixture SHA-256 matches the M1 manifest", fixtureHash);
  check(fixture.subarray(0, 7).toString("latin1") === "PMTiles", "fixture is a PMTiles archive");

  const { response: styleResponse, text: styleText } = await fetchText(base, "/map/style.json");
  check(styleResponse.status === 200, "style.json is served");
  const style = JSON.parse(styleText);
  check(style.version === 8, "style uses the v8 spec");
  check(!("glyphs" in style), "style does not reference remote glyphs");
  check(!("sprite" in style), "style does not reference remote sprites");
  check(style.sources?.basemap?.type === "vector", "style declares a vector basemap source");
  check(
    String(style.sources?.basemap?.url ?? "").startsWith("pmtiles://"),
    "basemap source uses the pmtiles:// protocol",
  );
  const metadataAttribution = String(style.metadata?.["pinpoint:attribution"] ?? "");
  const sourceAttribution = String(style.sources?.basemap?.attribution ?? "");
  const styleAttribution = `${metadataAttribution}\n${sourceAttribution}`;

  check(
    metadataAttribution.includes("© OpenStreetMap contributors"),
    "style metadata uses © OpenStreetMap contributors",
    metadataAttribution,
  );
  check(
    sourceAttribution.includes("© OpenStreetMap contributors"),
    "basemap source attribution uses © OpenStreetMap contributors",
    sourceAttribution,
  );
  check(
    styleAttribution.includes("OpenMapTiles") && styleAttribution.includes("ODbL 1.0"),
    "style attribution names OpenMapTiles and ODbL 1.0",
    styleAttribution,
  );
  for (const mojibake of [BROKEN_AC_ATTRIBUTION, "A\uFFFD", "\u00C2\u00A9", "\uFFFD"]) {
    check(
      !styleText.includes(mojibake),
      `style.json has no mojibake ${JSON.stringify(mojibake)}`,
    );
  }
  check(
    JSON.stringify(style.metadata ?? {}).includes("ODbL-1.0"),
    "style metadata records ODbL-1.0",
  );

  const sourceLayers = new Set(style.layers.map((layer) => layer["source-layer"]).filter(Boolean));
  for (const required of ["water", "landuse", "transportation", "boundary"]) {
    check(sourceLayers.has(required), `style renders the "${required}" source layer`);
  }
  check(style.layers.some((layer) => layer.id === "background"), "style renders a background");

  const { text: indexHtml } = await fetchText(base, "/map/index.html");
  check(indexHtml.includes("../vendor/maplibre-gl.css"), "preview loads local MapLibre CSS");
  check(indexHtml.includes("../vendor/pmtiles.js"), "preview loads local PMTiles JS");
  check(
    indexHtml.includes("©") &&
      indexHtml.includes("OpenStreetMap contributors") &&
      indexHtml.includes("ODbL"),
    "preview HTML shows visible © OpenStreetMap contributors / ODbL attribution",
  );
  check(
    !indexHtml.includes(BROKEN_AC_ATTRIBUTION) &&
      !indexHtml.includes("\uFFFD") &&
      !indexHtml.includes("\u00C2\u00A9"),
    "preview HTML has no attribution mojibake",
  );
  for (const forbidden of ["unpkg.com", "jsdelivr", "cdn.", "tile.openstreetmap.org"]) {
    check(!indexHtml.toLowerCase().includes(forbidden), `preview does not reference ${forbidden}`);
  }

  check(indexHtml.includes('id="online-map"'), "preview offers the opt-in online toggle");
  check(
    indexHtml.includes('id="maps-panel"') && indexHtml.includes('aria-live="polite"'),
    "Maps panel exposes live download progress",
  );
  check(
    indexHtml.includes("pinpoint.onlineMap") &&
      indexHtml.includes("https://tiles.openfreemap.org/styles/liberty"),
    "online style URL and localStorage key are page-owned",
  );
  check(
    !indexHtml.includes("setCacheMode") && !indexHtml.includes("LOAD_NO_CACHE"),
    "the page has no LOAD_NO_CACHE assumption",
  );
  const mapModuleText = readFileSync(path.join(WEB_ROOT, "map", "map.js"), "utf8");
  check(
    !mapModuleText.includes("https://"),
    "map.js has no remote URL (tests/assets.rs contract)",
  );

  const utf8Files = [
    "map/style.json",
    "map/index.html",
    "map/README.md",
    "map/fixtures/README.md",
  ];
  for (const relative of utf8Files) {
    const bytes = readFileSync(path.join(WEB_ROOT, relative));
    check(
      !(bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf),
      `${relative} is UTF-8 without BOM`,
    );

    let decoded = "";
    let validUtf8 = true;
    try {
      decoded = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch {
      validUtf8 = false;
    }
    check(validUtf8, `${relative} is valid UTF-8`);
    if (validUtf8) {
      check(
        !decoded.includes(BROKEN_AC_ATTRIBUTION) &&
          !decoded.includes("\uFFFD") &&
          !decoded.includes("\u00C2\u00A9"),
        `${relative} has no attribution mojibake`,
      );
    }
  }

  // -------------------------------------------------------------------------
  // Browser render
  // -------------------------------------------------------------------------
  let puppeteer = null;
  if (!STATIC_ONLY) {
    try {
      puppeteer = require("puppeteer");
    } catch {
      console.log("SKIP  browser: puppeteer is not installed (set NODE_PATH or run --static-only)");
      console.log("SKIP  static checks only; WebGL rendering was not exercised");
    }
  } else {
    console.log("SKIP  browser: --static-only requested");
  }

  if (puppeteer) {
    browserRan = true;
    mkdirSync(path.dirname(SCREENSHOT), { recursive: true });

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
      await page.setViewport({ width: 1440, height: 900, deviceScaleFactor: 1 });
      page.setDefaultTimeout(45000);

      const pageErrors = [];
      const consoleErrors = [];
      const failedRequests = [];
      const externalRequests = [];
      const notFound = [];

      page.on("pageerror", (error) => pageErrors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") consoleErrors.push(message.text());
      });
      page.on("response", (response) => {
        if (response.status() !== 404) return;
        try {
          notFound.push(new URL(response.url()).pathname);
        } catch {
          notFound.push(response.url());
        }
      });
      page.on("requestfailed", (request) => {
        failedRequests.push(`${request.url()} (${request.failure()?.errorText ?? "unknown"})`);
      });
      page.on("request", (request) => {
        const url = request.url();
        if (!url.startsWith("http://") && !url.startsWith("https://")) return;
        if (new URL(url).origin !== base) externalRequests.push(url);
      });

      await page.goto(`${base}/map/index.html`, {
        waitUntil: "load",
        timeout: 45000,
      });

      await page.waitForFunction(
        () =>
          window.__pinpointMapReady === true ||
          typeof window.__pinpointMapError === "string",
        { timeout: 45000 },
      );

      // The marker layer is added inside the load handler, so its first paint
      // lands one frame later. Wait for the features instead of racing it.
      await page
        .waitForFunction(
          () => {
            const map = window.__pinpointPreview?.map;
            try {
              return (
                map?.queryRenderedFeatures({
                  layers: ["pinpoint-locations-circles"],
                }).length >= 3
              );
            } catch {
              return false;
            }
          },
          { timeout: 15000 },
        )
        .catch(() => {});

      const result = await page.evaluate(() => {
        const map = window.__pinpointPreview?.map;
        const canvas = document.querySelector(".maplibregl-canvas");
        let renderedMarkers = -1;

        try {
          renderedMarkers = map?.queryRenderedFeatures({
            layers: ["pinpoint-locations-circles"],
          }).length ?? -1;
        } catch {
          renderedMarkers = -1;
        }

        return {
          ready: window.__pinpointMapReady === true,
          error: window.__pinpointMapError ?? null,
          canvas: canvas
            ? {
                width: canvas.width,
                height: canvas.height,
                clientWidth: canvas.clientWidth,
                clientHeight: canvas.clientHeight,
              }
            : null,
          layer: Boolean(map?.getLayer("pinpoint-locations-circles")),
          sourceUrl: map?.getStyle()?.sources?.basemap?.url ?? null,
          renderedMarkers,
          status: document.querySelector("#status")?.dataset.state ?? null,
          attribution:
            document.querySelector(".map-attribution")?.textContent.replace(/\s+/g, " ").trim() ??
            "",
        };
      });

      check(result.error === null, "no map initialization error", result.error ?? "");
      check(result.ready === true, "MapLibre load event fired");
      check(result.status === "ready", "preview status is ready", result.status ?? "missing");
      check(Boolean(result.canvas), "MapLibre canvas exists");
      check(
        Boolean(result.canvas && result.canvas.width > 0 && result.canvas.height > 0),
        "MapLibre canvas has a non-zero backing size",
        JSON.stringify(result.canvas),
      );
      check(result.layer === true, "friend marker layer was added");
      check(result.renderedMarkers >= 3, "three sample markers are rendered", String(result.renderedMarkers));
      check(
        String(result.sourceUrl ?? "").includes("pmtiles://") &&
          String(result.sourceUrl ?? "").includes("monaco.pmtiles"),
        "basemap source is bound to the local PMTiles fixture",
        result.sourceUrl ?? "missing",
      );
      check(
        result.attribution.includes("© OpenStreetMap contributors"),
        "visible attribution contains © OpenStreetMap contributors",
        result.attribution,
      );
      check(
        !result.attribution.includes(BROKEN_AC_ATTRIBUTION) &&
          !result.attribution.includes("\uFFFD") &&
          !result.attribution.includes("\u00C2\u00A9"),
        "visible attribution has no mojibake",
        result.attribution,
      );

      await page.screenshot({ path: SCREENSHOT });
      const screenshotOk = existsSync(SCREENSHOT) && statSync(SCREENSHOT).size > 10000;
      check(screenshotOk, `screenshot saved (${SCREENSHOT})`, "file missing or too small");

      const themeBefore = await page.evaluate(() => ({
        page: document.documentElement.dataset.theme,
        map: document.querySelector("#map")?.dataset.theme,
        darkClass: document.querySelector("#map")?.classList.contains("pinpoint-map--dark"),
        background: window.__pinpointPreview?.map?.getPaintProperty("background", "background-color"),
      }));

      await page.click("#theme");
      await page.waitForFunction(
        (previous) => {
          const container = document.querySelector("#map");
          const background = window.__pinpointPreview?.map?.getPaintProperty(
            "background",
            "background-color",
          );
          return (
            document.documentElement.dataset.theme !== previous.page &&
            container?.dataset.theme !== previous.map &&
            background !== previous.background
          );
        },
        { timeout: 5000 },
        themeBefore,
      );

      const themeAfter = await page.evaluate(() => ({
        page: document.documentElement.dataset.theme,
        map: document.querySelector("#map")?.dataset.theme,
        darkClass: document.querySelector("#map")?.classList.contains("pinpoint-map--dark"),
        background: window.__pinpointPreview?.map?.getPaintProperty("background", "background-color"),
      }));
      check(
        themeAfter.page !== themeBefore.page &&
          themeAfter.darkClass === (themeAfter.page === "dark") &&
          themeAfter.background !== themeBefore.background,
        "setTheme switches the page class and MapLibre background",
        JSON.stringify({ themeBefore, themeAfter }),
      );

      // Restore the initial theme before exercising the click callback.
      await page.click("#theme");
      await page.waitForFunction(
        (previous) => document.documentElement.dataset.theme === previous.page,
        { timeout: 5000 },
        themeBefore,
      );

      const clickPoint = await page.$eval("#map", (element) => {
        const rect = element.getBoundingClientRect();
        return { x: rect.left + rect.width * 0.72, y: rect.top + rect.height * 0.55 };
      });
      await page.mouse.click(clickPoint.x, clickPoint.y);
      await page.waitForFunction(
        () => {
          const text = document.querySelector("#click-readout")?.textContent?.trim();
          return Boolean(text && text !== "—");
        },
        { timeout: 5000 },
      );
      const clickText = await page.$eval("#click-readout", (element) => element.textContent.trim());
      check(
        /^-?\d+\.\d{4}, -?\d+\.\d{4}$/.test(clickText),
        "onClick reports { lat, lon }",
        clickText,
      );

      const pmtilesRangeRequests = pmtilesResponses.filter((entry) => entry.status === 206);
      check(
        pmtilesRangeRequests.length > 0,
        "browser fetched PMTiles with Range requests",
        JSON.stringify(pmtilesResponses),
      );
      check(externalRequests.length === 0, "no external network requests", externalRequests.join(", "));
      check(failedRequests.length === 0, "no failed network requests", failedRequests.join(", "));
      check(pageErrors.length === 0, "no page errors", pageErrors.join(" | "));

      const unexpectedNotFound = notFound.filter((pathname) => pathname !== "/map/catalog");
      check(
        unexpectedNotFound.length === 0,
        "only the expected catalog 404 is missing",
        JSON.stringify(notFound),
      );
      const unexpectedConsoleErrors = consoleErrors.filter((text) => {
        if (!text.includes("status of 404")) return true;
        return unexpectedNotFound.length > 0;
      });
      check(
        unexpectedConsoleErrors.length === 0,
        "no console errors besides the expected catalog 404",
        unexpectedConsoleErrors.join(" | "),
      );

      await page.close();
      await runMapsPanelChecks(browser);
    } finally {
      await browser.close();
    }
  }
} finally {
  await new Promise((resolve) => server.close(resolve));
}

console.log("");
if (failures === 0) {
  console.log(
    browserRan
      ? `Map foundation checks passed. Screenshot: ${SCREENSHOT}`
      : "Map foundation checks passed (static only).",
  );
  process.exitCode = 0;
} else {
  console.error(`Map foundation checks failed: ${failures}`);
  process.exitCode = 1;
}

/**
 * Opt-in online basemap test for `web/app/map.js`.
 *
 * Static checks always run. With Puppeteer available the test stages the real
 * adapter against a mocked MapLibre/PMTiles foundation and a mocked MapLibre
 * module, then drives the toggle in headless Chromium. No request leaves the
 * test server and no real OpenFreeMap tile is fetched.
 *
 * Usage (PowerShell):
 *   $env:NODE_PATH="C:\path\to\node_modules"
 *   node web/app/test/online-map.mjs
 *
 * Static-only fallback:
 *   node web/app/test/online-map.mjs --static-only
 */

import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createStaticServer } from "../../map/serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(HERE, "../..");
const STATIC_ONLY = process.argv.includes("--static-only");
const require = createRequire(import.meta.url);

const ONLINE_URL = "https://tiles.openfreemap.org/styles/liberty";
const LOCAL_STYLE = "/map-assets/style.json";
const LOCAL_PMTILES = "/map-assets/fixtures/monaco.pmtiles";
const STORAGE_KEY = "pinpoint.onlineMapWeb";

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

// ---------------------------------------------------------------------------
// Static checks
// ---------------------------------------------------------------------------
const adapter = readFileSync(path.join(WEB_ROOT, "app", "map.js"), "utf8");
const indexHtml = readFileSync(path.join(WEB_ROOT, "app", "index.html"), "utf8");
const appCss = readFileSync(path.join(WEB_ROOT, "app", "app.css"), "utf8");

check(
  indexHtml.includes('name="pinpoint-online-style"') &&
    indexHtml.includes(ONLINE_URL),
  "index.html page-owns the OpenFreeMap Liberty style URL",
);
check(
  !adapter.includes("https://") && !adapter.includes("http://"),
  "adapter stays free of absolute origins (asset guard)",
);
check(
  adapter.includes(STORAGE_KEY),
  "adapter persists the online preference",
);
check(
  adapter.includes("setOnline") && adapter.includes("base.setStyle"),
  "adapter exposes setOnline and switches through the foundation",
);
check(
  /base\.setStyle\(onlineStyleUrl\)/.test(adapter),
  "enabling online calls setStyle with the page-owned style URL",
);
check(
  /base\.setStyle\(localStyleUrl, localPmtilesUrl\)/.test(adapter),
  "disabling online restores the local style and PMTiles URL",
);
check(
  adapter.includes("ONLINE_TIMEOUT_MS = 10000") ||
    adapter.includes("ONLINE_TIMEOUT_MS = 10_000"),
  "online style switch has a 10 second timeout",
);
check(
  !adapter.includes("fetch(") &&
    !adapter.includes("Authorization") &&
    !adapter.includes("credentials"),
  "adapter sends no fetch, auth header or credentials to the online host",
);
check(
  indexHtml.includes('id="online-map-toggle"') &&
    indexHtml.includes("OpenFreeMap") &&
    /viewed area|area you view/i.test(indexHtml),
  "map settings show an Online map toggle and privacy note",
);
check(
  indexHtml.includes('id="map-attribution-online"') &&
    indexHtml.includes("tiles by OpenFreeMap"),
  "index.html carries the online attribution",
);
check(
  indexHtml.includes("OpenMapTiles") && indexHtml.includes("ODbL 1.0"),
  "index.html keeps the offline OpenMapTiles/ODbL credit",
);
check(
  appCss.includes(".online-map-toggle") &&
    appCss.includes(".online-map-error"),
  "app.css styles the toggle and inline error",
);

// ---------------------------------------------------------------------------
// Stage the adapter with mocked foundation + MapLibre modules.
// ---------------------------------------------------------------------------
const MOCK_FOUNDATION = `
const STYLE_CALLS = [];
globalThis.__styleCalls = STYLE_CALLS;
globalThis.__failOnline = false;

function createMockMap() {
  const sources = new Map();
  const layers = new Map();
  const controls = [];
  return {
    addControl(control) {
      controls.push(control);
    },
    on() {
      return this;
    },
    off() {
      return this;
    },
    once() {
      return this;
    },
    getSource(id) {
      return sources.get(id) ?? null;
    },
    addSource(id, source) {
      sources.set(id, source);
    },
    removeSource(id) {
      sources.delete(id);
    },
    getLayer(id) {
      return layers.get(id) ?? null;
    },
    addLayer(layer) {
      layers.set(layer.id, layer);
    },
    removeLayer(id) {
      layers.delete(id);
    },
    remove() {},
    resize() {},
    getStyle() {
      return { sources: {} };
    },
    getCenter() {
      return { lat: 0, lng: 0 };
    },
    getZoom() {
      return 2;
    },
    setStyle(style, pmtilesUrl) {
      STYLE_CALLS.push({ style: String(style), pmtilesUrl: pmtilesUrl ?? null });
      if (String(style).includes("openfreemap") && globalThis.__failOnline) {
        return Promise.reject(new Error("simulated online style failure"));
      }
      return Promise.resolve();
    },
  };
}

export function createMap(options) {
  const map = createMockMap();
  globalThis.__baseOptions = {
    styleUrl: String(options.styleUrl),
    pmtilesUrl: String(options.pmtilesUrl),
  };
  const handle = {
    setLocations(locations) {
      return Array.isArray(locations) ? locations.length : 0;
    },
    onClick() {
      return () => {};
    },
    setTheme() {},
    setStyle(style, pmtilesUrl) {
      return map.setStyle(style, pmtilesUrl);
    },
    drawRoute() {
      return true;
    },
    clearRoute() {},
    destroy() {},
    resize() {},
    getMap() {
      return map;
    },
  };
  queueMicrotask(() => options.onLoad?.(handle));
  return handle;
}
`;

const MOCK_MAPLIBRE = `
export class NavigationControl {
  constructor() {}
}

export class ScaleControl {
  constructor() {}
}

export class Marker {
  constructor() {}
  setLngLat() {
    return this;
  }
  addTo() {
    return this;
  }
  remove() {}
}
`;

const TEST_PAGE = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="pinpoint-online-style" content="${ONLINE_URL}" />
    <title>Pinpoint · online map adapter test</title>
    <link rel="icon" href="data:," />
    <link rel="stylesheet" href="/app/app.css" />
    <style>
      html, body { height: 100%; margin: 0; }
      #map { width: 480px; height: 320px; }
      #test-controls { padding: 8px; }
    </style>
  </head>
  <body>
    <div id="map" aria-label="Online map adapter test"></div>
    <div id="test-controls">
      <label class="online-map-toggle">
        <input type="checkbox" id="online-map-toggle" />
        Online map
      </label>
      <p class="muted online-map-note">
        Tiles are fetched from OpenFreeMap and reveal the viewed area.
      </p>
      <p id="online-map-error" class="online-map-error hidden" role="alert"></p>
    </div>
    <div class="map-attribution">
      <span id="map-attribution-offline">Map data © OpenStreetMap contributors · OpenMapTiles · ODbL 1.0</span>
      <span id="map-attribution-online" class="hidden">© OpenStreetMap contributors · tiles by OpenFreeMap</span>
    </div>
    <script type="module" src="/app/online-map-driver.js"></script>
  </body>
</html>
`;

const TEST_DRIVER = `
import { createMap } from "/app/map.js";

function build() {
  const api = createMap({
    element: document.getElementById("map"),
    center: [43.7384, 7.4246],
    zoom: 13.2,
    theme: "dark",
    onLoad() {
      window.__ready = true;
    },
    onError(error) {
      window.__mapError = error?.message || String(error);
    },
  });
  window.__api = api;
}

window.__createMap = () => {
  window.__ready = false;
  build();
};

window.__api = null;
build();
`;

const STAGE = mkdtempSync(path.join(tmpdir(), "pinpoint-online-map-"));
mkdirSync(path.join(STAGE, "app"), { recursive: true });
mkdirSync(path.join(STAGE, "map-assets"), { recursive: true });
mkdirSync(path.join(STAGE, "vendor"), { recursive: true });

copyFileSync(path.join(WEB_ROOT, "app", "map.js"), path.join(STAGE, "app", "map.js"));
copyFileSync(path.join(WEB_ROOT, "app", "app.css"), path.join(STAGE, "app", "app.css"));
writeFileSync(path.join(STAGE, "map-assets", "map.js"), MOCK_FOUNDATION);
writeFileSync(path.join(STAGE, "vendor", "maplibre-gl.js"), MOCK_MAPLIBRE);
writeFileSync(path.join(STAGE, "app", "online-map.html"), TEST_PAGE);
writeFileSync(path.join(STAGE, "app", "online-map-driver.js"), TEST_DRIVER);

const server = createStaticServer({ root: STAGE });
await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});

const address = server.address();
if (!address || typeof address === "string") {
  console.error("FAIL  could not determine the test server port");
  rmSync(STAGE, { recursive: true, force: true });
  process.exit(1);
}
const base = `http://127.0.0.1:${address.port}`;
console.log(`Serving staged online-map assets at ${base}`);

try {
  let puppeteer = null;
  if (!STATIC_ONLY) {
    try {
      puppeteer = require("puppeteer");
    } catch {
      console.log(
        "SKIP  browser: puppeteer is not installed (set NODE_PATH or run --static-only)",
      );
    }
  } else {
    console.log("SKIP  browser: --static-only requested");
  }

  if (puppeteer) {
    browserRan = true;
    const browser = await puppeteer.launch({
      headless: true,
      args: [
        "--no-sandbox",
        "--disable-setuid-sandbox",
        "--disable-dev-shm-usage",
        "--enable-unsafe-swiftshader",
      ],
    });

    try {
      const page = await browser.newPage();
      await page.setViewport({ width: 900, height: 700, deviceScaleFactor: 1 });
      page.setDefaultTimeout(15000);
      await page.evaluateOnNewDocument(() => {
        try {
          localStorage.clear();
        } catch {
          // Storage may be unavailable; the test then sees the default.
        }
      });

      const pageErrors = [];
      const consoleErrors = [];
      const externalRequests = [];
      page.on("pageerror", (error) => pageErrors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") consoleErrors.push(message.text());
      });
      page.on("request", (request) => {
        const url = request.url();
        if (!url.startsWith("http://") && !url.startsWith("https://")) return;
        if (new URL(url).origin !== base) externalRequests.push(url);
      });

      await page.goto(`${base}/app/online-map.html`, {
        waitUntil: "load",
        timeout: 15000,
      });
      await page.waitForFunction(() => window.__ready === true, {
        timeout: 15000,
      });

      const initial = await page.evaluate(() => ({
        api: typeof window.__api?.setOnline,
        calls: window.__styleCalls.length,
        checked: document.getElementById("online-map-toggle").checked,
        stored: localStorage.getItem("pinpoint.onlineMapWeb"),
        errorHidden: document
          .getElementById("online-map-error")
          .classList.contains("hidden"),
        offlineHidden: document
          .getElementById("map-attribution-offline")
          .classList.contains("hidden"),
        onlineHidden: document
          .getElementById("map-attribution-online")
          .classList.contains("hidden"),
        base: window.__baseOptions,
      }));

      check(initial.api === "function", "adapter exposes setOnline");
      check(initial.calls === 0, "online basemap is off by default (no setStyle call)");
      check(
        initial.checked === false && initial.stored === null,
        "toggle starts unchecked and no preference is stored",
        JSON.stringify({ checked: initial.checked, stored: initial.stored }),
      );
      check(
        initial.offlineHidden === false && initial.onlineHidden === true,
        "local attribution is shown while online is off",
      );
      check(
        initial.base.styleUrl === LOCAL_STYLE &&
          initial.base.pmtilesUrl === LOCAL_PMTILES,
        "the foundation starts on the local style and PMTiles fixture",
        JSON.stringify(initial.base),
      );

      await page.click("#online-map-toggle");
      await page.waitForFunction(
        (url) =>
          window.__styleCalls.length === 1 &&
          window.__styleCalls[0].style === url &&
          window.__styleCalls[0].pmtilesUrl === null,
        { timeout: 5000 },
        ONLINE_URL,
      );
      const onlineState = await page.evaluate(() => ({
        calls: window.__styleCalls,
        checked: document.getElementById("online-map-toggle").checked,
        stored: localStorage.getItem("pinpoint.onlineMapWeb"),
        offlineHidden: document
          .getElementById("map-attribution-offline")
          .classList.contains("hidden"),
        onlineHidden: document
          .getElementById("map-attribution-online")
          .classList.contains("hidden"),
        errorHidden: document
          .getElementById("online-map-error")
          .classList.contains("hidden"),
      }));
      check(
        onlineState.calls[0]?.style === ONLINE_URL,
        "the toggle calls setStyle with the OpenFreeMap URL",
        JSON.stringify(onlineState.calls),
      );
      check(
        onlineState.checked === true &&
          onlineState.stored === "1" &&
          onlineState.onlineHidden === false &&
          onlineState.offlineHidden === true &&
          onlineState.errorHidden === true,
        "enabling online persists the choice and swaps the credit",
        JSON.stringify(onlineState),
      );

      await page.click("#online-map-toggle");
      await page.waitForFunction(
        (local) =>
          window.__styleCalls.length === 2 &&
          window.__styleCalls[1].style === local.style &&
          window.__styleCalls[1].pmtilesUrl === local.pmtiles,
        { timeout: 5000 },
        { style: LOCAL_STYLE, pmtiles: LOCAL_PMTILES },
      );
      const offlineState = await page.evaluate(() => ({
        calls: window.__styleCalls,
        checked: document.getElementById("online-map-toggle").checked,
        stored: localStorage.getItem("pinpoint.onlineMapWeb"),
        offlineHidden: document
          .getElementById("map-attribution-offline")
          .classList.contains("hidden"),
        onlineHidden: document
          .getElementById("map-attribution-online")
          .classList.contains("hidden"),
      }));
      check(
        offlineState.calls[1]?.style === LOCAL_STYLE &&
          offlineState.calls[1]?.pmtilesUrl === LOCAL_PMTILES,
        "turning online off restores the local style and PMTiles URL",
        JSON.stringify(offlineState.calls),
      );
      check(
        offlineState.checked === false &&
          offlineState.stored === "0" &&
          offlineState.offlineHidden === false &&
          offlineState.onlineHidden === true,
        "turning online off persists the choice and restores the local credit",
        JSON.stringify(offlineState),
      );

      await page.evaluate(() => {
        globalThis.__failOnline = true;
      });
      await page.click("#online-map-toggle");
      await page.waitForFunction(
        (local) =>
          window.__styleCalls.length === 4 &&
          window.__styleCalls[2].style.includes("openfreemap") &&
          window.__styleCalls[3].style === local.style &&
          window.__styleCalls[3].pmtilesUrl === local.pmtiles,
        { timeout: 5000 },
        { style: LOCAL_STYLE, pmtiles: LOCAL_PMTILES },
      );
      const failure = await page.evaluate(() => ({
        calls: window.__styleCalls,
        checked: document.getElementById("online-map-toggle").checked,
        stored: localStorage.getItem("pinpoint.onlineMapWeb"),
        errorText: document.getElementById("online-map-error").textContent,
        errorHidden: document
          .getElementById("online-map-error")
          .classList.contains("hidden"),
        offlineHidden: document
          .getElementById("map-attribution-offline")
          .classList.contains("hidden"),
        onlineHidden: document
          .getElementById("map-attribution-online")
          .classList.contains("hidden"),
      }));
      check(
        failure.calls[2]?.style === ONLINE_URL &&
          failure.calls[3]?.style === LOCAL_STYLE,
        "a failed online style reverts to the local style",
        JSON.stringify(failure.calls),
      );
      check(
        failure.checked === false &&
          failure.stored === "0" &&
          failure.errorHidden === false &&
          /unavailable/i.test(failure.errorText) &&
          failure.offlineHidden === false &&
          failure.onlineHidden === true,
        "a failed online style shows the inline error and local credit",
        JSON.stringify(failure),
      );

      // Recreate the map with a saved preference: the adapter must apply it.
      await page.evaluate(() => {
        window.__api.destroy();
        globalThis.__failOnline = false;
        window.__styleCalls.length = 0;
        localStorage.setItem("pinpoint.onlineMapWeb", "1");
        window.__createMap();
      });
      await page.waitForFunction(
        (url) =>
          window.__ready === true &&
          window.__styleCalls.length === 1 &&
          window.__styleCalls[0].style === url,
        { timeout: 5000 },
        ONLINE_URL,
      );
      const recreated = await page.evaluate(() => ({
        checked: document.getElementById("online-map-toggle").checked,
        stored: localStorage.getItem("pinpoint.onlineMapWeb"),
        offlineHidden: document
          .getElementById("map-attribution-offline")
          .classList.contains("hidden"),
        onlineHidden: document
          .getElementById("map-attribution-online")
          .classList.contains("hidden"),
      }));
      check(
        recreated.checked === true &&
          recreated.stored === "1" &&
          recreated.onlineHidden === false &&
          recreated.offlineHidden === true,
        "a saved online preference is applied when the map is recreated",
        JSON.stringify(recreated),
      );

      // Without the page-owned meta the adapter must fail closed, not fetch.
      await page.evaluate(() => {
        window.__api.destroy();
        document
          .querySelector('meta[name="pinpoint-online-style"]')
          ?.remove();
        window.__styleCalls.length = 0;
        localStorage.setItem("pinpoint.onlineMapWeb", "0");
        window.__createMap();
      });
      await page.waitForFunction(() => window.__ready === true, {
        timeout: 5000,
      });
      await page.click("#online-map-toggle");
      await page.waitForFunction(
        () =>
          !document
            .getElementById("online-map-error")
            .classList.contains("hidden"),
        { timeout: 5000 },
      );
      const missingConfig = await page.evaluate(() => ({
        calls: window.__styleCalls.length,
        checked: document.getElementById("online-map-toggle").checked,
        stored: localStorage.getItem("pinpoint.onlineMapWeb"),
        errorText: document.getElementById("online-map-error").textContent,
      }));
      check(
        missingConfig.calls === 0 &&
          missingConfig.checked === false &&
          missingConfig.stored === "0" &&
          /not configured/i.test(missingConfig.errorText),
        "a missing page-owned style URL shows the inline error and sends nothing",
        JSON.stringify(missingConfig),
      );

      check(
        externalRequests.length === 0,
        "zero requests to the online host in the mocked test",
        externalRequests.join(", "),
      );
      check(pageErrors.length === 0, "zero page errors", pageErrors.join(" | "));
      check(
        consoleErrors.length === 0,
        "zero console errors",
        consoleErrors.join(" | "),
      );
    } finally {
      await browser.close();
    }
  }
} finally {
  await new Promise((resolve) => server.close(resolve));
  rmSync(STAGE, { recursive: true, force: true });
}

console.log("");
if (failures === 0) {
  console.log(
    browserRan
      ? "Online map checks passed."
      : "Online map checks passed (static only).",
  );
  process.exitCode = 0;
} else {
  console.error(`Online map checks failed: ${failures}`);
  process.exitCode = 1;
}

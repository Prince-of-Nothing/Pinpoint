/**
 * Headless integration test for `web/app/map.js` (MapLibre adapter).
 *
 * Static checks always run. With Puppeteer available (or NODE_PATH pointing at
 * a node_modules that has it) the test stages the real adapter, the shared map
 * foundation, the vendored MapLibre/PMTiles build and the committed fixture in
 * a temp directory, serves them with `web/map/serve.mjs` (Range support) and
 * opens `map-adapter.html` in headless Chromium.
 *
 * Usage (PowerShell):
 *   $env:NODE_PATH="C:\path\to\node_modules"
 *   node web/app/test/map-adapter.mjs
 *
 * Static-only fallback:
 *   node web/app/test/map-adapter.mjs --static-only
 */

import { createHash } from "node:crypto";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createStaticServer } from "../../map/serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(HERE, "../..");
const FIXTURE = path.join(WEB_ROOT, "map", "fixtures", "monaco.pmtiles");
const FIXTURE_BYTES = 94389;
const FIXTURE_SHA256 =
  "a600dbfd9ddb25b82661f01d228a39d2b386dd13e401d2081b3ce7fd9c6de82f";
const SCREENSHOT =
  process.env.PINPOINT_ADAPTER_SCREENSHOT ||
  path.join(tmpdir(), "web-app-map-adapter.png");
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

// ---------------------------------------------------------------------------
// Stage the same URL layout the Rust server exposes: /app/*, /map-assets/*,
// /vendor/*. This exercises the adapter's absolute imports without copying or
// rewriting any of the shipped source.
// ---------------------------------------------------------------------------
const STAGE = mkdtempSync(path.join(tmpdir(), "pinpoint-map-adapter-"));
mkdirSync(path.join(STAGE, "app"), { recursive: true });
mkdirSync(path.join(STAGE, "map-assets", "fixtures"), { recursive: true });

copyFileSync(path.join(WEB_ROOT, "app", "map.js"), path.join(STAGE, "app", "map.js"));
copyFileSync(path.join(WEB_ROOT, "app", "app.css"), path.join(STAGE, "app", "app.css"));
copyFileSync(
  path.join(HERE, "map-adapter.html"),
  path.join(STAGE, "app", "map-adapter.html"),
);
copyFileSync(
  path.join(HERE, "map-adapter-driver.js"),
  path.join(STAGE, "app", "map-adapter-driver.js"),
);
copyFileSync(
  path.join(WEB_ROOT, "map", "map.js"),
  path.join(STAGE, "map-assets", "map.js"),
);
copyFileSync(
  path.join(WEB_ROOT, "map", "style.json"),
  path.join(STAGE, "map-assets", "style.json"),
);
copyFileSync(
  path.join(WEB_ROOT, "map", "fixtures", "monaco.pmtiles"),
  path.join(STAGE, "map-assets", "fixtures", "monaco.pmtiles"),
);
cpSync(path.join(WEB_ROOT, "vendor"), path.join(STAGE, "vendor"), {
  recursive: true,
});

const server = createStaticServer({ root: STAGE });
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
  rmSync(STAGE, { recursive: true, force: true });
  process.exit(1);
}
const base = `http://127.0.0.1:${address.port}`;
console.log(`Serving staged web assets at ${base}`);

try {
  // -------------------------------------------------------------------------
  // Static checks
  // -------------------------------------------------------------------------
  const adapter = readFileSync(path.join(WEB_ROOT, "app", "map.js"), "utf8");
  const indexHtml = readFileSync(path.join(WEB_ROOT, "app", "index.html"), "utf8");
  const appCss = readFileSync(path.join(WEB_ROOT, "app", "app.css"), "utf8");
  const testPage = readFileSync(path.join(HERE, "map-adapter.html"), "utf8");
  const styleText = readFileSync(path.join(WEB_ROOT, "map", "style.json"), "utf8");
  const style = JSON.parse(styleText);

  check(
    adapter.includes('from "/map-assets/map.js"'),
    "adapter imports the shared foundation module",
  );
  check(
    adapter.includes('from "/vendor/maplibre-gl.js"'),
    "adapter imports same-origin MapLibre",
  );
  check(
    !/leaflet/i.test(adapter) && !/leaflet/i.test(indexHtml) && !/leaflet/i.test(appCss),
    "no Leaflet references remain in the web app shell",
  );
  check(
    !indexHtml.includes("/map/tiles/"),
    "the app no longer requests the raster tile proxy",
  );
  check(
    indexHtml.includes("/vendor/maplibre-gl.css") &&
      indexHtml.includes("/map-assets/fixtures/monaco.pmtiles") &&
      indexHtml.includes('name="pinpoint-map-data"'),
    "index.html loads MapLibre CSS and configures the same-origin fixture",
  );
  check(
    indexHtml.includes("OpenStreetMap contributors") &&
      indexHtml.includes("OpenMapTiles") &&
      indexHtml.includes("ODbL 1.0"),
    "index.html shows visible OSM/OpenMapTiles/ODbL attribution",
  );
  check(
    testPage.includes("OpenStreetMap contributors") &&
      testPage.includes("OpenMapTiles") &&
      testPage.includes("ODbL 1.0"),
    "test page shows the same attribution",
  );
  check(
    adapter.includes("setFriendMarkers") &&
      adapter.includes("setSelectedPlace") &&
      adapter.includes("setSelfLocation") &&
      adapter.includes("clearSelfLocation") &&
      adapter.includes("drawRoute") &&
      adapter.includes("clearRoute") &&
      adapter.includes("fitRoute") &&
      adapter.includes("locate"),
    "adapter keeps the app/nav public API",
  );
  check(
    style.sources?.basemap?.url?.startsWith("pmtiles://"),
    "style binds the basemap with the pmtiles:// protocol",
  );
  check(
    !("glyphs" in style) && !("sprite" in style),
    "style has no remote glyph or sprite host",
  );
  check(
    JSON.stringify(style.sources?.basemap?.attribution ?? "").includes(
      "OpenStreetMap contributors",
    ),
    "style source carries the OSM attribution",
  );

  const fixture = readFileSync(FIXTURE);
  const fixtureHash = createHash("sha256").update(fixture).digest("hex");
  check(
    fixture.length === FIXTURE_BYTES && fixtureHash === FIXTURE_SHA256,
    "committed Monaco fixture matches the M1 manifest",
    `${fixture.length} bytes, ${fixtureHash}`,
  );

  const served = [
    ["/app/map.js", "text/javascript"],
    ["/app/app.css", "text/css"],
    ["/app/map-adapter.html", "text/html"],
    ["/app/map-adapter-driver.js", "text/javascript"],
    ["/map-assets/map.js", "text/javascript"],
    ["/map-assets/style.json", "application/json"],
    ["/map-assets/fixtures/monaco.pmtiles", "application/octet-stream"],
    ["/vendor/maplibre-gl.js", "text/javascript"],
    ["/vendor/maplibre-gl-worker.mjs", "text/javascript"],
    ["/vendor/maplibre-gl.css", "text/css"],
    ["/vendor/pmtiles.js", "text/javascript"],
  ];
  for (const [pathname, expectedType] of served) {
    const response = await fetch(base + pathname);
    const type = response.headers.get("content-type") ?? "";
    check(
      response.status === 200,
      `HTTP 200 for ${pathname}`,
      `got ${response.status}`,
    );
    check(
      type.startsWith(expectedType),
      `content type for ${pathname}`,
      `got ${type}`,
    );
    await response.arrayBuffer();
  }

  const rangeResponse = await fetch(
    base + "/map-assets/fixtures/monaco.pmtiles",
    { headers: { Range: "bytes=0-126" } },
  );
  const rangeBody = Buffer.from(await rangeResponse.arrayBuffer());
  check(
    rangeResponse.status === 206,
    "PMTiles fixture range request returns HTTP 206",
    `got ${rangeResponse.status}`,
  );
  check(
    rangeResponse.headers.get("content-range") === `bytes 0-126/${FIXTURE_BYTES}` &&
      rangeBody.length === 127 &&
      rangeBody.subarray(0, 7).toString("latin1") === "PMTiles",
    "range response is the PMTiles header",
  );

  // -------------------------------------------------------------------------
  // Browser render
  // -------------------------------------------------------------------------
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

      page.on("pageerror", (error) => pageErrors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") consoleErrors.push(message.text());
      });
      page.on("requestfailed", (request) => {
        failedRequests.push(
          `${request.url()} (${request.failure()?.errorText ?? "unknown"})`,
        );
      });
      page.on("request", (request) => {
        const url = request.url();
        if (!url.startsWith("http://") && !url.startsWith("https://")) return;
        if (new URL(url).origin !== base) externalRequests.push(url);
      });

      await page.goto(`${base}/app/map-adapter.html`, {
        waitUntil: "load",
        timeout: 45000,
      });

      await page.waitForFunction(
        () =>
          window.__mapReady === true || typeof window.__mapError === "string",
        { timeout: 45000 },
      ).catch(async (error) => {
        const diagnostic = await page
          .evaluate(() => ({
            ready: window.__mapReady ?? null,
            error: window.__mapError ?? null,
            adapter: typeof window.__mapAdapter,
            test: typeof window.__pinpointTest,
            body: document.body?.innerText?.slice(0, 400) ?? "",
          }))
          .catch(() => null);
        console.error(
          `DIAG  ${JSON.stringify({
            diagnostic,
            pageErrors,
            consoleErrors,
            failedRequests,
          })}`,
        );
        throw error;
      });

      await page
        .waitForFunction(
          () => {
            const map = window.__mapAdapter?.getMap();
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
        const api = window.__mapAdapter;
        const map = api?.getMap();
        const container = document.getElementById("map");
        const canvas = document.querySelector(".maplibregl-canvas");
        let renderedMarkers = -1;
        try {
          renderedMarkers =
            map?.queryRenderedFeatures({
              layers: ["pinpoint-locations-circles"],
            }).length ?? -1;
        } catch {
          renderedMarkers = -1;
        }
        return {
          ready: window.__mapReady === true,
          error: window.__mapError ?? null,
          canvas: canvas
            ? {
                width: canvas.width,
                height: canvas.height,
                clientWidth: canvas.clientWidth,
                clientHeight: canvas.clientHeight,
              }
            : null,
          layer: Boolean(map?.getLayer("pinpoint-locations-circles")),
          renderedMarkers,
          sourceUrl: map?.getStyle()?.sources?.basemap?.url ?? null,
          selectedLabel:
            document.querySelector(".pin-selected .pin-label")?.textContent ?? "",
          selfMarker: Boolean(document.querySelector(".pin-self")),
          accuracyLayer: Boolean(map?.getLayer("pinpoint-self-accuracy-line")),
          routeCasing: Boolean(map?.getLayer("pinpoint-route-casing")),
          routeLine: Boolean(map?.getLayer("pinpoint-route-line")),
          routeMarkers: document.querySelectorAll(".pin-route-end").length,
          attribution:
            document
              .querySelector(".map-attribution")
              ?.textContent.replace(/\s+/g, " ")
              .trim() ?? "",
          theme: api?.getTheme?.() ?? null,
          containerTheme: container?.dataset.theme ?? null,
          darkClass: container?.classList.contains("pinpoint-map--dark") ?? false,
          center: api?.map.getCenter?.() ?? null,
          zoom: api?.map.getZoom?.() ?? null,
          geolocation: typeof api?.locate === "function",
        };
      });

      check(result.error === null, "no map initialization error", result.error ?? "");
      check(result.ready === true, "MapLibre load event fired through the adapter");
      check(Boolean(result.canvas), "MapLibre canvas exists");
      check(
        Boolean(result.canvas && result.canvas.width > 0 && result.canvas.height > 0),
        "MapLibre canvas has a non-zero backing size",
        JSON.stringify(result.canvas),
      );
      check(result.layer === true, "friend marker layer was added");
      check(
        result.renderedMarkers >= 3,
        "three friend markers are rendered",
        String(result.renderedMarkers),
      );
      check(
        String(result.sourceUrl ?? "").includes("pmtiles://") &&
          String(result.sourceUrl ?? "").includes("monaco.pmtiles"),
        "basemap source is bound to the same-origin PMTiles fixture",
        result.sourceUrl ?? "missing",
      );
      check(
        result.selectedLabel === "Café du Port",
        "selected place marker renders its label",
        result.selectedLabel,
      );
      check(result.selfMarker === true, "current-position marker renders");
      check(result.accuracyLayer === true, "current-position accuracy polygon renders");
      check(
        result.routeCasing && result.routeLine,
        "route casing and line layers render",
      );
      check(
        result.routeMarkers === 3,
        "route A/B/stop markers render",
        String(result.routeMarkers),
      );
      check(
        result.attribution.includes("© OpenStreetMap contributors") &&
          result.attribution.includes("OpenMapTiles") &&
          result.attribution.includes("ODbL 1.0"),
        "visible attribution names OSM/OpenMapTiles/ODbL",
        result.attribution,
      );
      check(
        result.theme === "dark" && result.darkClass === true,
        "adapter defaults to the dark theme",
        JSON.stringify({ theme: result.theme, dark: result.darkClass }),
      );
      check(
        Number.isFinite(result.center?.lat) &&
          Number.isFinite(result.center?.lng) &&
          Number.isFinite(result.zoom),
        "Leaflet-shaped getCenter/getZoom facade returns numbers",
        JSON.stringify({ center: result.center, zoom: result.zoom }),
      );
      check(result.geolocation === true, "locate() stays available");

      await page.screenshot({ path: SCREENSHOT });
      check(
        existsSync(SCREENSHOT) && statSync(SCREENSHOT).size > 10000,
        `screenshot saved (${SCREENSHOT})`,
        "file missing or too small",
      );

      // Theme switch: class, dataset and the MapLibre background must all move.
      const themeBefore = await page.evaluate(() => ({
        theme: window.__mapAdapter.getTheme(),
        background: window.__mapAdapter
          .getMap()
          ?.getPaintProperty("background", "background-color"),
      }));
      await page.evaluate(() => window.__mapAdapter.setTheme("light"));
      await page.waitForFunction(
        (previous) => {
          const api = window.__mapAdapter;
          const container = document.getElementById("map");
          const background = api
            .getMap()
            ?.getPaintProperty("background", "background-color");
          return (
            api.getTheme() === "light" &&
            container.dataset.theme === "light" &&
            !container.classList.contains("pinpoint-map--dark") &&
            background !== previous.background
          );
        },
        { timeout: 5000 },
        themeBefore,
      );
      check(true, "setTheme('light') updates the class and MapLibre paint");
      await page.evaluate(() => window.__mapAdapter.setTheme("dark"));

      // Click callback: marker popups may cover the centre, so use a corner.
      const clickPoint = await page.$eval("#map", (element) => {
        const rect = element.getBoundingClientRect();
        return { x: rect.left + rect.width * 0.16, y: rect.top + rect.height * 0.8 };
      });
      await page.mouse.click(clickPoint.x, clickPoint.y);
      await page.waitForFunction(
        () => {
          const text = document.getElementById("click-readout")?.textContent?.trim();
          return Boolean(text && text !== "—");
        },
        { timeout: 5000 },
      );
      const clickText = await page.$eval("#click-readout", (element) =>
        element.textContent.trim(),
      );
      check(
        /^-?\d+\.\d{4}, -?\d+\.\d{4}$/.test(clickText),
        "onClick reports { lat, lon }",
        clickText,
      );

      // Leaflet-compatible view methods used by nav.js must not throw.
      const facade = await page.evaluate(() => {
        const api = window.__mapAdapter;
        api.map.invalidateSize();
        api.resize();
        api.map.flyTo([43.74, 7.42], 14, { duration: 0.05 });
        api.map.panTo([43.74, 7.42], { animate: false });
        return {
          lat: typeof api.map.getCenter().lat,
          lng: typeof api.map.getCenter().lng,
          zoom: typeof api.map.getZoom(),
        };
      });
      check(
        facade.lat === "number" &&
          facade.lng === "number" &&
          facade.zoom === "number",
        "flyTo/panTo/resize/resize facade calls succeed",
        JSON.stringify(facade),
      );

      const fitBefore = await page.evaluate(() => ({
        zoom: window.__mapAdapter.map.getZoom(),
      }));
      await page.evaluate(() =>
        window.__mapAdapter.fitRoute(window.__pinpointTest.route),
      );
      await page
        .waitForFunction(
          (previous) =>
            window.__mapAdapter.map.getZoom() > previous.zoom + 0.1,
          { timeout: 5000 },
          fitBefore,
        )
        .catch(() => {});
      const fitAfter = await page.evaluate(() => ({
        zoom: window.__mapAdapter.map.getZoom(),
      }));
      check(
        fitAfter.zoom > fitBefore.zoom,
        "fitRoute fits the drawn route",
        JSON.stringify({ fitBefore, fitAfter }),
      );

      const pmtilesRangeRequests = pmtilesResponses.filter(
        (entry) => entry.status === 206,
      );
      check(
        pmtilesRangeRequests.length > 0,
        "browser fetched the PMTiles fixture with Range requests",
        JSON.stringify(pmtilesResponses.slice(0, 5)),
      );
      check(
        externalRequests.length === 0,
        "zero external network requests",
        externalRequests.join(", "),
      );
      check(
        failedRequests.length === 0,
        "zero failed network requests",
        failedRequests.join(", "),
      );
      check(pageErrors.length === 0, "zero page errors", pageErrors.join(" | "));
      check(
        consoleErrors.length === 0,
        "zero console errors",
        consoleErrors.join(" | "),
      );

      // Destroy must remove the canvas, markers and layers and be idempotent.
      await page.evaluate(() => {
        window.__mapAdapter.destroy();
        window.__mapAdapter.destroy();
      });
      await page.waitForFunction(
        () =>
          !document.querySelector(".maplibregl-canvas") &&
          !document.querySelector(".pin-selected") &&
          !document.querySelector(".pin-self") &&
          document.querySelectorAll(".pin-route-end").length === 0,
        { timeout: 5000 },
      );
      check(true, "destroy() removes the canvas and all markers twice safely");
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
      ? `Map adapter checks passed. Screenshot: ${SCREENSHOT}`
      : "Map adapter checks passed (static only).",
  );
  process.exitCode = 0;
} else {
  console.error(`Map adapter checks failed: ${failures}`);
  process.exitCode = 1;
}

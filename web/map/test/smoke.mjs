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

      page.on("pageerror", (error) => pageErrors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") consoleErrors.push(message.text());
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
      check(consoleErrors.length === 0, "no console errors", consoleErrors.join(" | "));
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

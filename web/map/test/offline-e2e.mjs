/**
 * Browser E2E for the native offline pack install.
 *
 * Started by `tests/map_offline_e2e.rs` against the real desktop loopback map
 * server and a local artifact host. It presses the Offline map button, waits
 * for streamed progress and the installed state, checks `/map/config`, waits
 * for the map to reload onto `/packs/moldova.pmtiles`, and routes over the
 * downloaded graph. Any external request, page error or console error fails.
 *
 * Usage (normally through the ignored Rust test):
 *   node web/map/test/offline-e2e.mjs http://127.0.0.1:PORT/map/index.html
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
  process.env.PINPOINT_SCREENSHOT ||
  path.join(tmpdir(), "pinpoint-map-offline.png");

if (!indexUrl) {
  console.error("usage: node web/map/test/offline-e2e.mjs <map index URL>");
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
  const packResponses = [];

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
  page.on("response", (response) => {
    let parsed;
    try {
      parsed = new URL(response.url());
    } catch {
      return;
    }
    if (parsed.origin === origin && parsed.pathname === "/packs/moldova.pmtiles") {
      packResponses.push(response.status());
    }
  });
  page.on("requestfailed", (request) => {
    const errorText = request.failure()?.errorText ?? "unknown";
    // MapLibre cancels in-flight tile reads when the map is replaced after an
    // install; an intentional cancellation is not a failed request.
    if (errorText.includes("ERR_ABORTED")) return;
    failedRequests.push(`${request.url()} (${errorText})`);
  });

  await page.goto(indexUrl, { waitUntil: "load", timeout: 20000 });
  await page.waitForFunction(() => window.__pinpointMapReady === true);
  check(true, "initial MapLibre map loaded");

  const initial = await page.evaluate(async () => {
    const config = await fetch("/map/config", { cache: "no-store" }).then((r) =>
      r.json(),
    );
    return {
      config,
      panel: document.querySelector("#offline-panel")?.dataset.state ?? "",
      badge: document.querySelector("#offline-badge")?.textContent ?? "",
      detail: document.querySelector("#offline-detail")?.textContent ?? "",
      graph: document.querySelector("#offline-graph")?.textContent ?? "",
      buttonHidden: document.querySelector("#offline-download")?.hidden ?? null,
      buttonText: document.querySelector("#offline-download")?.textContent ?? "",
      source: window.__pinpointPreview?.source?.pmtilesUrl ?? "",
      status: await fetch("/map/offline/status", { cache: "no-store" }).then((r) =>
        r.json(),
      ),
    };
  });
  check(
    initial.config.pmtilesUrl === "./fixtures/monaco.pmtiles",
    "config starts on the Monaco fixture",
    initial.config.pmtilesUrl,
  );
  check(
    initial.config.packInstalled === false && initial.config.graphInstalled === false,
    "config reports nothing installed",
    JSON.stringify(initial.config),
  );
  check(
    initial.status.pack.installed === false && initial.status.graph.installed === false,
    "status starts uninstalled",
    JSON.stringify(initial.status),
  );
  check(initial.source === "./fixtures/monaco.pmtiles", "initial source is the fixture");
  check(initial.panel === "idle", "offline panel starts idle", initial.panel);
  check(
    initial.buttonHidden === false &&
      initial.buttonText.startsWith("Download Moldova map ("),
    "download button is visible with the size label",
    `${initial.buttonHidden} ${initial.buttonText}`,
  );
  check(
    initial.graph === "Download routing data",
    "routing graph state prompts the download",
    initial.graph,
  );
  check(
    Number(initial.status.pack.expected_bytes) > 0 &&
      Number(initial.status.graph.expected_bytes) > 0,
    "status publishes expected artifact sizes",
    JSON.stringify(initial.status),
  );

  await page.click("#offline-download");

  await page.waitForFunction(
    () => {
      const progress = document.querySelector("#offline-progress");
      return progress && !progress.hidden && Number(progress.value) > 0;
    },
    { timeout: 15000 },
  );
  const during = await page.evaluate(() => ({
    state: document.querySelector("#offline-panel")?.dataset.state ?? "",
    badge: document.querySelector("#offline-badge")?.textContent ?? "",
    detail: document.querySelector("#offline-detail")?.textContent ?? "",
    progress: Number(document.querySelector("#offline-progress")?.value ?? 0),
    expected: Number(document.querySelector("#offline-progress")?.max ?? 0),
    buttonHidden: document.querySelector("#offline-download")?.hidden ?? null,
  }));
  check(
    during.state === "downloading",
    "offline panel reports downloading",
    during.state,
  );
  check(
    during.progress > 0 && during.progress <= during.expected,
    "progress bar advances within the expected size",
    `${during.progress}/${during.expected}`,
  );
  check(
    /Downloading|Verifying/.test(during.detail),
    "progress detail names the phase",
    during.detail,
  );
  check(during.buttonHidden === true, "download button hides while running");

  await page.waitForFunction(
    () => document.querySelector("#offline-panel")?.dataset.state === "installed",
    { timeout: 30000 },
  );
  const installed = await page.evaluate(async () => {
    const config = await fetch("/map/config", { cache: "no-store" }).then((r) =>
      r.json(),
    );
    return {
      config,
      badge: document.querySelector("#offline-badge")?.textContent ?? "",
      graph: document.querySelector("#offline-graph")?.textContent ?? "",
      buttonHidden: document.querySelector("#offline-download")?.hidden ?? null,
      status: await fetch("/map/offline/status", { cache: "no-store" }).then((r) =>
        r.json(),
      ),
    };
  });
  check(installed.badge === "Installed", "badge reports installed", installed.badge);
  check(
    installed.graph === "Routing graph installed",
    "routing graph is installed",
    installed.graph,
  );
  check(installed.buttonHidden === true, "download button is hidden after install");
  check(
    installed.config.pmtilesUrl === "/packs/moldova.pmtiles" &&
      installed.config.packInstalled === true &&
      installed.config.graphInstalled === true,
    "config points at the installed PMTiles pack",
    JSON.stringify(installed.config),
  );
  check(
    installed.status.pack.installed === true &&
      installed.status.graph.installed === true &&
      installed.status.pack.bytes === installed.status.pack.expected_bytes &&
      installed.status.graph.bytes === installed.status.graph.expected_bytes,
    "status reports exact installed sizes",
    JSON.stringify(installed.status),
  );

  await page.waitForFunction(
    () =>
      window.__pinpointMapReady === true &&
      window.__pinpointPreview?.source?.pmtilesUrl === "/packs/moldova.pmtiles",
    { timeout: 20000 },
  );
  const reloaded = await page.evaluate(() => {
    const map = window.__pinpointPreview?.map;
    return {
      source: window.__pinpointPreview?.source?.pmtilesUrl ?? "",
      styleUrl: map?.getStyle()?.sources?.basemap?.url ?? "",
      meta: document.querySelector("#map-meta")?.textContent ?? "",
      status: document.querySelector("#status")?.dataset.state ?? "",
    };
  });
  check(
    reloaded.source === "/packs/moldova.pmtiles",
    "page switched to the installed pack URL",
    reloaded.source,
  );
  check(
    String(reloaded.styleUrl).includes("/packs/moldova.pmtiles"),
    "MapLibre source is bound to the installed pack",
    reloaded.styleUrl,
  );
  check(reloaded.meta.includes("Moldova"), "map meta names Moldova", reloaded.meta);
  check(reloaded.status === "ready", "reloaded map is ready", reloaded.status);
  check(
    packResponses.filter((status) => status === 206).length > 0,
    "installed pack was read with HTTP Range 206",
    JSON.stringify(packResponses),
  );

  await page.waitForFunction(
    () => window.__pinpointRoute?.ui?.element?.isConnected === true,
  );
  await page.$eval(
    '#route-panel input[name="from"]',
    (element, value) => {
      element.value = value;
      element.dispatchEvent(new Event("input", { bubbles: true }));
      element.dispatchEvent(new Event("change", { bubbles: true }));
    },
    FROM,
  );
  await page.$eval(
    '#route-panel input[name="to"]',
    (element, value) => {
      element.value = value;
      element.dispatchEvent(new Event("input", { bubbles: true }));
      element.dispatchEvent(new Event("change", { bubbles: true }));
    },
    TO,
  );
  await page.click('#route-panel [data-profile="auto"]');

  const responsePromise = page.waitForResponse(
    (response) => {
      const url = new URL(response.url());
      return url.origin === origin && url.pathname === "/map/route";
    },
    { timeout: 30000 },
  );
  await page.click('#route-panel button[data-action="route"]');
  const routeResponse = await responsePromise;
  const body = await routeResponse.json();

  check(routeResponse.status() === 200, "downloaded graph routes locally");
  check(Array.isArray(body.points) && body.points.length > 1, "route has geometry");
  check(
    Array.isArray(body.maneuvers) && body.maneuvers.length > 0,
    "route has turn-by-turn maneuvers",
  );
  check(
    routeRequests.length === 1 &&
      String(routeRequests[0]).includes("from_lat=47.0105"),
    "exactly one local route request",
    JSON.stringify(routeRequests),
  );

  await page.waitForFunction(
    () =>
      Boolean(window.__pinpointPreview?.map?.getLayer("pinpoint-route-line")) &&
      Boolean(window.__pinpointPreview?.map?.getSource("pinpoint-route")),
  );
  const summary = await page.$eval('[data-role="summary"]', (element) =>
    element.textContent.replace(/\s+/g, " ").trim(),
  );
  check(summary.includes("Auto"), "route summary renders", summary);

  await page.screenshot({ path: screenshot });
  check(true, `screenshot saved (${screenshot})`);

  check(
    externalRequests.length === 0,
    "no external network requests",
    externalRequests.join(", "),
  );
  check(failedRequests.length === 0, "no failed network requests", failedRequests.join(", "));
  check(pageErrors.length === 0, "no page errors", pageErrors.join(" | "));
  check(consoleErrors.length === 0, "no console errors", consoleErrors.join(" | "));

  console.log(
    `OFFLINE_E2E ${JSON.stringify({
      pack_bytes: installed.status.pack.bytes,
      graph_bytes: installed.status.graph.bytes,
      pack_ranges: packResponses.filter((status) => status === 206).length,
      route_points: body.points.length,
      maneuvers: body.maneuvers.length,
      first: body.maneuvers[0]?.instruction ?? "",
    })}`,
  );
} finally {
  await browser.close();
}

if (failures > 0) {
  console.error(`Map offline E2E failed: ${failures} check(s)`);
  process.exit(1);
}
console.log("Map offline E2E passed.");

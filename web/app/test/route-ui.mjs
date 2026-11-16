/**
 * Smoke test for the Pinpoint route and turn-by-turn UI.
 *
 * Static checks and module-level unit checks always run. When Puppeteer is
 * available, the test also serves `web/` on a loopback port, drives
 * `web/app/route-demo.html` in headless Chromium, asserts the whole route flow,
 * captures a screenshot and fails on any page/console/network error.
 *
 * Usage (PowerShell):
 *   $env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
 *   $env:PINPOINT_SCREENSHOT="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\web-route-ui.png"
 *   node web/app/test/route-ui.mjs
 *
 * Static checks only (explicitly not a browser pass):
 *   node web/app/test/route-ui.mjs --static-only
 *
 * Screenshot path: $env:PINPOINT_SCREENSHOT, otherwise <system temp>/web-route-ui.png.
 */

import { existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createStaticServer } from "../../map/serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(HERE, "../..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..");
const APP_ROOT = path.join(WEB_ROOT, "app");
const SCREENSHOT = process.env.PINPOINT_SCREENSHOT || path.join(tmpdir(), "web-route-ui.png");
const STATIC_ONLY = process.argv.includes("--static-only");
const require = createRequire(import.meta.url);

const NEW_FILES = [
  "web/app/route-ui.js",
  "web/app/route.css",
  "web/app/route-demo.html",
  "web/app/route-demo.js",
  "web/app/README-route-ui.md",
  "web/app/test/route-ui.mjs",
];

const BANNED_HOSTS = [
  "unpkg.com",
  "jsdelivr",
  "cdn.",
  "cdnjs",
  "fonts.googleapis",
  "fonts.gstatic",
  "tile.openstreetmap.org",
  "google-analytics",
  "googletagmanager",
];

// Built at runtime so this guard file itself never contains the broken text.
const BROKEN_AC_ATTRIBUTION = "Ac" + " OpenStreetMap";

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

function read(relative) {
  return readFileSync(path.join(REPO_ROOT, relative), "utf8");
}

async function text(page, selector) {
  return page.$eval(selector, (element) => element.textContent.replace(/\s+/g, " ").trim());
}

async function isVisible(page, selector) {
  return page.$eval(selector, (element) => !element.hidden && element.getClientRects().length > 0);
}

// ---------------------------------------------------------------------------
// Files, source policy and pure helpers
// ---------------------------------------------------------------------------

const routeSource = read("web/app/route-ui.js");
const cssSource = read("web/app/route.css");
const demoHtml = read("web/app/route-demo.html");
const demoJs = read("web/app/route-demo.js");
const readme = read("web/app/README-route-ui.md");

check(routeSource.includes("export function createRouteUI"), "route-ui.js exports createRouteUI");
check(
  !/^\s*import\s/m.test(routeSource),
  "route-ui.js is self-contained (no imports)",
);
for (const [needle, label] of [
  ["fetch(", "fetch"],
  ["XMLHttpRequest", "XMLHttpRequest"],
  ["WebSocket", "WebSocket"],
  ["EventSource", "EventSource"],
  ["sendBeacon", "sendBeacon"],
  ["http://", "absolute http URL"],
  ["https://", "absolute https URL"],
]) {
  check(!routeSource.includes(needle), `route-ui.js does not use ${label}`);
}
check(!routeSource.includes("tile.openstreetmap.org"), "route-ui.js never references OSM tiles");

check(cssSource.includes(".pinpoint-route"), "route.css scopes styles under .pinpoint-route");
check(!cssSource.includes("@import"), "route.css has no @import");
check(!cssSource.includes("url("), "route.css has no url() assets");
check(!cssSource.includes("http"), "route.css has no absolute URLs");

check(demoHtml.includes('href="route.css"'), "demo loads the local route.css");
check(demoHtml.includes('src="route-demo.js"'), "demo loads the local route-demo.js");
check(!demoHtml.includes("<link rel=\"stylesheet\" href=\"http"), "demo has no remote stylesheet");
check(!demoHtml.includes("<script src=\"http"), "demo has no remote script");
const htmlLower = demoHtml.toLowerCase();
for (const host of BANNED_HOSTS) {
  check(!htmlLower.includes(host), `demo HTML does not reference ${host}`);
}
const absoluteInHtml = (demoHtml.match(/https:\/\//g) || []).length;
check(absoluteInHtml === 2, "demo HTML has only the two attribution links", String(absoluteInHtml));
check(demoHtml.includes("OpenStreetMap contributors"), "demo shows OpenStreetMap attribution");
check(demoHtml.includes("ODbL 1.0"), "demo shows ODbL attribution");

check(demoJs.includes('from "./route-ui.js"'), "demo imports the local component");
check(demoJs.includes("Chișinău") && demoJs.includes("Bălți"), "demo stub names Chișinău and Bălți");
check(!demoJs.includes("fetch("), "demo router never fetches");
check(demoJs.includes("const STUB_MANEUVERS"), "demo defines a local maneuver stub");
const stubManeuvers = (demoJs.match(/instruction:/g) || []).length;
check(stubManeuvers === 2, "demo stub has exactly two maneuvers", String(stubManeuvers));

for (const name of ["createRouteUI", "setRoute", "clear", "destroy", "onRoute", "onCancel"]) {
  check(readme.includes(name), `README documents ${name}`);
}

for (const relative of NEW_FILES) {
  const bytes = readFileSync(path.join(REPO_ROOT, relative));
  check(
    !(bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf),
    `${relative} is UTF-8 without BOM`,
  );

  let decoded = "";
  let valid = true;
  try {
    decoded = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    valid = false;
  }
  check(valid, `${relative} is valid UTF-8`);
  if (valid) {
    check(
      !decoded.includes("\uFFFD") &&
        !decoded.includes("\u00C2\u00A9") &&
        !decoded.includes(BROKEN_AC_ATTRIBUTION),
      `${relative} has no mojibake`,
    );
  }
}

const routeModule = await import(pathToFileURL(path.join(APP_ROOT, "route-ui.js")).href);
check(typeof routeModule.createRouteUI === "function", "route-ui.js imports as an ES module");
check(routeModule.formatDistance(128400) === "128.4 km", "formatDistance formats kilometres");
check(routeModule.formatDistance(742) === "742 m", "formatDistance formats metres");
check(routeModule.formatDuration(6600) === "1 h 50 min", "formatDuration formats hours");
check(routeModule.formatDuration(840) === "14 min", "formatDuration formats minutes");

const parsed = routeModule.parseLatLon("47.0105, 28.8638");
check(parsed.lat === 47.0105 && parsed.lon === 28.8638, "parseLatLon accepts lat, lon");
const parsedSpace = routeModule.parseLatLon("47.0105 28.8638");
check(
  parsedSpace.lat === 47.0105 && parsedSpace.lon === 28.8638,
  "parseLatLon accepts whitespace separation",
);
for (const [input, label] of [
  ["not coordinates", "garbage"],
  ["91, 28", "out-of-range latitude"],
  ["47, 181", "out-of-range longitude"],
]) {
  let threw = false;
  try {
    routeModule.parseLatLon(input);
  } catch {
    threw = true;
  }
  check(threw, `parseLatLon rejects ${label}`);
}

// ---------------------------------------------------------------------------
// HTTP + browser flow
// ---------------------------------------------------------------------------

const server = createStaticServer({ root: WEB_ROOT });
await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});

const address = server.address();
if (!address || typeof address === "string") {
  console.error("FAIL  could not determine the test server port");
  server.close();
  process.exit(1);
}
const base = `http://127.0.0.1:${address.port}`;
console.log(`Serving ${WEB_ROOT} at ${base}`);

try {
  for (const [pathname, expectedType] of [
    ["/app/route-demo.html", "text/html"],
    ["/app/route-ui.js", "text/javascript"],
    ["/app/route-demo.js", "text/javascript"],
    ["/app/route.css", "text/css"],
    ["/app/README-route-ui.md", "text/plain"],
    ["/app/favicon.svg", "image/svg+xml"],
  ]) {
    const response = await fetch(base + pathname);
    const type = response.headers.get("content-type") ?? "";
    check(response.status === 200, `HTTP 200 for ${pathname}`, `got ${response.status}`);
    check(type.startsWith(expectedType), `content type for ${pathname}`, `got ${type}`);
    await response.arrayBuffer();
  }

  let puppeteer = null;
  if (!STATIC_ONLY) {
    try {
      puppeteer = require("puppeteer");
    } catch {
      console.error(
        "FAIL  browser: puppeteer is not installed; set NODE_PATH to a node_modules that contains it",
      );
      console.error(
        "LIMIT the route UI flow was NOT exercised in a browser; rerun without --static-only once Puppeteer is available",
      );
    }
  } else {
    console.log("SKIP  browser: --static-only requested; the UI flow was not exercised");
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
      await page.setViewport({ width: 1280, height: 1000, deviceScaleFactor: 1 });
      page.setDefaultTimeout(15000);

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

      await page.goto(`${base}/app/route-demo.html`, {
        waitUntil: "load",
        timeout: 15000,
      });
      await page.waitForFunction(() => window.__pinpointRouteDemo?.ready === true);
      check(true, "route demo module loaded");

      const styling = await page.evaluate(() => {
        const root = document.querySelector(".pinpoint-route");
        if (!root) return null;
        const style = getComputedStyle(root);
        return {
          display: style.display,
          borderWidth: style.borderTopWidth,
          background: style.backgroundColor,
          fromLabel: document.querySelector('input[name="from"]')?.labels?.[0]?.textContent ?? "",
          toLabel: document.querySelector('input[name="to"]')?.labels?.[0]?.textContent ?? "",
          profilePressed:
            document.querySelector('[data-profile="auto"]')?.getAttribute("aria-pressed") ?? "",
        };
      });
      check(styling !== null, "route component rendered");
      check(styling?.display === "grid", "route.css is applied", styling?.display ?? "missing");
      check(styling?.borderWidth === "1px", "route component has a visible border");
      check(
        String(styling?.fromLabel).includes("From") &&
          String(styling?.toLabel).includes("To"),
        "From and To inputs have accessible labels",
        JSON.stringify(styling),
      );
      check(styling?.profilePressed === "true", "Auto is the initial pressed profile");

      // Empty submit: validation must appear and must not call the router.
      await page.click('button[data-action="route"]');
      await page.waitForFunction(
        () => document.querySelector('[data-role="error"]')?.hidden === false,
      );
      const validationText = await text(page, '[data-role="error"]');
      check(
        validationText.includes("From") && validationText.includes("lat, lon"),
        "invalid input shows a field-specific error",
        validationText,
      );
      const callsAfterValidation = await page.evaluate(
        () => window.__pinpointRouteDemo.calls.length,
      );
      check(callsAfterValidation === 0, "invalid input never calls the router");

      await page.click('input[name="from"]');
      await page.type('input[name="from"]', "47.0105, 28.8638");
      await page.click('input[name="to"]');
      await page.type('input[name="to"]', "47.7615, 28.1940");
      await page.click('[data-profile="auto"]');

      const profileState = await page.evaluate(() => ({
        pressed: document.querySelector('[data-profile="auto"]').getAttribute("aria-pressed"),
        active: document.querySelector('[data-profile="auto"]').classList.contains("is-active"),
      }));
      check(
        profileState.pressed === "true" && profileState.active === true,
        "Auto stays selected",
        JSON.stringify(profileState),
      );

      await page.click('button[data-action="route"]');
      await page.waitForFunction(() => {
        const root = document.querySelector(".pinpoint-route");
        const summary = document.querySelector('[data-role="summary"]')?.textContent ?? "";
        return root?.dataset.state === "ready" && summary.includes("128.4 km");
      });

      const summaryText = await text(page, '[data-role="summary"]');
      check(summaryText.includes("128.4 km"), "summary shows the distance", summaryText);
      check(summaryText.includes("1 h 50 min"), "summary shows the duration", summaryText);
      check(summaryText.includes("Auto"), "summary shows the profile", summaryText);

      const steps = await page.$$eval('[data-role="steps"] li', (items) =>
        items.map((item) => item.textContent.replace(/\s+/g, " ").trim()),
      );
      check(steps.length === 2, "both maneuvers render in the directions list", String(steps.length));
      check(
        steps[0]?.includes("Head north on the M14"),
        "first maneuver text renders",
        steps[0] ?? "missing",
      );
      check(
        steps[1]?.includes("Strada Ștefan cel Mare"),
        "second maneuver text renders with diacritics",
        steps[1] ?? "missing",
      );
      check(
        steps[0]?.includes("42.0 km") && steps[1]?.includes("86.4 km"),
        "maneuver distances render",
        JSON.stringify(steps),
      );

      const bannerHidden = await page.$eval('[data-role="banner"]', (element) => element.hidden);
      check(bannerHidden === true, "next-maneuver banner is hidden before Start");

      const preview = await page.evaluate(() => ({
        polylines: document.querySelectorAll("#route-preview polyline").length,
        markers: document.querySelectorAll("#route-preview circle").length,
        readout: document.querySelector("#route-readout")?.textContent ?? "",
      }));
      check(preview.polylines === 2, "onRoute draws the route casing and line");
      check(preview.markers === 2, "onRoute draws start and destination markers");
      check(
        preview.readout.includes("128.4 km") && preview.readout.includes("2 maneuvers"),
        "demo readout reflects the route",
        preview.readout,
      );

      const calls = await page.evaluate(() => window.__pinpointRouteDemo.calls);
      check(calls.length === 1, "the injected router ran exactly once", String(calls.length));
      check(
        calls[0]?.profile === "auto" &&
          Math.abs(calls[0]?.from?.lat - 47.0105) < 1e-6 &&
          Math.abs(calls[0]?.to?.lon - 28.194) < 1e-6,
        "router received the parsed coordinates and Auto profile",
        JSON.stringify(calls),
      );

      await page.click('button[data-action="nav"]');
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-route")?.dataset.state === "navigating",
      );
      const firstBanner = await page.evaluate(() => ({
        hidden: document.querySelector('[data-role="banner"]')?.hidden,
        kicker: document.querySelector('[data-role="maneuver-kicker"]')?.textContent ?? "",
        text: document.querySelector('[data-role="maneuver"]')?.textContent ?? "",
        meta: document.querySelector('[data-role="maneuver-distance"]')?.textContent ?? "",
        navLabel: document.querySelector('button[data-action="nav"]')?.textContent ?? "",
        navPressed:
          document.querySelector('button[data-action="nav"]')?.getAttribute("aria-pressed") ?? "",
        currentStep:
          document.querySelector('[data-role="steps"] li.is-current')?.dataset.index ?? "",
      }));
      check(firstBanner.hidden === false, "Start reveals the next-maneuver banner");
      check(
        firstBanner.kicker.includes("1 of 2"),
        "banner shows the first maneuver position",
        firstBanner.kicker,
      );
      check(
        firstBanner.text.includes("Head north on the M14"),
        "banner shows the first maneuver instruction",
        firstBanner.text,
      );
      check(
        firstBanner.meta.includes("42.0 km"),
        "banner shows the maneuver distance",
        firstBanner.meta,
      );
      check(
        firstBanner.navLabel === "Stop" && firstBanner.navPressed === "true",
        "Start becomes Stop while navigating",
        JSON.stringify(firstBanner),
      );
      check(firstBanner.currentStep === "0", "first direction is highlighted");

      await page.click('button[data-action="next"]');
      await page.waitForFunction(
        () =>
          document
            .querySelector('[data-role="maneuver"]')
            ?.textContent.includes("roundabout") === true,
      );
      const secondBanner = await page.evaluate(() => ({
        kicker: document.querySelector('[data-role="maneuver-kicker"]')?.textContent ?? "",
        text: document.querySelector('[data-role="maneuver"]')?.textContent ?? "",
        meta: document.querySelector('[data-role="maneuver-distance"]')?.textContent ?? "",
        nextHidden: document.querySelector('button[data-action="next"]')?.hidden,
        currentStep:
          document.querySelector('[data-role="steps"] li.is-current')?.dataset.index ?? "",
      }));
      check(
        secondBanner.text.includes("second exit") &&
          secondBanner.text.includes("Ștefan cel Mare"),
        "Next maneuver shows the second instruction",
        secondBanner.text,
      );
      check(secondBanner.kicker === "Final maneuver", "banner marks the final maneuver");
      check(secondBanner.meta.includes("86.4 km"), "second maneuver distance renders");
      check(secondBanner.nextHidden === true, "Next maneuver is hidden on the last maneuver");
      check(secondBanner.currentStep === "1", "second direction is highlighted");

      await page.screenshot({ path: SCREENSHOT });
      const screenshotOk = existsSync(SCREENSHOT) && statSync(SCREENSHOT).size > 10000;
      check(screenshotOk, `screenshot saved (${SCREENSHOT})`, "file missing or too small");

      // Stop keeps the route; Clear removes it and notifies the host.
      await page.click('button[data-action="nav"]');
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-route")?.dataset.state === "ready",
      );
      const afterStop = await page.evaluate(() => ({
        bannerHidden: document.querySelector('[data-role="banner"]')?.hidden,
        summary: document.querySelector('[data-role="summary"]')?.textContent ?? "",
        navLabel: document.querySelector('button[data-action="nav"]')?.textContent ?? "",
      }));
      check(afterStop.bannerHidden === true, "Stop hides the banner");
      check(afterStop.summary.includes("128.4 km"), "Stop keeps the route summary");
      check(afterStop.navLabel === "Start", "Stop restores the Start label");

      await page.click('button[data-action="clear"]');
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-route")?.dataset.state === "idle",
      );
      const afterClear = await page.evaluate(() => ({
        summary: document.querySelector('[data-role="summary"]')?.textContent ?? "",
        bannerHidden: document.querySelector('[data-role="banner"]')?.hidden,
        steps: document.querySelectorAll('[data-role="steps"] li').length,
        navDisabled: document.querySelector('button[data-action="nav"]')?.disabled,
        clearDisabled: document.querySelector('button[data-action="clear"]')?.disabled,
        previewChildren: document.querySelector("#route-preview")?.childElementCount,
        readout: document.querySelector("#route-readout")?.textContent ?? "",
        cancels: window.__pinpointRouteDemo.cancels,
      }));
      check(afterClear.summary === "", "Clear empties the summary", afterClear.summary);
      check(afterClear.bannerHidden === true, "Clear hides the banner");
      check(afterClear.steps === 0, "Clear empties the directions", String(afterClear.steps));
      check(afterClear.navDisabled === true, "Clear disables Start");
      check(afterClear.clearDisabled === true, "Clear disables itself");
      check(afterClear.previewChildren === 0, "onCancel removed the preview geometry");
      check(
        afterClear.readout.includes("Route cleared"),
        "demo handled the clear cancellation",
        afterClear.readout,
      );
      check(
        afterClear.cancels.join(",") === "stop,clear",
        "onCancel received stop then clear",
        JSON.stringify(afterClear.cancels),
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
if (failures === 0 && !browserRan && !STATIC_ONLY) {
  console.error("Route UI checks failed: browser flow was not exercised (Puppeteer missing).");
  process.exitCode = 1;
} else if (failures === 0) {
  console.log(
    browserRan
      ? `Route UI checks passed. Screenshot: ${SCREENSHOT}`
      : "Route UI static checks passed (browser not exercised; explicit --static-only).",
  );
  process.exitCode = 0;
} else {
  console.error(`Route UI checks failed: ${failures}`);
  process.exitCode = 1;
}

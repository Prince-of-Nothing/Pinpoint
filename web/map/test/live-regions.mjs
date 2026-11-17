/**
 * Live/lab E2E for the global map catalog and per-region offline install.
 *
 * Runs against a deployed Pinpoint backend and a real Chromium through
 * Puppeteer. It checks the public map page, the catalog-backed region row and
 * the opt-in online basemap policy; with lab credentials it signs in through
 * the app form, downloads the selected region, polls `/map/offline/status` and
 * checks that `/map/config` switched to `/packs/<region>/`.
 *
 * Usage (PowerShell):
 *   $env:NODE_PATH = "C:\path\to\node_modules"   # must contain puppeteer
 *   $env:PINPOINT_BASE = "http://127.0.0.1:8099"
 *   $env:PINPOINT_EMAIL = "lab@example.com"      # optional
 *   $env:PINPOINT_PASSWORD = "..."               # optional
 *   node web/map/test/live-regions.mjs
 *
 * Env:
 *   NODE_PATH             node_modules that contains puppeteer
 *   PINPOINT_BASE         backend origin, default http://127.0.0.1:8099
 *   PINPOINT_EMAIL        lab account email; login is skipped without it
 *   PINPOINT_PASSWORD     lab account password; login is skipped without it
 *   PINPOINT_REGION       region id, default moldova
 *   PINPOINT_SKIP_DOWNLOAD 1/true to skip the download trigger and polling
 *
 * A missing backend or missing credentials is a SKIP (exit 0). Missing
 * puppeteer is also a SKIP. Credentials are never printed.
 */

import { createRequire } from "node:module";

const BASE = (process.env.PINPOINT_BASE || "http://127.0.0.1:8099").replace(/\/+$/, "");
const MAP_URL = `${BASE}/map/index.html`;
const APP_URL = `${BASE}/`;
const REGION = (process.env.PINPOINT_REGION || "moldova").trim();
const EMAIL = (process.env.PINPOINT_EMAIL || "").trim();
const PASSWORD = process.env.PINPOINT_PASSWORD || "";
const SKIP_DOWNLOAD = /^(1|true|yes)$/i.test(process.env.PINPOINT_SKIP_DOWNLOAD || "");
const DOWNLOAD_TIMEOUT_MS = 15 * 60 * 1000;
const POLL_MS = 2000;
const SECRETS = [EMAIL, PASSWORD].filter(Boolean);

let baseUrl;
try {
  baseUrl = new URL(BASE);
} catch {
  console.error(`FAIL  PINPOINT_BASE is not a valid URL: ${BASE}`);
  process.exit(2);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function redact(value) {
  let text = String(value ?? "");
  for (const secret of SECRETS) text = text.split(secret).join("[redacted]");
  return text;
}

let failures = 0;
let ran = false;

function check(condition, message, detail = "") {
  if (condition) {
    console.log(`PASS  ${message}`);
  } else {
    failures += 1;
    console.error(`FAIL  ${message}${detail ? ` — ${redact(detail)}` : ""}`);
  }
}

function skip(message) {
  console.log(`SKIP  ${redact(message)}`);
}

function summarize(items, limit = 5) {
  return items
    .slice(0, limit)
    .map((item) => (typeof item === "string" ? item : item.url))
    .join(", ");
}

function isOpenFreeMapHost(host) {
  const value = String(host).toLowerCase();
  return value === "openfreemap.org" || value.endsWith(".openfreemap.org");
}

async function fetchJson(pathname, timeoutMs = 5000) {
  const response = await fetch(new URL(pathname, BASE), {
    cache: "no-store",
    headers: { Accept: "application/json" },
    signal: AbortSignal.timeout(timeoutMs),
  });
  const text = await response.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = null;
  }
  return { status: response.status, ok: response.ok, body, text: text.slice(0, 300) };
}

// Smallest visible DOM subtree that mentions the region and a byte size. Works
// without assuming an id/class contract for the pending region-list UI.
function locateRegionRow(regionId, regionName) {
  const clean = (value) => String(value ?? "").replace(/\s+/g, " ").trim();
  const visible = (element) => {
    if (!element || !element.isConnected) return false;
    const style = window.getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0") {
      return false;
    }
    return element.getClientRects().length > 0;
  };
  const sizePattern = /\b\d+(?:[.,]\d+)?\s?(?:B|KB|MB|GB|KiB|MiB|GiB)\b/i;
  const id = String(regionId).toLowerCase();
  const name = String(regionName ?? regionId).toLowerCase();
  const mentions = (text) => {
    const lower = text.toLowerCase();
    return lower.includes(name) || lower.includes(id);
  };

  for (const selector of [
    `[data-region="${regionId}"]`,
    `[data-region-id="${regionId}"]`,
    `#region-${regionId}`,
    `#offline-region-${regionId}`,
  ]) {
    const element = document.querySelector(selector);
    if (!visible(element)) continue;
    const text = clean(element.innerText || element.textContent);
    return { found: true, text, hasSize: sizePattern.test(text), selector };
  }

  let best = null;
  for (const element of document.querySelectorAll("body *")) {
    if (!visible(element)) continue;
    const text = clean(element.innerText || element.textContent);
    if (!text || text.length > 600 || !mentions(text) || !sizePattern.test(text)) continue;
    const score = element.querySelectorAll("*").length;
    if (!best || score < best.score || (score === best.score && text.length < best.text.length)) {
      best = { element, text, score };
    }
  }
  if (best) {
    return { found: true, text: best.text, hasSize: true, selector: "text" };
  }

  let fallback = null;
  for (const element of document.querySelectorAll("[data-region], [data-region-id], li, tr, article")) {
    if (!visible(element)) continue;
    const text = clean(element.innerText || element.textContent);
    if (!text || !mentions(text)) continue;
    if (!fallback || text.length < fallback.text.length) fallback = { text };
  }
  return fallback
    ? { found: true, text: fallback.text, hasSize: false, selector: "text" }
    : { found: false, text: "", hasSize: false, selector: null };
}

// Finds the online basemap toggle and optionally clicks it. It must stay off by
// default; the harness tracks when it is on so only OpenFreeMap may be contacted.
function onlineToggleInfo(click) {
  const clean = (value) => String(value ?? "").replace(/\s+/g, " ").trim();
  const visible = (element) => {
    if (!element || !element.isConnected) return false;
    const style = window.getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0") {
      return false;
    }
    return element.getClientRects().length > 0;
  };

  const candidates = [];
  for (const selector of [
    "#online-map",
    "#online-toggle",
    "#map-online-toggle",
    "#offline-online",
    "#online-basemap",
    "[data-action='toggle-online']",
    "[data-action='online']",
    "[data-role='online-toggle']",
    "input[name='online']",
    "input[name='online-basemap']",
  ]) {
    candidates.push(document.querySelector(selector));
  }
  for (const element of document.querySelectorAll("button, label, [role='switch'], input[type='checkbox']")) {
    const text = clean(
      `${element.textContent ?? ""} ${element.getAttribute("aria-label") ?? ""} ${
        element.getAttribute("title") ?? ""
      }`,
    );
    if (/\bonline\b/i.test(text)) candidates.push(element);
  }

  const element = candidates.find(visible);
  if (!element) return null;

  const control =
    element.control ??
    (element.htmlFor ? document.getElementById(element.htmlFor) : null) ??
    element;
  const checked = control.type === "checkbox" ? control.checked : null;
  const pressed = control.getAttribute?.("aria-pressed") ?? element.getAttribute("aria-pressed");
  const ariaChecked =
    control.getAttribute?.("aria-checked") ?? element.getAttribute("aria-checked");
  const state = String(control.dataset?.state ?? element.dataset?.state ?? "").toLowerCase();
  const classes = `${control.className ?? ""} ${element.className ?? ""}`.toLowerCase();
  const isOn =
    checked === true ||
    pressed === "true" ||
    ariaChecked === "true" ||
    /\b(on|active|enabled)\b/.test(state) ||
    /\bactive\b/.test(classes);

  const info = {
    id: element.id || control.id || "",
    tag: element.tagName.toLowerCase(),
    text: clean(element.textContent || element.value || ""),
    checked,
    pressed,
    ariaChecked,
    state,
    isOn,
    disabled: Boolean(element.disabled || control.disabled),
    active: window.__pinpointPreview?.online ?? null,
    errorShown: (() => {
      const error = document.querySelector("#online-error");
      return Boolean(error && !error.hidden);
    })(),
  };
  if (click) {
    element.click();
    info.clicked = true;
  }
  return info;
}

// Marks the region's download control so Puppeteer can perform a trusted click.
function locateDownloadTrigger(regionId) {
  const clean = (value) => String(value ?? "").replace(/\s+/g, " ").trim();
  const visible = (element) => {
    if (!element || !element.isConnected) return false;
    const style = window.getComputedStyle(element);
    if (style.display === "none" || style.visibility === "hidden" || style.opacity === "0") {
      return false;
    }
    return element.getClientRects().length > 0;
  };

  document
    .querySelectorAll("[data-live-e2e-download]")
    .forEach((element) => element.removeAttribute("data-live-e2e-download"));

  const picks = [];
  const row = document.querySelector(
    `[data-region="${regionId}"], [data-region-id="${regionId}"], #region-${regionId}, #offline-region-${regionId}`,
  );
  if (row) {
    picks.push(...row.querySelectorAll("button, a, [role='button'], input[type='button'], input[type='submit']"));
  }
  for (const selector of [
    `[data-action="download"][data-region="${regionId}"]`,
    `[data-action="download"][data-region-id="${regionId}"]`,
    `[data-region="${regionId}"] [data-action="download"]`,
    `[data-region-id="${regionId}"] [data-action="download"]`,
    `#region-${regionId} [data-action="download"]`,
    `#offline-region-${regionId} [data-action="download"]`,
    "[data-action='download-region']",
    "#offline-download",
    "#region-download",
    "#download-region",
  ]) {
    picks.push(document.querySelector(selector));
  }
  for (const element of document.querySelectorAll(
    "button, a, [role='button'], input[type='button'], input[type='submit']",
  )) {
    const text = clean(
      element.textContent || element.value || element.getAttribute("aria-label") || "",
    );
    if (/\bdownload\b/i.test(text)) picks.push(element);
  }

  const element = picks.find(visible);
  if (!element) return { found: false, text: "" };

  element.setAttribute("data-live-e2e-download", "1");
  return {
    found: true,
    text: clean(element.textContent || element.value || element.getAttribute("aria-label") || ""),
    disabled: Boolean(element.disabled || element.getAttribute("aria-disabled") === "true"),
  };
}

function statusInstalled(status, regionId) {
  if (!status || typeof status !== "object") return false;
  const statusRegion = status.region ?? status.region_id ?? status.regionId;
  if (statusRegion && String(statusRegion).toLowerCase() !== String(regionId).toLowerCase()) {
    return false;
  }
  if (status.installed === true) return true;
  if (status.pack?.installed === true && status.graph?.installed === true) return true;
  if (status.pack?.phase === "installed" && status.graph?.phase === "installed") return true;
  const phase = String(status.phase ?? status.state ?? status.status ?? "").toLowerCase();
  if (["installed", "complete", "completed", "done"].includes(phase)) return true;
  if (
    Array.isArray(status.files) &&
    status.files.length > 0 &&
    status.files.every((file) => file?.installed === true)
  ) {
    return true;
  }
  if (Array.isArray(status.regions)) {
    const entry = status.regions.find(
      (region) =>
        String(region?.id ?? region?.region ?? "").toLowerCase() === String(regionId).toLowerCase(),
    );
    if (entry?.installed === true) return true;
  }
  return false;
}

function statusFailure(status, regionId) {
  if (!status || typeof status !== "object") return null;
  const statusRegion = status.region ?? status.region_id ?? status.regionId;
  if (statusRegion && String(statusRegion).toLowerCase() !== String(regionId).toLowerCase()) {
    return null;
  }
  const phases = [
    status.phase,
    status.state,
    status.status,
    status.pack?.phase,
    status.graph?.phase,
  ]
    .filter((value) => value !== undefined && value !== null)
    .map((value) => String(value).toLowerCase());
  const failed = phases.some((phase) =>
    ["failed", "error", "cancelled", "canceled"].includes(phase),
  );
  if (!failed) return null;
  const errors = [status.error, status.pack?.error, status.graph?.error].filter(Boolean);
  return errors.length > 0 ? errors.join("; ") : `phase ${phases.join("/")}`;
}

async function main() {
  let probe;
  try {
    probe = await fetchJson("/map/catalog");
  } catch (error) {
    const reason = error?.cause?.code || error?.name || "unreachable";
    skip(`backend ${BASE} is not reachable (${reason})`);
    skip("live regions E2E did not run");
    return;
  }

  check(probe.ok, "GET /map/catalog returns 200", `HTTP ${probe.status} ${probe.text}`);
  if (!probe.ok) return;
  check(Boolean(probe.body), "GET /map/catalog returns JSON", probe.text);
  if (!probe.body) return;

  const probeRegions = Array.isArray(probe.body.regions) ? probe.body.regions : [];
  const probeRegion = probeRegions.find(
    (region) => String(region?.id ?? "").toLowerCase() === REGION.toLowerCase(),
  );
  check(Boolean(probeRegion), `catalog lists ${REGION}`, `ids: ${probeRegions.map((r) => r?.id).join(", ") || "none"}`);
  if (!probeRegion) return;

  let regionName = String(probeRegion.name || REGION);

  let puppeteer;
  try {
    puppeteer = createRequire(import.meta.url)("puppeteer");
  } catch {
    skip("puppeteer is not installed; set NODE_PATH to a node_modules that contains it");
    skip("live regions E2E did not run");
    return;
  }

  let browser;
  try {
    browser = await puppeteer.launch({
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
  } catch (error) {
    skip(`puppeteer could not launch Chromium: ${error.message}`);
    skip("live regions E2E did not run");
    return;
  }
  ran = true;

  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900, deviceScaleFactor: 1 });
    page.setDefaultTimeout(45000);

    const pageErrors = [];
    const consoleErrors = [];
    const failedRequests = [];
    const externalRequests = [];
    let onlineEnabled = false;
    let lastPageWasMap = false;

    page.on("pageerror", (error) => pageErrors.push(error.message));
    page.on("console", (message) => {
      if (message.type() === "error") consoleErrors.push(message.text());
    });
    page.on("requestfailed", (request) => {
      const errorText = request.failure()?.errorText ?? "unknown";
      // MapLibre aborts in-flight tile reads when a style or map is replaced.
      if (errorText.includes("ERR_ABORTED")) return;
      // A failed OpenFreeMap request is an online-mode attempt with a dead
      // network; the external-request policy below still records it. A 404 or
      // bad style is surfaced by the page's own fallback, not as a harness
      // failure.
      try {
        if (isOpenFreeMapHost(new URL(request.url()).hostname)) return;
      } catch {
        // Not a parseable URL; keep the failure.
      }
      failedRequests.push(`${request.url()} (${errorText})`);
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
      if (parsed.origin === baseUrl.origin) return;
      externalRequests.push({ url, host: parsed.hostname, whenOnline: onlineEnabled });
    });

    async function openMapPage() {
      pageErrors.length = 0;
      consoleErrors.length = 0;
      failedRequests.length = 0;
      let response = null;
      try {
        response = await page.goto(MAP_URL, { waitUntil: "load", timeout: 45000 });
      } catch (error) {
        check(false, `map page ${MAP_URL} loads`, error.message);
        lastPageWasMap = false;
        return false;
      }
      check(
        Boolean(response) && response.status() < 400,
        `map page ${MAP_URL} loads`,
        `HTTP ${response?.status() ?? "unknown"}`,
      );
      if (!response || response.status() >= 400) {
        lastPageWasMap = false;
        return false;
      }

      try {
        await page.waitForFunction(
          () =>
            window.__offlineMapReady === true ||
            window.__pinpointMapReady === true ||
            Boolean(document.querySelector(".maplibregl-canvas, #map canvas")),
          { timeout: 45000, polling: 250 },
        );
      } catch {
        const diagnostic = await page
          .evaluate(() => ({
            offline: window.__offlineMapError ?? null,
            pinpoint: window.__pinpointMapError ?? null,
            status: document.querySelector("#status")?.dataset.state ?? null,
            text: document.querySelector("#status-text")?.textContent?.trim() ?? "",
          }))
          .catch(() => null);
        check(false, "map page reports ready or renders a canvas", JSON.stringify(diagnostic));
        lastPageWasMap = false;
        return false;
      }

      const mapError = await page
        .evaluate(() => window.__offlineMapError ?? window.__pinpointMapError ?? null)
        .catch(() => null);
      check(!mapError, "map page reports no initialization error", String(mapError ?? ""));
      lastPageWasMap = true;
      return true;
    }

    async function readCatalog() {
      const result = await page.evaluate(async () => {
        try {
          const response = await fetch("/map/catalog", {
            cache: "no-store",
            headers: { Accept: "application/json" },
          });
          const body = await response.json().catch(() => null);
          return { status: response.status, ok: response.ok, body };
        } catch (error) {
          return { status: 0, ok: false, body: null, error: String(error) };
        }
      });
      check(
        result.ok,
        "GET /map/catalog returns 200 from the map page",
        result.status ? `HTTP ${result.status}` : result.error,
      );
      if (!result.ok) return null;
      check(result.body?.schema === 1, "catalog schema is 1", String(result.body?.schema));
      const regions = Array.isArray(result.body?.regions) ? result.body.regions : [];
      const entry = regions.find(
        (region) => String(region?.id ?? "").toLowerCase() === REGION.toLowerCase(),
      );
      check(
        Boolean(entry),
        `catalog lists ${REGION}`,
        `ids: ${regions.map((region) => region?.id).join(", ") || "none"}`,
      );
      return entry ?? null;
    }

    async function readStatus() {
      return page.evaluate(async () => {
        try {
          const response = await fetch("/map/offline/status", {
            cache: "no-store",
            headers: { Accept: "application/json" },
          });
          const body = await response.json().catch(() => null);
          return { status: response.status, ok: response.ok, body };
        } catch (error) {
          return { status: 0, ok: false, body: null, error: String(error) };
        }
      });
    }

    async function readConfig() {
      return page.evaluate(async () => {
        try {
          const response = await fetch("/map/config", {
            cache: "no-store",
            headers: { Accept: "application/json" },
          });
          const body = await response.json().catch(() => null);
          return { status: response.status, ok: response.ok, body };
        } catch (error) {
          return { status: 0, ok: false, body: null, error: String(error) };
        }
      });
    }

    /**
     * Waits for the online toggle to settle. The page fetches the OpenFreeMap
     * style asynchronously (10s abort) and falls back to the local pack on
     * failure, so a click is not the final state.
     */
    async function waitOnlineSettled(expectedOn, timeoutMs) {
      const deadline = Date.now() + timeoutMs;
      let last = null;
      while (Date.now() < deadline) {
        last = await page.evaluate(onlineToggleInfo, false).catch(() => null);
        if (!last) return null;
        if (expectedOn) {
          if (last.active === true) return last;
          if (last.isOn === false) return last;
          if (last.active === null && last.isOn === true) return last;
        } else if (last.active === false && last.isOn !== true) {
          return last;
        } else if (last.active === null && last.isOn !== true) {
          return last;
        }
        await sleep(250);
      }
      return last;
    }

    async function assertConfig(regionName) {
      const config = await readConfig();
      check(
        config.ok,
        "GET /map/config returns 200",
        config.status ? `HTTP ${config.status}` : config.error,
      );
      if (!config.ok) return;

      const body = config.body ?? {};
      const pmtilesUrl = String(body.pmtilesUrl ?? body.pmtiles_url ?? "");
      let pathname = "";
      try {
        pathname = new URL(pmtilesUrl, BASE).pathname;
      } catch {
        pathname = "";
      }
      check(
        pathname.startsWith(`/packs/${REGION}/`),
        `/map/config points at /packs/${REGION}/`,
        pmtilesUrl || "pmtilesUrl missing",
      );

      if (Array.isArray(body.regions)) {
        const listed = body.regions.some((region) => {
          const id = String(region?.id ?? region?.region ?? "").toLowerCase();
          const name = String(region?.name ?? "").toLowerCase();
          return id === REGION.toLowerCase() || name === String(regionName).toLowerCase();
        });
        check(listed, `/map/config lists ${REGION}`, JSON.stringify(body.regions));
      } else {
        skip("/map/config has no regions array (single-region shape)");
      }
    }

    async function loginThroughApp() {
      lastPageWasMap = false;
      let response = null;
      try {
        response = await page.goto(APP_URL, { waitUntil: "load", timeout: 45000 });
      } catch (error) {
        check(false, `app page ${APP_URL} loads`, error.message);
        return false;
      }
      check(
        Boolean(response) && response.status() < 400,
        `app page ${APP_URL} loads`,
        `HTTP ${response?.status() ?? "unknown"}`,
      );
      if (!response || response.status() >= 400) return false;

      const alreadySignedIn = await page
        .evaluate(() => {
          const panel = document.querySelector("#login-panel");
          return !panel || panel.classList.contains("hidden");
        })
        .catch(() => false);
      if (alreadySignedIn) {
        check(true, "app session is already signed in");
        return true;
      }

      try {
        await page.waitForSelector("#login-email", { visible: true, timeout: 20000 });
        await page.waitForSelector("#login-password", { visible: true, timeout: 5000 });
      } catch {
        check(false, "app login form is visible");
        return false;
      }

      await page.type("#login-email", EMAIL);
      await page.type("#login-password", PASSWORD);
      const submitted = page
        .waitForFunction(
          () => {
            const login = document.querySelector("#login-panel");
            const app = document.querySelector("#app-panel");
            return (
              (login && login.classList.contains("hidden")) ||
              (app && !app.classList.contains("hidden")) ||
              Boolean(document.querySelector("#user-email")?.textContent?.trim())
            );
          },
          { timeout: 30000, polling: 250 },
        )
        .then(() => true)
        .catch(() => false);
      await page.click('#login-form button[type="submit"]');
      const signedIn = await submitted;
      const after = await page
        .evaluate(() => ({
          loginHidden: document.querySelector("#login-panel")?.classList.contains("hidden") ?? true,
          appVisible: (() => {
            const panel = document.querySelector("#app-panel");
            return Boolean(panel && !panel.classList.contains("hidden"));
          })(),
          userEmail: document.querySelector("#user-email")?.textContent?.trim() ?? "",
          status: document.querySelector("#status")?.textContent?.trim() ?? "",
        }))
        .catch(() => null);
      const ok =
        signedIn ||
        Boolean(after?.appVisible) ||
        Boolean(after?.loginHidden) ||
        Boolean(after?.userEmail);
      check(ok, "signed in through the app form", ok ? "" : after?.status ?? "");
      return ok;
    }

    async function downloadRegion(regionName) {
      if (SKIP_DOWNLOAD) {
        skip("region download: PINPOINT_SKIP_DOWNLOAD=1");
        const current = await readStatus();
        if (current.ok && statusInstalled(current.body, REGION)) {
          await assertConfig(regionName);
        } else {
          skip(`/map/config pack assertion skipped: ${REGION} is not installed`);
        }
        return;
      }

      const current = await readStatus();
      check(
        current.ok,
        "GET /map/offline/status returns 200",
        current.status ? `HTTP ${current.status}` : current.error,
      );
      if (!current.ok) return;
      if (statusInstalled(current.body, REGION)) {
        check(true, `${REGION} is already installed`);
        await assertConfig(regionName);
        return;
      }

      let trigger = null;
      for (let attempt = 0; attempt < 60; attempt += 1) {
        trigger = await page.evaluate(locateDownloadTrigger, REGION);
        if (trigger.found && !trigger.disabled) break;
        await sleep(500);
      }
      check(
        Boolean(trigger?.found) && trigger?.disabled !== true,
        `download control for ${REGION} is ready`,
        !trigger?.found ? "not found" : trigger.text || "disabled",
      );
      if (!trigger?.found || trigger.disabled) return;

      const downloadResponse = page
        .waitForResponse(
          (response) => {
            try {
              const url = new URL(response.url());
              return url.origin === baseUrl.origin && url.pathname === "/map/offline/download";
            } catch {
              return false;
            }
          },
          { timeout: 15000 },
        )
        .catch(() => null);

      let clicked = true;
      try {
        await page.click('[data-live-e2e-download="1"]');
      } catch (error) {
        clicked = false;
        check(false, "download control clicked", error.message);
      }
      const response = clicked ? await downloadResponse : null;
      await page
        .evaluate(() =>
          document
            .querySelectorAll("[data-live-e2e-download]")
            .forEach((element) => element.removeAttribute("data-live-e2e-download")),
        )
        .catch(() => {});
      if (!clicked) return;

      if (response) {
        const status = response.status();
        check(
          status === 202 || status === 409,
          "POST /map/offline/download accepted",
          `HTTP ${status}`,
        );
        if (status !== 202 && status !== 409) return;
      } else {
        skip("no /map/offline/download response observed; polling status");
      }

      const started = Date.now();
      let lastStatus = null;
      let outcome = "timeout";
      let detail = "";
      while (Date.now() - started < DOWNLOAD_TIMEOUT_MS) {
        let status;
        try {
          status = await readStatus();
        } catch (error) {
          outcome = "error";
          detail = `status read failed: ${error.message}`;
          break;
        }
        if (!status.ok) {
          outcome = "error";
          detail = status.status
            ? `GET /map/offline/status -> HTTP ${status.status}`
            : `status read failed: ${status.error}`;
          break;
        }
        lastStatus = status.body;
        if (statusInstalled(lastStatus, REGION)) {
          outcome = "installed";
          break;
        }
        const failure = statusFailure(lastStatus, REGION);
        if (failure) {
          outcome = "failed";
          detail = failure;
          break;
        }
        await sleep(POLL_MS);
      }

      check(
        outcome === "installed",
        `${REGION} installed within 15 minutes`,
        outcome === "installed"
          ? ""
          : detail ||
              `last status: ${JSON.stringify(lastStatus)?.slice(0, 400) ?? "none"}`,
      );
      if (outcome === "installed") await assertConfig(regionName);
    }

    const mapReady = await openMapPage();
    if (!mapReady) {
      skip("catalog, region row and online policy checks skipped: map page unavailable");
    } else {
      const entry = await readCatalog();
      regionName = entry?.name || regionName;

      let row = null;
      const rowDeadline = Date.now() + 15000;
      while (Date.now() < rowDeadline) {
        row = await page.evaluate(locateRegionRow, REGION, regionName);
        if (row?.found) break;
        await sleep(500);
      }
      check(row?.found === true, `catalog row for ${REGION} is visible`, row?.text ?? "not found");
      check(
        row?.found === true && row?.hasSize === true,
        `catalog row for ${REGION} shows a size`,
        row?.text ?? "not found",
      );

      const toggle = await page.evaluate(onlineToggleInfo, false).catch(() => null);
      if (!toggle) {
        skip("online toggle not found; no external request is allowed while it stays off");
      } else if (toggle.disabled) {
        skip(`online toggle is disabled (${toggle.text || toggle.id})`);
      } else {
        check(toggle.isOn !== true, "online basemap is off by default", JSON.stringify(toggle));
        if (toggle.isOn === true) {
          await page.evaluate(onlineToggleInfo, true).catch(() => {});
          const restored = await waitOnlineSettled(false, 15000);
          onlineEnabled = false;
          check(
            restored?.isOn !== true && restored?.active !== true,
            "online basemap is off after the default check",
            JSON.stringify(restored),
          );
        } else {
          onlineEnabled = true;
          const enabled = await page.evaluate(onlineToggleInfo, true).catch(() => null);
          onlineEnabled = Boolean(enabled);
          check(Boolean(enabled), "online toggle clicked");
          if (enabled) {
            const settled = await waitOnlineSettled(true, 15000);
            const openFreeMapRequests = externalRequests.filter(
              (request) => request.whenOnline && isOpenFreeMapHost(request.host),
            ).length;
            console.log(
              `INFO  online mode ${settled?.active === true ? "active" : "not active"}; ` +
                `${openFreeMapRequests} OpenFreeMap request(s)`,
            );
            if (settled?.isOn === true) {
              await page.evaluate(onlineToggleInfo, true).catch(() => {});
              const restored = await waitOnlineSettled(false, 15000);
              check(
                restored?.isOn !== true && restored?.active !== true,
                "online basemap returns to offline",
                JSON.stringify(restored),
              );
            }
            await sleep(500);
            onlineEnabled = false;
          }
        }
      }
    }

    const hasCredentials = Boolean(EMAIL && PASSWORD);
    if (!hasCredentials) {
      skip(
        EMAIL || PASSWORD
          ? "login/download: set both PINPOINT_EMAIL and PINPOINT_PASSWORD"
          : "login/download: PINPOINT_EMAIL and PINPOINT_PASSWORD are not set",
      );
    } else if (!mapReady) {
      skip("login/download: map page unavailable");
    } else {
      const signedIn = await loginThroughApp();
      if (!signedIn) {
        skip("region download: login failed");
      } else {
        const reopened = await openMapPage();
        if (!reopened) {
          skip("region download: map page unavailable after login");
        } else {
          await downloadRegion(regionName);
        }
      }
    }

    const offlineExternal = externalRequests.filter((request) => !request.whenOnline);
    check(
      offlineExternal.length === 0,
      "no external requests while online mode is off",
      summarize(offlineExternal),
    );
    const forbiddenOnline = externalRequests.filter(
      (request) => request.whenOnline && !isOpenFreeMapHost(request.host),
    );
    check(
      forbiddenOnline.length === 0,
      "online mode contacts only OpenFreeMap",
      summarize(forbiddenOnline),
    );

    if (lastPageWasMap) {
      check(failedRequests.length === 0, "no failed map page requests", summarize(failedRequests));
      check(pageErrors.length === 0, "no map page errors", summarize(pageErrors));
      if (consoleErrors.length > 0) {
        // Browsers log resource 404s (for example /favicon.ico) as console
        // errors; that is not part of the catalog/download contract.
        skip(`map console errors (not enforced): ${summarize(consoleErrors)}`);
      }
    } else {
      skip("map page request/console hygiene not checked: last page was the app form");
    }
  } finally {
    await browser.close();
  }
}

await main().catch((error) => {
  failures += 1;
  console.error(`FAIL  live regions E2E crashed — ${redact(error?.stack || error?.message || error)}`);
});

console.log("");
if (failures > 0) {
  console.error(`Live regions E2E failed: ${failures} check(s).`);
  process.exitCode = 1;
} else if (ran) {
  console.log(`Live regions E2E passed for ${REGION}.`);
  process.exitCode = 0;
} else {
  console.log("Live regions E2E skipped.");
  process.exitCode = 0;
}

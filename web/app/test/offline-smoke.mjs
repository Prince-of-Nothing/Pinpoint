/**
 * Offline pack manager smoke test.
 *
 * Static checks always run. When Puppeteer is available (or NODE_PATH points
 * at a node_modules that has it), the test opens `/app/offline-demo.html` in
 * headless Chromium and exercises the real path:
 *
 *   1. service worker registers, activates and claims the page;
 *   2. a corrupt manifest is rejected and leaves no installed pack;
 *   3. the Monaco fixture downloads with progress and SHA-256 verification;
 *   4. the installed PMTiles is served from OPFS with HTTP Range (206);
 *   5. MapLibre renders a non-zero canvas from `/offline-packs/...`;
 *   6. delete removes the pack, its metadata and the map;
 *   7. no external, failed, console or page errors occurred.
 *
 * Usage (PowerShell):
 *   $env:NODE_PATH="C:\path\to\node_modules"
 *   node web/app/test/offline-smoke.mjs
 *
 * Static-only fallback:
 *   node web/app/test/offline-smoke.mjs --static-only
 *
 * Screenshot path: $env:PINPOINT_OFFLINE_SCREENSHOT, otherwise
 * <system temp>/web-offline-packs.png.
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createStaticServer } from "../../map/serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(HERE, "../..");
const PACK_FIXTURE = path.join(WEB_ROOT, "app", "fixtures", "offline-pack", "manifest.json");
const BROKEN_FIXTURE = path.join(WEB_ROOT, "app", "fixtures", "offline-pack", "manifest-broken.json");
const LICENSE_FIXTURE = path.join(WEB_ROOT, "app", "fixtures", "offline-pack", "LICENSE.txt");
const PMTILES_FIXTURE = path.join(WEB_ROOT, "map", "fixtures", "monaco.pmtiles");
const PMTILES_BYTES = 94389;
const PMTILES_SHA256 = "a600dbfd9ddb25b82661f01d228a39d2b386dd13e401d2081b3ce7fd9c6de82f";
const LICENSE_BYTES = 405;
const LICENSE_SHA256 = "bf40327577f4ae509a121e90610b0d6dcad71d64a5e9a4ceac9d6d190933becf";
const TOTAL_BYTES = PMTILES_BYTES + LICENSE_BYTES;
const SCREENSHOT =
  process.env.PINPOINT_OFFLINE_SCREENSHOT || path.join(tmpdir(), "web-offline-packs.png");
const STATIC_ONLY = process.argv.includes("--static-only");
// Built at runtime so this guard file itself never contains the mojibake text.
const BROKEN_AC_ATTRIBUTION = "Ac" + " OpenStreetMap";
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

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function readUtf8(file) {
  const bytes = readFileSync(file);
  const noBom = !(bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf);
  let text = "";
  let valid = true;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    valid = false;
  }
  return { bytes, text, noBom, valid };
}

const server = createStaticServer({ root: WEB_ROOT });
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
  // Static assets and contract
  // -------------------------------------------------------------------------
  const assets = [
    ["/app/offline-demo.html", "text/html"],
    ["/app/offline.css", "text/css"],
    ["/app/offline.js", "text/javascript"],
    ["/app/offline-catalog.js", "text/javascript"],
    ["/sw.js", "text/javascript"],
    ["/app/fixtures/offline-pack/manifest.json", "application/json"],
    ["/app/fixtures/offline-pack/manifest-broken.json", "application/json"],
    ["/app/fixtures/offline-pack/LICENSE.txt", "text/plain"],
    ["/map/fixtures/monaco.pmtiles", "application/octet-stream"],
    ["/map/map.js", "text/javascript"],
    ["/map/style.json", "application/json"],
    ["/vendor/maplibre-gl.js", "text/javascript"],
    ["/vendor/maplibre-gl-worker.mjs", "text/javascript"],
    ["/vendor/maplibre-gl.css", "text/css"],
    ["/vendor/pmtiles.js", "text/javascript"],
  ];

  for (const [pathname, expectedType] of assets) {
    const response = await fetch(base + pathname);
    const type = response.headers.get("content-type") ?? "";
    check(response.status === 200, `HTTP 200 for ${pathname}`, `got ${response.status}`);
    check(type.startsWith(expectedType), `content type for ${pathname}`, `got ${type}`);
    await response.arrayBuffer();
  }

  const pmtiles = readFileSync(PMTILES_FIXTURE);
  check(pmtiles.length === PMTILES_BYTES, `Monaco fixture is ${PMTILES_BYTES} bytes`, `got ${pmtiles.length}`);
  check(sha256(pmtiles) === PMTILES_SHA256, "Monaco fixture SHA-256 matches the manifest", sha256(pmtiles));
  check(pmtiles.subarray(0, 7).toString("latin1") === "PMTiles", "Monaco fixture is a PMTiles archive");

  const license = readFileSync(LICENSE_FIXTURE);
  check(license.length === LICENSE_BYTES, `LICENSE.txt is ${LICENSE_BYTES} bytes`, `got ${license.length}`);
  check(sha256(license) === LICENSE_SHA256, "LICENSE.txt SHA-256 matches the manifest", sha256(license));

  const manifestText = readFileSync(PACK_FIXTURE, "utf8");
  const manifest = JSON.parse(manifestText);
  check(manifest.id === "monaco", "manifest id is monaco", String(manifest.id));
  check(manifest.version === "2026.11.16", "manifest version is pinned", String(manifest.version));
  check(Array.isArray(manifest.files) && manifest.files.length === 2, "manifest lists two files");
  check(
    manifest.files.reduce((sum, file) => sum + file.bytes, 0) === TOTAL_BYTES,
    `manifest total size is ${TOTAL_BYTES} bytes`,
  );

  const manifestPmtiles = manifest.files.find((file) => file.name === "monaco.pmtiles");
  const manifestLicense = manifest.files.find((file) => file.name === "LICENSE.txt");
  check(manifestPmtiles?.bytes === PMTILES_BYTES, "manifest PMTiles byte count is real");
  check(manifestPmtiles?.sha256 === PMTILES_SHA256, "manifest PMTiles SHA-256 is real");
  check(manifestLicense?.bytes === LICENSE_BYTES, "manifest LICENSE byte count is real");
  check(manifestLicense?.sha256 === LICENSE_SHA256, "manifest LICENSE SHA-256 is real");
  check(
    manifest.attribution === "© OpenStreetMap contributors",
    "manifest carries © OpenStreetMap contributors",
    String(manifest.attribution),
  );
  check(manifest.license === "ODbL-1.0", "manifest license is ODbL-1.0", String(manifest.license));
  check(
    manifest.license_url === "https://opendatacommons.org/licenses/odbl/1-0/",
    "manifest license URL is the ODbL deed",
  );

  const broken = JSON.parse(readFileSync(BROKEN_FIXTURE, "utf8"));
  check(broken.id === "monaco-broken", "broken fixture has its own id");
  check(
    broken.files[0].sha256 !== PMTILES_SHA256,
    "broken fixture intentionally mismatches the PMTiles SHA-256",
  );

  const demo = readFileSync(path.join(WEB_ROOT, "app", "offline-demo.html"), "utf8");
  check(demo.includes("../vendor/maplibre-gl.css"), "demo loads local MapLibre CSS");
  check(demo.includes("../vendor/pmtiles.js"), "demo loads local PMTiles JS");
  check(demo.includes('../map/map.js'), "demo uses the shared map module");
  check(demo.includes('./offline.js'), "demo imports the offline manager");
  check(
    demo.includes("© OpenStreetMap contributors") && demo.includes("ODbL"),
    "demo shows © OpenStreetMap contributors / ODbL",
  );
  check(demo.includes("/offline-packs/"), "demo documents the OPFS pack URL");
  check(demo.includes("monaco.pmtiles"), "demo opens monaco.pmtiles from the installed pack");

  const serviceWorker = readFileSync(path.join(WEB_ROOT, "sw.js"), "utf8");
  for (const token of [
    "offline-packs",
    "navigator.storage.getDirectory",
    "Accept-Ranges",
    "Content-Range",
    "application/vnd.pmtiles",
    "206",
  ]) {
    check(serviceWorker.includes(token), `service worker implements ${token}`);
  }
  check(serviceWorker.includes(".pmtiles"), "service worker maps .pmtiles content type");

  const offlineModule = readFileSync(path.join(WEB_ROOT, "app", "offline.js"), "utf8");
  for (const token of [
    "export async function listPacks",
    "export async function downloadPack",
    "export async function deletePack",
    "export async function openPack",
    "export function verifyManifest",
    "crypto.subtle.digest",
    "getDirectory",
  ]) {
    check(offlineModule.includes(token), `offline.js exposes ${token}`);
  }

  const catalog = readFileSync(path.join(WEB_ROOT, "app", "offline-catalog.js"), "utf8");
  for (const proxyPath of [
    "/offline/packs/moldova/manifest.json",
    "/offline/packs/moldova/moldova.pmtiles",
    "/offline/packs/moldova/moldova-search.sqlite",
    "/offline/packs/moldova/moldova-valhalla.tar",
  ]) {
    check(catalog.includes(proxyPath), `catalog declares same-origin ${proxyPath}`);
  }
  check(catalog.includes("approxBytes: 220_000_000"), "catalog documents the ~220 MB Moldova pack");
  check(catalog.includes("manual: true"), "catalog marks Moldova as a manual download");
  check(catalog.includes("id: \"monaco\""), "catalog keeps the Monaco fixture");
  check(
    !catalog.includes("releases/download") && !catalog.includes("github.com/Prince-of-Nothing"),
    "catalog contains no GitHub release URL",
  );
  check(
    !demo.includes("releases/download") && !demo.includes("github.com/Prince-of-Nothing"),
    "demo contains no GitHub release URL",
  );
  check(demo.includes("offline-catalog.js"), "demo imports the same-origin catalog");

  const newTextFiles = [
    "app/offline.js",
    "app/offline-catalog.js",
    "app/offline-demo.html",
    "app/offline.css",
    "app/README-offline.md",
    "app/fixtures/offline-pack/manifest.json",
    "app/fixtures/offline-pack/manifest-broken.json",
    "app/fixtures/offline-pack/LICENSE.txt",
    "sw.js",
  ];
  for (const relative of newTextFiles) {
    const { text, noBom, valid } = readUtf8(path.join(WEB_ROOT, relative));
    check(noBom, `${relative} is UTF-8 without BOM`);
    check(valid, `${relative} is valid UTF-8`);
    check(
      !text.includes(BROKEN_AC_ATTRIBUTION) && !text.includes("\uFFFD") && !text.includes("\u00C2\u00A9"),
      `${relative} has no attribution mojibake`,
    );
  }

  for (const relative of newTextFiles.filter((file) => !file.endsWith(".pmtiles"))) {
    const lower = readFileSync(path.join(WEB_ROOT, relative), "utf8").toLowerCase();
    for (const forbidden of ["unpkg.com", "jsdelivr", "cdnjs", "tile.openstreetmap.org"]) {
      check(!lower.includes(forbidden), `${relative} does not reference ${forbidden}`);
    }
  }

  // -------------------------------------------------------------------------
  // Browser
  // -------------------------------------------------------------------------
  let puppeteer = null;
  if (!STATIC_ONLY) {
    try {
      puppeteer = require("puppeteer");
    } catch {
      console.log("SKIP  browser: puppeteer is not installed (set NODE_PATH or run --static-only)");
      console.log("SKIP  static checks only; OPFS, service worker and MapLibre were not exercised");
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

      await page.goto(`${base}/app/offline-demo.html`, { waitUntil: "load", timeout: 45000 });
      await page.waitForFunction(() => window.__offlineDemo?.ready === true, { timeout: 30000 });
      await page.waitForFunction(() => window.__offlineDemo?.swReady === true, { timeout: 15000 });

      const initial = await page.evaluate(() => ({
        swReady: window.__offlineDemo.swReady,
        packs: window.__offlineDemo.packs.length,
        manifestName: document.querySelector("#manifest-name")?.textContent?.trim() ?? "",
        manifestVersion: document.querySelector("#manifest-version")?.textContent?.trim() ?? "",
        manifestSize: document.querySelector("#manifest-size")?.textContent?.trim() ?? "",
        manifestFiles: document.querySelector("#manifest-files")?.textContent?.trim() ?? "",
        attribution:
          document.querySelector("#manifest-attribution")?.textContent?.replace(/\s+/g, " ").trim() ?? "",
        downloadDisabled: document.querySelector("#download")?.disabled ?? true,
        downloadText: document.querySelector("#download")?.textContent?.trim() ?? "",
        emptyHidden: document.querySelector("#packs-empty")?.hidden ?? true,
        packageUrl: window.__offlineDemo.packs[0]?.pmtilesUrl ?? null,
        selectedId: window.__offlineDemo.selected?.id ?? null,
        catalog: window.__offlineDemo.catalog.map((entry) => ({
          id: entry.id,
          manifestUrl: entry.manifestUrl,
          manual: entry.manual,
          approxBytes: entry.approxBytes,
        })),
        catalogRows: [...document.querySelectorAll("#catalog [data-catalog-id]")].map(
          (row) => row.dataset.catalogId,
        ),
        catalogNote:
          document.querySelector(".catalog-note")?.textContent?.replace(/\s+/g, " ").trim() ?? "",
      }));

      check(initial.swReady === true, "service worker is active and controlling the page");
      check(initial.packs === 0, "no packs are installed initially", String(initial.packs));
      check(initial.emptyHidden === false, "empty installed state is visible");
      check(initial.manifestName === "Monaco", "manifest name is shown", initial.manifestName);
      check(initial.manifestVersion === "v2026.11.16", "manifest version is shown", initial.manifestVersion);
      check(initial.manifestSize.includes("KB"), "manifest download size is shown", initial.manifestSize);
      check(initial.manifestFiles === "2 files", "manifest file count is shown", initial.manifestFiles);
      check(
        initial.attribution.includes("© OpenStreetMap contributors") &&
          initial.attribution.includes("ODbL-1.0"),
        "manifest shows © OpenStreetMap contributors / ODbL-1.0",
        initial.attribution,
      );
      check(initial.downloadDisabled === false, "download button is enabled after the manifest loads");
      check(
        initial.downloadText === "Download Monaco fixture",
        "download button names the selected fixture",
        initial.downloadText,
      );
      check(initial.selectedId === "monaco", "Monaco fixture is selected by default");
      check(
        initial.catalog.length === 2 && initial.catalogRows.join(",") === "monaco,moldova",
        "catalog lists Monaco and Moldova",
        JSON.stringify({ catalog: initial.catalog, rows: initial.catalogRows }),
      );
      const moldovaEntry = initial.catalog.find((entry) => entry.id === "moldova");
      check(
        moldovaEntry?.manifestUrl === "/offline/packs/moldova/manifest.json",
        "Moldova uses the same-origin proxy manifest",
        moldovaEntry?.manifestUrl ?? "missing",
      );
      check(moldovaEntry?.manual === true, "Moldova is marked as a manual download");
      check(
        moldovaEntry?.approxBytes === 220_000_000,
        "Moldova shows the ~220 MB size before download",
      );
      check(
        initial.catalogNote.includes("/offline/packs/moldova/") &&
          initial.catalogNote.includes("CORS"),
        "catalog explains the same-origin proxy and the CORS constraint",
        initial.catalogNote,
      );

      // ---------------------------------------------------------------------
      // Manifest validation and corrupt-pack rejection
      // ---------------------------------------------------------------------
      const validation = await page.evaluate(async () => {
        const mod = await import("/app/offline.js");
        const valid = {
          id: "monaco",
          version: "1",
          attribution: "© OpenStreetMap contributors",
          license: "ODbL-1.0",
          license_url: "https://opendatacommons.org/licenses/odbl/1-0/",
          files: [
            {
              name: "monaco.pmtiles",
              url: "monaco.pmtiles",
              bytes: 1,
              sha256: "0".repeat(64),
            },
          ],
        };
        const codes = [];
        const badManifests = [
          { ...valid, id: "../monaco" },
          { ...valid, files: [{ ...valid.files[0], name: "../../monaco.pmtiles" }] },
          { ...valid, files: [{ ...valid.files[0], sha256: "abc" }] },
        ];
        for (const candidate of badManifests) {
          try {
            mod.verifyManifest(candidate);
            codes.push("accepted");
          } catch (error) {
            codes.push(error.code ?? error.name);
          }
        }
        return { codes };
      });
      check(
        validation.codes.length === 3 && validation.codes.every((code) => code === "MANIFEST_INVALID"),
        "verifyManifest rejects path traversal, bad ids and bad hashes",
        JSON.stringify(validation.codes),
      );

      const corrupt = await page.evaluate(async (manifestUrl) => {
        const mod = await import("/app/offline.js");
        try {
          await mod.downloadPack(manifestUrl);
          return { installed: true, code: null, message: null };
        } catch (error) {
          return { installed: false, code: error.code ?? error.name, message: error.message };
        }
      }, `${base}/app/fixtures/offline-pack/manifest-broken.json`);
      check(corrupt.installed === false, "corrupt pack download is rejected");
      check(corrupt.code === "HASH_MISMATCH", "corrupt pack fails SHA-256 verification", corrupt.code ?? "");

      const afterCorrupt = await page.evaluate(async () => {
        const mod = await import("/app/offline.js");
        const root = await navigator.storage.getDirectory();
        let names = [];
        try {
          const packs = await root.getDirectoryHandle("offline-packs");
          for await (const [name] of packs.entries()) names.push(name);
        } catch {
          names = [];
        }
        const installedPacks = await mod.listPacks();
        return { names, packs: installedPacks };
      });
      check(
        !afterCorrupt.names.includes("monaco-broken"),
        "corrupt pack has no installed directory",
        afterCorrupt.names.join(", "),
      );
      check(
        !afterCorrupt.names.some((name) => name.startsWith(".staging-")),
        "corrupt pack leaves no staging directory",
        afterCorrupt.names.join(", "),
      );
      check(afterCorrupt.packs.length === 0, "corrupt pack is not listed as installed");

      // ---------------------------------------------------------------------
      // Real download, progress and OPFS contents
      // ---------------------------------------------------------------------
      await page.click("#download");
      await page.waitForFunction(
        () => window.__offlineDemo?.lastInstall?.id === "monaco",
        { timeout: 45000 },
      );

      const installed = await page.evaluate(() => ({
        lastInstall: window.__offlineDemo.lastInstall,
        lastError: window.__offlineDemo.lastError,
        progress: window.__offlineDemo.progressHistory,
        packs: window.__offlineDemo.packs,
      }));

      check(installed.lastError === null, "download completed without an error", installed.lastError ?? "");
      check(installed.lastInstall?.files?.includes("monaco.pmtiles"), "installed pack contains monaco.pmtiles");
      check(installed.lastInstall?.files?.includes("LICENSE.txt"), "installed pack contains LICENSE.txt");
      check(
        installed.lastInstall?.totalBytes === TOTAL_BYTES,
        `installed pack reports ${TOTAL_BYTES} bytes`,
        String(installed.lastInstall?.totalBytes),
      );
      check(
        installed.lastInstall?.pmtilesUrl === "/offline-packs/monaco/monaco.pmtiles",
        "installed pack exposes /offline-packs/monaco/monaco.pmtiles",
        installed.lastInstall?.pmtilesUrl ?? "missing",
      );
      check(installed.packs.length === 1, "installed pack is listed", String(installed.packs.length));
      check(
        installed.progress.some((entry) => entry.phase === "download"),
        "progress reported a download phase",
        JSON.stringify(installed.progress.slice(0, 6)),
      );
      check(
        installed.progress.some((entry) => entry.phase === "verify"),
        "progress reported a SHA-256 verification phase",
      );
      check(
        installed.progress.some((entry) => entry.phase === "done" && entry.percent >= 99),
        "progress reached 100%",
        JSON.stringify(installed.progress.slice(-3)),
      );

      const opfs = await page.evaluate(async () => {
        const root = await navigator.storage.getDirectory();
        const packs = await root.getDirectoryHandle("offline-packs");
        const names = [];
        for await (const [name] of packs.entries()) names.push(name);

        const packDir = await packs.getDirectoryHandle("monaco");
        const metadata = JSON.parse(
          await (await (await packDir.getFileHandle("pack.json")).getFile()).text(),
        );
        const filesDir = await packDir.getDirectoryHandle("files");
        const files = [];
        for await (const [name, handle] of filesDir.entries()) {
          const file = await handle.getFile();
          const bytes = new Uint8Array(await file.arrayBuffer());
          const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
          const hex = Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
          files.push({ name, size: file.size, sha256: hex });
        }

        return { names, metadata, files };
      });

      check(opfs.names.includes("monaco"), "OPFS contains the monaco directory", opfs.names.join(", "));
      check(!opfs.names.some((name) => name.startsWith(".")), "OPFS has no staging leftovers", opfs.names.join(", "));
      check(opfs.metadata.id === "monaco", "OPFS metadata id is monaco");
      check(opfs.metadata.version === "2026.11.16", "OPFS metadata version is pinned");
      check(opfs.metadata.totalBytes === TOTAL_BYTES, "OPFS metadata records the total size");
      check(
        opfs.metadata.attribution === "© OpenStreetMap contributors",
        "OPFS metadata keeps the ODbL attribution",
      );
      check(
        opfs.files.some((file) => file.name === "monaco.pmtiles" && file.size === PMTILES_BYTES),
        "OPFS stores monaco.pmtiles with the expected size",
      );
      check(
        opfs.files.some((file) => file.name === "monaco.pmtiles" && file.sha256 === PMTILES_SHA256),
        "OPFS stores monaco.pmtiles with the expected SHA-256",
      );
      check(
        opfs.files.some((file) => file.name === "LICENSE.txt" && file.size === LICENSE_BYTES),
        "OPFS stores LICENSE.txt with the expected size",
      );

      // ---------------------------------------------------------------------
      // Map from the installed pack and HTTP Range from OPFS
      // ---------------------------------------------------------------------
      await page.waitForFunction(
        () =>
          window.__offlineMapReady === true ||
          typeof window.__offlineMapError === "string",
        { timeout: 45000 },
      );
      await page
        .waitForFunction(
          () => {
            const map = window.__offlineDemo?.map?.getMap?.();
            try {
              return Boolean(map?.isSourceLoaded("basemap") && map?.loaded());
            } catch {
              return false;
            }
          },
          { timeout: 15000 },
        )
        .catch(() => {});
      await page.evaluate(
        () =>
          new Promise((resolve) => {
            requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 250)));
          }),
      );

      const mapInfo = await page.evaluate(() => {
        const handle = window.__offlineDemo.map;
        const map = handle?.getMap?.() ?? null;
        const canvas = document.querySelector(".maplibregl-canvas");
        return {
          ready: window.__offlineMapReady === true,
          error: window.__offlineMapError ?? null,
          canvas: canvas
            ? {
                width: canvas.width,
                height: canvas.height,
                clientWidth: canvas.clientWidth,
                clientHeight: canvas.clientHeight,
              }
            : null,
          sourceUrl: map?.getStyle()?.sources?.basemap?.url ?? null,
          sourceLoaded: (() => {
            try {
              return Boolean(map?.isSourceLoaded("basemap"));
            } catch {
              return false;
            }
          })(),
          renderedFeatures: (() => {
            try {
              return map?.queryRenderedFeatures?.().length ?? -1;
            } catch {
              return -1;
            }
          })(),
          layerCount: map?.getStyle()?.layers?.length ?? 0,
          status: document.querySelector("#map-status")?.dataset.state ?? null,
          statusText: document.querySelector("#map-status-text")?.textContent?.trim() ?? "",
          attribution:
            document.querySelector("#attribution")?.textContent?.replace(/\s+/g, " ").trim() ?? "",
          emptyHidden: document.querySelector("#map-empty")?.hidden ?? false,
        };
      });

      check(mapInfo.error === null, "no map initialization error", mapInfo.error ?? "");
      check(mapInfo.ready === true, "MapLibre load event fired from the OPFS pack");
      check(mapInfo.status === "ready", "map status is ready", mapInfo.status ?? "missing");
      check(mapInfo.emptyHidden === true, "map empty state is hidden");
      check(Boolean(mapInfo.canvas), "MapLibre canvas exists");
      check(
        Boolean(mapInfo.canvas && mapInfo.canvas.width > 0 && mapInfo.canvas.height > 0),
        "MapLibre canvas has a non-zero backing size",
        JSON.stringify(mapInfo.canvas),
      );
      check(
        Boolean(mapInfo.canvas && mapInfo.canvas.clientHeight > 500),
        "MapLibre canvas fills the map panel",
        JSON.stringify(mapInfo.canvas),
      );
      check(
        mapInfo.renderedFeatures > 0,
        "MapLibre rendered vector features from the pack",
        String(mapInfo.renderedFeatures),
      );
      check(
        mapInfo.sourceUrl === "pmtiles:///offline-packs/monaco/monaco.pmtiles",
        "basemap source is bound to /offline-packs/monaco/monaco.pmtiles",
        mapInfo.sourceUrl ?? "missing",
      );
      check(mapInfo.sourceLoaded === true, "PMTiles basemap source is loaded");
      check(mapInfo.layerCount > 0, "style layers are present", String(mapInfo.layerCount));
      check(
        mapInfo.attribution.includes("© OpenStreetMap contributors") && mapInfo.attribution.includes("ODbL"),
        "visible attribution contains © OpenStreetMap contributors / ODbL",
        mapInfo.attribution,
      );
      check(
        !mapInfo.attribution.includes(BROKEN_AC_ATTRIBUTION) &&
          !mapInfo.attribution.includes("\uFFFD") &&
          !mapInfo.attribution.includes("\u00C2\u00A9"),
        "visible attribution has no mojibake",
        mapInfo.attribution,
      );

      const range = await page.evaluate(async () => {
        const pack = window.__offlineDemo.packs[0];
        const response = await fetch(pack.pmtilesUrl, {
          headers: { Range: "bytes=0-126" },
        });
        const bytes = new Uint8Array(await response.arrayBuffer());
        return {
          status: response.status,
          acceptRanges: response.headers.get("accept-ranges"),
          contentRange: response.headers.get("content-range"),
          contentType: response.headers.get("content-type"),
          contentLength: response.headers.get("content-length"),
          length: bytes.length,
          magic: String.fromCharCode(...bytes.subarray(0, 7)),
        };
      });

      check(range.status === 206, "service worker returns HTTP 206 for Range", String(range.status));
      check(range.acceptRanges === "bytes", "service worker advertises Accept-Ranges: bytes", range.acceptRanges ?? "");
      check(
        range.contentRange === `bytes 0-126/${PMTILES_BYTES}`,
        "service worker sets the correct Content-Range",
        range.contentRange ?? "missing",
      );
      check(
        String(range.contentType).startsWith("application/vnd.pmtiles"),
        "service worker sets the PMTiles content type",
        range.contentType ?? "missing",
      );
      check(range.contentLength === "127", "service worker sets the range Content-Length", range.contentLength ?? "");
      check(
        range.length === 127 && range.magic === "PMTiles",
        "range response starts with the PMTiles magic bytes",
        `${range.length} / ${range.magic}`,
      );

      await page.waitForFunction(async () => {
        const stats = await window.__offlineDemo.swStats();
        return Boolean(stats && stats.pack206 > 0);
      }, { timeout: 15000 });

      const swStats = await page.evaluate(() => window.__offlineDemo.swStats());
      check(swStats !== null, "service worker reports diagnostics");
      check(swStats.packRequests > 0, "service worker served pack requests", String(swStats?.packRequests));
      check(swStats.pack206 > 0, "service worker served at least one 206 response", String(swStats?.pack206));
      check(swStats.pack404 === 0, "service worker had no missing pack files", String(swStats?.pack404));

      await page.screenshot({ path: SCREENSHOT });
      const screenshotOk = existsSync(SCREENSHOT) && statSync(SCREENSHOT).size > 10000;
      check(screenshotOk, `screenshot saved (${SCREENSHOT})`, "file missing or too small");

      // ---------------------------------------------------------------------
      // Update: replace the installed pack atomically and reopen the map
      // ---------------------------------------------------------------------
      await page.click('#packs [data-pack-id="monaco"] [data-action="update"]');
      await page.waitForFunction(
        () => window.__offlineDemo?.lastInstall?.updated === true,
        { timeout: 45000 },
      );
      await page.waitForFunction(
        () => window.__offlineMapReady === true || typeof window.__offlineMapError === "string",
        { timeout: 45000 },
      );

      const updated = await page.evaluate(async () => {
        const root = await navigator.storage.getDirectory();
        const packsDir = await root.getDirectoryHandle("offline-packs");
        const names = [];
        for await (const [name] of packsDir.entries()) names.push(name);
        return {
          lastInstall: window.__offlineDemo.lastInstall,
          packs: window.__offlineDemo.packs,
          mapReady: window.__offlineMapReady,
          mapError: window.__offlineMapError ?? null,
          names,
        };
      });

      check(updated.lastInstall?.updated === true, "update replaces the existing pack");
      check(updated.packs.length === 1, "update leaves exactly one installed pack");
      check(updated.packs[0]?.version === "2026.11.16", "updated pack keeps its version");
      check(updated.mapReady === true, "map reopens from the updated pack", updated.mapError ?? "");
      check(
        !updated.names.some((name) => name.startsWith(".staging-") || name.startsWith(".old-")),
        "update leaves no staging or backup directory",
        updated.names.join(", "),
      );

      // ---------------------------------------------------------------------
      // Delete: map, metadata and OPFS directory
      // ---------------------------------------------------------------------
      await page.click('#packs [data-pack-id="monaco"] [data-action="delete"]');
      await page.waitForFunction(
        () => window.__offlineDemo?.lastDelete?.removed === true && window.__offlineDemo.packs.length === 0,
        { timeout: 15000 },
      );

      const afterDelete = await page.evaluate(async () => {
        const root = await navigator.storage.getDirectory();
        const packs = await root.getDirectoryHandle("offline-packs");
        const names = [];
        for await (const [name] of packs.entries()) names.push(name);

        let metadataPresent = true;
        try {
          const packDir = await packs.getDirectoryHandle("monaco");
          await packDir.getFileHandle("pack.json");
        } catch {
          metadataPresent = false;
        }

        return {
          names,
          metadataPresent,
          packs: window.__offlineDemo.packs,
          lastDelete: window.__offlineDemo.lastDelete,
          mapReady: window.__offlineMapReady,
          canvas: Boolean(document.querySelector(".maplibregl-canvas")),
          emptyHidden: document.querySelector("#map-empty")?.hidden ?? true,
          listText: document.querySelector("#packs")?.textContent?.trim() ?? "",
        };
      });

      check(afterDelete.lastDelete?.removed === true, "deletePack reported removal");
      check(afterDelete.packs.length === 0, "installed list is empty after delete");
      check(afterDelete.listText === "", "installed list has no row text after delete");
      check(!afterDelete.names.includes("monaco"), "OPFS monaco directory is gone", afterDelete.names.join(", "));
      check(!afterDelete.metadataPresent, "OPFS pack.json metadata is gone");
      check(!afterDelete.names.some((name) => name.startsWith(".staging-")), "no staging directory remains");
      check(afterDelete.mapReady === false, "map handle was reset after delete");
      check(afterDelete.canvas === false, "MapLibre canvas was removed after delete");
      check(afterDelete.emptyHidden === false, "map empty state is visible after delete");

      // ---------------------------------------------------------------------
      // Network and console hygiene
      // ---------------------------------------------------------------------
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
      ? `Offline pack checks passed. Screenshot: ${SCREENSHOT}`
      : "Offline pack checks passed (static only).",
  );
  process.exitCode = 0;
} else {
  console.error(`Offline pack checks failed: ${failures}`);
  process.exitCode = 1;
}

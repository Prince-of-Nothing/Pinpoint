/**
 * Smoke test for the Pinpoint offline SQLite search module.
 *
 * The test serves `web/` on a random loopback port, verifies the pinned
 * SQLite/WASM assets and the fixture over HTTP, then exercises
 * `web/map/search.js`. When Puppeteer is available it runs in headless
 * Chromium (the real browser path); otherwise it falls back to Node's fetch +
 * the same module so the assertions still run in CI.
 *
 * Usage:
 *   node web/map/test/search-smoke.mjs
 *
 * With the Puppeteer installation in a separate node_modules:
 *   $env:NODE_PATH="C:\path\to\node_modules"
 *   node web/map/test/search-smoke.mjs
 *
 * Force the Node fallback:
 *   node web/map/test/search-smoke.mjs --no-browser
 *
 * Every request is to the local test server; the browser path also asserts
 * that no request left the server origin.
 */

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createStaticServer } from "../serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(HERE, "../..");
const SEARCH_MODULE = path.join(WEB_ROOT, "map", "search.js");
const FIXTURE = path.join(WEB_ROOT, "map", "fixtures", "balti-search.sqlite");
const FIXTURE_BYTES = 24576;
const FIXTURE_SHA256 = "118c826db8aec4207b1bb8aa909dd5d45eaa2d4172d95e9ab64ecf6afba93772";
const NO_BROWSER = process.argv.includes("--no-browser");
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

function printResults(session) {
  for (const [label, value] of Object.entries(session.results)) {
    console.log(`search(${JSON.stringify(label)}) -> ${JSON.stringify(value)}`);
  }
  console.log(`search after close -> ${JSON.stringify(session.closedError)}`);
  console.log(`openSearch(cross-origin) -> ${JSON.stringify(session.crossOriginError)}`);
}

function assertResults(session) {
  const { results, closedError } = session;

  check(Array.isArray(results.balti), "balti returns an array");
  check(results.balti.length === 1, "balti finds exactly one place", JSON.stringify(results.balti));
  check(results.balti[0]?.name === "Bălți", "balti finds Bălți", results.balti[0]?.name ?? "missing");
  check(results.balti[0]?.kind === "city", "balti result kind is city", results.balti[0]?.kind ?? "missing");
  check(
    Math.abs(Number(results.balti[0]?.lat) - 47.76) < 1e-9 &&
      Math.abs(Number(results.balti[0]?.lon) - 27.93) < 1e-9,
    "balti result carries the Bălți coordinates",
    JSON.stringify(results.balti[0]),
  );
  check(results.balti[0]?.address === null, "balti result address is null", String(results.balti[0]?.address));
  check(
    Math.abs(Number(results.balti[0]?.importance) - 0.9) < 1e-9,
    "balti result carries importance 0.9",
    String(results.balti[0]?.importance),
  );

  check(results.kishin.length === 1, "кишин finds exactly one place", JSON.stringify(results.kishin));
  check(
    results.kishin[0]?.name === "Кишинёв",
    "кишин finds Кишинёв",
    results.kishin[0]?.name ?? "missing",
  );
  check(
    Math.abs(Number(results.kishin[0]?.lat) - 47.01) < 1e-9 &&
      Math.abs(Number(results.kishin[0]?.lon) - 28.86) < 1e-9,
    "кишин result carries the Кишинёв coordinates",
    JSON.stringify(results.kishin[0]),
  );

  check(
    results.diacritics.length === 1 && results.diacritics[0]?.name === "Bălți",
    "diacritics query Bălți is folded",
    JSON.stringify(results.diacritics),
  );
  check(
    results.punctuated.length === 1 && results.punctuated[0]?.name === "Bălți",
    "punctuation around a word is a separator",
    JSON.stringify(results.punctuated),
  );

  for (const label of ["punctuation", "empty", "whitespace", "emoji"]) {
    check(
      Array.isArray(results[label]) && results[label].length === 0,
      `${label} query returns []`,
      JSON.stringify(results[label]),
    );
  }

  check(
    results.limitOne.length === 1 && results.limitOne[0]?.name === "Bălți",
    "limit 1 returns the top place",
    JSON.stringify(results.limitOne),
  );
  check(
    typeof closedError === "string" && closedError.includes("closed"),
    "search after close throws a closed-handle error",
    String(closedError),
  );
  check(
    typeof session.crossOriginError === "string" &&
      session.crossOriginError.includes("same-origin"),
    "openSearch rejects a cross-origin SQLite URL",
    String(session.crossOriginError),
  );

  for (const [name, value] of Object.entries(results)) {
    check(Array.isArray(value), `${name} result is JSON-serialisable array`);
  }
}

async function runSearchSession(openSearch, base) {
  const search = await openSearch({
    sqliteUrl: `${base}/map/fixtures/balti-search.sqlite`,
    wasmUrl: `${base}/vendor/sqlite3.wasm`,
  });

  const results = {
    balti: search.search("balti"),
    kishin: search.search("кишин"),
    diacritics: search.search("Bălți"),
    punctuated: search.search("  Bălți!!!  "),
    punctuation: search.search("!!! ??? --- ..."),
    empty: search.search(""),
    whitespace: search.search("   \t\n  "),
    emoji: search.search("🚀"),
    limitOne: search.search("balti", 1),
  };

  search.close();
  search.close();

  let closedError = null;
  try {
    search.search("balti");
  } catch (error) {
    closedError = String(error?.message ?? error);
  }

  let crossOriginError = null;
  try {
    await openSearch({ sqliteUrl: "https://example.com/places.sqlite" });
  } catch (error) {
    crossOriginError = String(error?.message ?? error);
  }

  return { results, closedError, crossOriginError };
}

async function runNodeFallback(base) {
  const previousDocument = globalThis.document;
  const previousLocation = globalThis.location;
  const previousFetch = globalThis.fetch;
  const previousLocalStorage = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  const externalFetches = [];

  globalThis.document = { baseURI: `${base}/privacy.html` };
  globalThis.location = new URL(`${base}/privacy.html`);
  // Node 26 exposes a lazy localStorage getter that warns when Emscripten
  // probes it. A tiny stub keeps the fallback output clean; it is never used
  // by the search module.
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      getItem: () => null,
      setItem: () => {},
      removeItem: () => {},
      clear: () => {},
      key: () => null,
      length: 0,
    },
  });
  globalThis.fetch = (input, init) => {
    const raw = input instanceof URL ? input.href : typeof input === "string" ? input : input?.url;
    const url = new URL(String(raw), base);
    if (url.origin !== base) {
      externalFetches.push(url.href);
      return Promise.reject(new Error(`external fetch blocked: ${url.href}`));
    }
    return previousFetch(input, init);
  };

  try {
    const module = await import(pathToFileURL(SEARCH_MODULE).href);
    const session = await runSearchSession(module.openSearch, base);
    check(externalFetches.length === 0, "Node fallback made no external requests", externalFetches.join(", "));
    return session;
  } finally {
    globalThis.fetch = previousFetch;
    if (previousDocument === undefined) delete globalThis.document;
    else globalThis.document = previousDocument;
    if (previousLocation === undefined) delete globalThis.location;
    else globalThis.location = previousLocation;
    if (previousLocalStorage) Object.defineProperty(globalThis, "localStorage", previousLocalStorage);
    else delete globalThis.localStorage;
  }
}

async function runBrowser(base) {
  const puppeteer = require("puppeteer");
  browserRan = true;

  const browser = await puppeteer.launch({
    headless: true,
    args: ["--no-sandbox", "--disable-setuid-sandbox", "--disable-dev-shm-usage"],
  });

  try {
    console.log(`browser: ${await browser.version()}`);
    const page = await browser.newPage();
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

    // Keep the console clean: a bare page requests /favicon.ico, which is not
    // part of this test and otherwise produces a 404 console error.
    await page.setRequestInterception(true);
    page.on("request", (request) => {
      const handler = request.url().endsWith("/favicon.ico")
        ? request.respond({ status: 204 })
        : request.continue();
      handler.catch(() => {});
    });

    await page.goto(`${base}/privacy.html`, { waitUntil: "domcontentloaded" });

    const session = await page.evaluate(async (baseUrl) => {
      const { openSearch } = await import("/map/search.js");
      const search = await openSearch({
        sqliteUrl: `${baseUrl}/map/fixtures/balti-search.sqlite`,
        wasmUrl: `${baseUrl}/vendor/sqlite3.wasm`,
      });

      const results = {
        balti: search.search("balti"),
        kishin: search.search("кишин"),
        diacritics: search.search("Bălți"),
        punctuated: search.search("  Bălți!!!  "),
        punctuation: search.search("!!! ??? --- ..."),
        empty: search.search(""),
        whitespace: search.search("   \t\n  "),
        emoji: search.search("🚀"),
        limitOne: search.search("balti", 1),
      };

      search.close();
      search.close();

      let closedError = null;
      try {
        search.search("balti");
      } catch (error) {
        closedError = String(error?.message ?? error);
      }

      let crossOriginError = null;
      try {
        await openSearch({ sqliteUrl: "https://example.com/places.sqlite" });
      } catch (error) {
        crossOriginError = String(error?.message ?? error);
      }

      return { results, closedError, crossOriginError };
    }, base);

    check(externalRequests.length === 0, "no external network requests", externalRequests.join(", "));
    check(failedRequests.length === 0, "no failed network requests", failedRequests.join(", "));
    check(pageErrors.length === 0, "no page errors", pageErrors.join(" | "));
    check(consoleErrors.length === 0, "no console errors", consoleErrors.join(" | "));

    return session;
  } finally {
    await browser.close();
  }
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
  // Static assets and fixture
  // -------------------------------------------------------------------------
  const assets = [
    ["/map/search.js", "text/javascript"],
    ["/map/search.md", "text/plain"],
    ["/map/fixtures/balti-search.sqlite", "application/vnd.sqlite3"],
    ["/map/fixtures/README.md", "text/plain"],
    ["/vendor/sqlite3.mjs", "text/javascript"],
    ["/vendor/sqlite3.wasm", "application/wasm"],
    ["/vendor/sqlite3.LICENSE.txt", "text/plain"],
  ];

  for (const [pathname, expectedType] of assets) {
    const response = await fetch(base + pathname);
    const type = response.headers.get("content-type") ?? "";
    check(response.status === 200, `HTTP 200 for ${pathname}`, `got ${response.status}`);
    check(type.startsWith(expectedType), `content type for ${pathname}`, `got ${type}`);
    await response.arrayBuffer();
  }

  const fixture = readFileSync(FIXTURE);
  const fixtureHash = createHash("sha256").update(fixture).digest("hex");
  check(fixture.length === FIXTURE_BYTES, `fixture is ${FIXTURE_BYTES} bytes`, `got ${fixture.length}`);
  check(fixtureHash === FIXTURE_SHA256, "fixture SHA-256 matches the committed fixture", fixtureHash);
  check(
    fixture.subarray(0, 16).toString("latin1") === "SQLite format 3\u0000",
    "fixture has the SQLite file header",
  );
  check(fixture.includes(Buffer.from("Bălți", "utf8")), "fixture contains Bălți as UTF-8");
  check(fixture.includes(Buffer.from("Кишинёв", "utf8")), "fixture contains Кишинёв as UTF-8");

  const searchSource = readFileSync(SEARCH_MODULE, "utf8");
  for (const forbidden of ["unpkg.com", "jsdelivr", "cdn.", "http://", "https://"]) {
    check(
      !searchSource.includes(forbidden),
      `search.js does not reference ${forbidden}`,
    );
  }

  // -------------------------------------------------------------------------
  // Search assertions: Chromium when available, Node fallback otherwise
  // -------------------------------------------------------------------------
  let puppeteer = null;
  if (!NO_BROWSER) {
    try {
      puppeteer = require("puppeteer");
    } catch {
      console.log("SKIP  browser: puppeteer is not installed (set NODE_PATH or pass --no-browser)");
    }
  } else {
    console.log("SKIP  browser: --no-browser requested");
  }

  let session;
  if (puppeteer) {
    session = await runBrowser(base);
  } else {
    console.log("RUN   search assertions in the Node fallback");
    session = await runNodeFallback(base);
  }

  console.log("");
  printResults(session);
  console.log("");
  assertResults(session);
} finally {
  await new Promise((resolve) => server.close(resolve));
}

console.log("");
if (failures === 0) {
  console.log(
    browserRan
      ? "Offline search smoke passed in Chromium."
      : "Offline search smoke passed (Node fallback).",
  );
  process.exitCode = 0;
} else {
  console.error(`Offline search smoke failed: ${failures}`);
  process.exitCode = 1;
}

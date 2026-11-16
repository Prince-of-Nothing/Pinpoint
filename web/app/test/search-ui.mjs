/**
 * Headless smoke test for the Pinpoint offline search UI component.
 *
 * It serves the `web/` tree on a random loopback port, checks the component,
 * stylesheet and demo for the expected local-only contract, then opens the
 * demo in headless Chromium. The browser flow types "balti", selects the first
 * result with ArrowDown + Enter, verifies the injected onSelect payload,
 * clears the query, and exercises the empty/error states and the imperative
 * methods. Any console error, page error, failed request or external request
 * fails the run.
 *
 * Usage (PowerShell):
 *   $env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
 *   $env:PINPOINT_SCREENSHOT="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\search-ui.png"
 *   node web/app/test/search-ui.mjs
 *
 * Static-only fallback (explicitly does not run the browser):
 *   node web/app/test/search-ui.mjs --static-only
 *
 * Screenshot path: $env:PINPOINT_SCREENSHOT, otherwise
 * <system temp>/pinpoint-search-ui.png. A second screenshot after selection is
 * written next to it as pinpoint-search-ui-selected.png (override with
 * $env:PINPOINT_SCREENSHOT_SELECTED).
 */

import { existsSync, mkdirSync, readFileSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createStaticServer } from "../../map/serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(HERE, "../..");
const SCREENSHOT = process.env.PINPOINT_SCREENSHOT || path.join(tmpdir(), "pinpoint-search-ui.png");
const SELECTED_SCREENSHOT =
  process.env.PINPOINT_SCREENSHOT_SELECTED ||
  path.join(path.dirname(SCREENSHOT), "pinpoint-search-ui-selected.png");
const STATIC_ONLY = process.argv.includes("--static-only");
const BROKEN_ATTRIBUTION = "Ac" + " OpenStreetMap";
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
  // Static assets and local-only contract
  // -------------------------------------------------------------------------
  const assets = [
    ["/app/search-demo.html", "text/html"],
    ["/app/search-ui.js", "text/javascript"],
    ["/app/search.css", "text/css"],
    ["/app/README-search-ui.md", "text/plain"],
  ];

  for (const [pathname, expectedType] of assets) {
    const response = await fetch(base + pathname);
    const type = response.headers.get("content-type") ?? "";
    check(response.status === 200, `HTTP 200 for ${pathname}`, `got ${response.status}`);
    check(type.startsWith(expectedType), `content type for ${pathname}`, `got ${type}`);
    await response.arrayBuffer();
  }

  const { text: demoHtml } = await fetchText(base, "/app/search-demo.html");
  const { text: moduleSource } = await fetchText(base, "/app/search-ui.js");
  const { text: searchCss } = await fetchText(base, "/app/search.css");
  const { text: readme } = await fetchText(base, "/app/README-search-ui.md");

  check(
    moduleSource.includes("export function createSearchUI"),
    "search-ui.js exports createSearchUI",
  );
  check(
    !/\bfetch\s*\(/.test(moduleSource),
    "search-ui.js makes no fetch calls",
  );
  check(
    !/^\s*import\s/m.test(moduleSource),
    "search-ui.js has no imports and loads no remote modules",
  );
  check(
    moduleSource.includes("Search a place…"),
    "search-ui.js carries the required default placeholder",
  );
  check(
    moduleSource.includes("aria-activedescendant") &&
      moduleSource.includes("aria-expanded") &&
      moduleSource.includes("role"),
    "search-ui.js wires the combobox ARIA contract",
  );

  check(
    demoHtml.includes("./search-ui.js") && demoHtml.includes("./search.css"),
    "demo loads the local component and stylesheet",
  );
  const demoText = demoHtml
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  check(
    demoText.includes("© OpenStreetMap contributors") && demoText.includes("ODbL 1.0"),
    "demo shows visible © OpenStreetMap contributors / ODbL 1.0 attribution",
    demoText,
  );
  check(
    demoHtml.includes("balti") && demoHtml.includes("кишинёв"),
    "demo advertises the Bălți and Кишинёв search examples",
  );
  check(
    !demoHtml.includes(BROKEN_ATTRIBUTION) &&
      !demoHtml.includes("\uFFFD") &&
      !demoHtml.includes("\u00C2\u00A9"),
    "demo HTML has no attribution mojibake",
  );

  const remoteAssetPatterns = [
    /<script\b[^>]*\bsrc\s*=\s*["']https?:/i,
    /<link\b[^>]*\bhref\s*=\s*["']https?:/i,
    /<img\b[^>]*\bsrc\s*=\s*["']https?:/i,
    /\burl\(\s*["']?https?:/i,
  ];
  for (const pattern of remoteAssetPatterns) {
    check(!pattern.test(demoHtml), `demo has no remote asset matching ${pattern}`, pattern.source);
  }
  for (const forbidden of ["unpkg.com", "jsdelivr", "cdn.", "tile.openstreetmap.org"]) {
    check(
      !demoHtml.toLowerCase().includes(forbidden),
      `demo does not reference ${forbidden}`,
    );
  }

  check(searchCss.includes(".pinpoint-search"), "search.css targets the component");
  check(
    searchCss.includes('[data-theme="light"]'),
    "search.css has a light theme variant",
  );
  check(!/\burl\(\s*["']?https?:/i.test(searchCss), "search.css has no remote url()");

  check(
    readme.includes("web/map/search.js") && readme.includes("openSearch"),
    "README documents the web/map/search.js openSearch hookup",
  );

  for (const relative of [
    "app/search-ui.js",
    "app/search.css",
    "app/search-demo.html",
    "app/test/search-ui.mjs",
    "app/README-search-ui.md",
  ]) {
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
        !decoded.includes(BROKEN_ATTRIBUTION) &&
          !decoded.includes("\uFFFD") &&
          !decoded.includes("\u00C2\u00A9"),
        `${relative} has no attribution mojibake`,
      );
    }
  }

  // -------------------------------------------------------------------------
  // Browser flow
  // -------------------------------------------------------------------------
  let puppeteer = null;
  if (!STATIC_ONLY) {
    try {
      puppeteer = require("puppeteer");
    } catch (error) {
      console.log("SKIP  browser: puppeteer is not resolvable from this Node process.");
      console.log(`      NODE_PATH=${process.env.NODE_PATH || "(unset)"}`);
      console.log("      Install puppeteer or set NODE_PATH to a node_modules that contains it.");
      console.log("      The headless smoke did NOT run; this is not a pass.");
      failures += 1;
    }
  } else {
    console.log("SKIP  browser: --static-only requested; the headless smoke did NOT run.");
  }

  if (puppeteer) {
    browserRan = true;
    mkdirSync(path.dirname(SCREENSHOT), { recursive: true });

    const browser = await puppeteer.launch({
      headless: true,
      args: ["--no-sandbox", "--disable-setuid-sandbox", "--disable-dev-shm-usage"],
    });

    try {
      const page = await browser.newPage();
      await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
      page.setDefaultTimeout(20000);

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

      const demoUrl = `${base}/app/search-demo.html`;
      try {
        await page.goto(demoUrl, { waitUntil: "domcontentloaded", timeout: 30000 });
      } catch (error) {
        // A cold Chromium start occasionally misses the local navigation
        // budget; retry once and surface the retry in the output.
        console.log(`RETRY page navigation: ${error.message}`);
        await page.goto(demoUrl, { waitUntil: "domcontentloaded", timeout: 30000 });
      }
      await page.waitForFunction(() => Boolean(window.__searchDemo?.ui), {
        timeout: 15000,
      });

      const initial = await page.evaluate(() => {
        const input = document.querySelector(".pinpoint-search__input");
        const list = document.querySelector(".pinpoint-search__list");
        const label = document.querySelector(".pinpoint-search__label");
        const clear = document.querySelector(".pinpoint-search__clear");
        const attribution = document
          .querySelector(".demo-attribution")
          ?.textContent.replace(/\s+/g, " ")
          .trim();

        return {
          role: input?.getAttribute("role"),
          expanded: input?.getAttribute("aria-expanded"),
          controls: input?.getAttribute("aria-controls"),
          listId: list?.id,
          listRole: list?.getAttribute("role"),
          labelFor: label?.getAttribute("for"),
          inputId: input?.id,
          placeholder: input?.getAttribute("placeholder"),
          clearLabel: clear?.getAttribute("aria-label"),
          clearHidden: clear?.hidden ?? null,
          attribution,
        };
      });

      check(initial.role === "combobox", "input uses the combobox role", initial.role ?? "missing");
      check(
        initial.expanded === "false" && initial.controls === initial.listId,
        "combobox starts collapsed and controls the listbox",
        JSON.stringify(initial),
      );
      check(initial.listRole === "listbox", "results use the listbox role", initial.listRole ?? "missing");
      check(
        initial.labelFor === initial.inputId && Boolean(initial.inputId),
        "the input has an associated accessible label",
        JSON.stringify({ labelFor: initial.labelFor, inputId: initial.inputId }),
      );
      check(
        initial.placeholder === "Search a place…",
        "the demo uses the required placeholder",
        initial.placeholder ?? "missing",
      );
      check(
        initial.clearLabel === "Clear search" && initial.clearHidden === true,
        "the clear button has an accessible label and starts hidden",
        JSON.stringify(initial),
      );
      check(
        String(initial.attribution ?? "").includes("© OpenStreetMap contributors") &&
          String(initial.attribution ?? "").includes("ODbL 1.0"),
        "visible attribution contains © OpenStreetMap contributors and ODbL 1.0",
        initial.attribution ?? "missing",
      );

      await page.click(".pinpoint-search__input");
      await page.type(".pinpoint-search__input", "balti", { delay: 25 });
      await page.waitForFunction(
        () => {
          const items = [...document.querySelectorAll(".pinpoint-search__item")];
          return items.length > 0 && items.some((item) => item.textContent.includes("Bălți"));
        },
        { timeout: 15000 },
      );

      const resultsState = await page.evaluate(() => {
        const input = document.querySelector(".pinpoint-search__input");
        const list = document.querySelector(".pinpoint-search__list");
        const items = [...document.querySelectorAll(".pinpoint-search__item")];

        return {
          count: items.length,
          names: items.map((item) =>
            item.querySelector(".pinpoint-search__name")?.textContent?.trim(),
          ),
          firstText: items[0]?.textContent.replace(/\s+/g, " ").trim() ?? "",
          expanded: input?.getAttribute("aria-expanded"),
          focused: document.activeElement === input,
          listVisible: Boolean(list && !list.hidden),
        };
      });

      check(resultsState.count >= 1, "typing balti renders at least one result", String(resultsState.count));
      check(
        resultsState.names[0] === "Bălți",
        "the first result for balti is Bălți",
        JSON.stringify(resultsState.names),
      );
      check(
        resultsState.firstText.includes("Bălți, Moldova"),
        "the result shows its address",
        resultsState.firstText,
      );
      check(resultsState.expanded === "true", "aria-expanded becomes true with results");
      check(resultsState.listVisible, "the result list is visible");
      check(resultsState.focused, "keyboard focus stays on the search input");

      await page.keyboard.press("ArrowDown");
      await page.waitForFunction(() => {
        const item = document.querySelector('.pinpoint-search__item[aria-selected="true"]');
        const input = document.querySelector(".pinpoint-search__input");
        return Boolean(item && input?.getAttribute("aria-activedescendant") === item.id);
      });

      const active = await page.evaluate(() => {
        const item = document.querySelector('.pinpoint-search__item[aria-selected="true"]');
        const input = document.querySelector(".pinpoint-search__input");
        return {
          id: item?.id ?? null,
          text: item?.textContent.replace(/\s+/g, " ").trim() ?? "",
          activeDescendant: input?.getAttribute("aria-activedescendant") ?? null,
        };
      });
      check(
        active.id !== null && active.activeDescendant === active.id,
        "ArrowDown marks the first option active and points aria-activedescendant at it",
        JSON.stringify(active),
      );

      await page.screenshot({ path: SCREENSHOT });
      check(
        existsSync(SCREENSHOT) && statSync(SCREENSHOT).size > 10000,
        `screenshot saved (${SCREENSHOT})`,
        "file missing or too small",
      );

      await page.keyboard.press("Enter");
      await page.waitForFunction(() => window.__searchDemo?.selections?.length === 1, {
        timeout: 10000,
      });

      const selection = await page.evaluate(() => window.__searchDemo.selections[0]);
      check(selection?.name === "Bălți", "Enter calls onSelect with Bălți", selection?.name ?? "missing");
      check(selection?.kind === "city", "onSelect receives the provider kind", selection?.kind ?? "missing");
      check(
        selection?.lat === 47.7615 && selection?.lon === 27.9289,
        "onSelect receives the provider coordinates",
        JSON.stringify({ lat: selection?.lat, lon: selection?.lon }),
      );
      check(
        typeof selection?.address === "string" && selection.address.includes("Moldova"),
        "onSelect receives the provider address",
        selection?.address ?? "missing",
      );
      check(
        typeof selection?.importance === "number",
        "onSelect receives the provider importance",
        String(selection?.importance),
      );
      check(
        !Object.prototype.hasOwnProperty.call(selection ?? {}, "aliases"),
        "onSelect receives only the public result fields",
      );

      const closedAfterSelection = await page.evaluate(() => {
        const input = document.querySelector(".pinpoint-search__input");
        const popover = document.querySelector(".pinpoint-search__popover");
        return {
          expanded: input?.getAttribute("aria-expanded"),
          hidden: popover?.hidden ?? null,
          activeDescendant: input?.getAttribute("aria-activedescendant"),
        };
      });
      check(
        closedAfterSelection.expanded === "false" &&
          closedAfterSelection.hidden === true &&
          closedAfterSelection.activeDescendant === null,
        "selecting a result closes the list and clears aria-activedescendant",
        JSON.stringify(closedAfterSelection),
      );

      const readout = await page.$eval("#selection", (element) => element.textContent.trim());
      check(
        readout.includes("Bălți") && readout.includes("47.76150"),
        "the demo shows the selected result and coordinates",
        readout,
      );

      await page.screenshot({ path: SELECTED_SCREENSHOT });
      check(
        existsSync(SELECTED_SCREENSHOT) && statSync(SELECTED_SCREENSHOT).size > 10000,
        `selected-state screenshot saved (${SELECTED_SCREENSHOT})`,
        "file missing or too small",
      );

      await page.click(".pinpoint-search__clear");
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__input")?.value === "",
      );

      const cleared = await page.evaluate(() => {
        const input = document.querySelector(".pinpoint-search__input");
        const popover = document.querySelector(".pinpoint-search__popover");
        const clear = document.querySelector(".pinpoint-search__clear");
        return {
          value: input?.value,
          expanded: input?.getAttribute("aria-expanded"),
          popoverHidden: popover?.hidden ?? null,
          items: document.querySelectorAll(".pinpoint-search__item").length,
          focused: document.activeElement === input,
          clearHidden: clear?.hidden ?? null,
        };
      });
      check(cleared.value === "", "clear empties the query");
      check(
        cleared.expanded === "false" && cleared.popoverHidden === true && cleared.items === 0,
        "clear closes the list and removes results",
        JSON.stringify(cleared),
      );
      check(cleared.focused, "clear returns focus to the search input");
      check(cleared.clearHidden === true, "clear hides the clear button");

      // A late response must not reopen a list the user dismissed by clicking
      // outside while the provider was still working.
      await page.click(".pinpoint-search__input");
      await page.type(".pinpoint-search__input", "balti", { delay: 20 });
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search")?.dataset.state === "loading",
        { timeout: 15000 },
      );
      const stagePoint = await page.$eval(".demo-stage", (element) => {
        const rect = element.getBoundingClientRect();
        return { x: rect.left + rect.width * 0.75, y: rect.top + rect.height * 0.45 };
      });
      await page.mouse.click(stagePoint.x, stagePoint.y);
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__popover")?.hidden === true,
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
      const dismissedState = await page.evaluate(() => {
        const wrapper = document.querySelector(".pinpoint-search");
        const popover = document.querySelector(".pinpoint-search__popover");
        return {
          state: wrapper?.dataset.state,
          hidden: popover?.hidden ?? null,
          items: document.querySelectorAll(".pinpoint-search__item").length,
        };
      });
      check(
        dismissedState.state === "results" &&
          dismissedState.hidden === true &&
          dismissedState.items > 0,
        "a late result does not reopen a list dismissed by an outside click",
        JSON.stringify(dismissedState),
      );
      await page.click(".pinpoint-search__input");
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__popover")?.hidden === false,
      );
      check(true, "focusing the input reopens the retained results");
      await page.click(".pinpoint-search__clear");
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__input")?.value === "",
      );

      // Cyrillic query: the stub folds Кишинёв to the same key as Chișinău.
      await page.click(".pinpoint-search__input");
      await page.type(".pinpoint-search__input", "кишинёв", { delay: 20 });
      await page.waitForFunction(
        () => {
          const items = [...document.querySelectorAll(".pinpoint-search__item")];
          return items.length > 0 && items[0].textContent.includes("Chișinău");
        },
        { timeout: 15000 },
      );
      const cyrillicNames = await page.evaluate(() =>
        [...document.querySelectorAll(".pinpoint-search__name")].map((node) =>
          node.textContent.trim(),
        ),
      );
      check(
        cyrillicNames[0] === "Chișinău",
        "a Cyrillic query (кишинёв) finds Chișinău",
        JSON.stringify(cyrillicNames),
      );

      await page.keyboard.press("Escape");
      await page.waitForFunction(() => {
        const popover = document.querySelector(".pinpoint-search__popover");
        const input = document.querySelector(".pinpoint-search__input");
        return popover?.hidden === true && input?.getAttribute("aria-expanded") === "false";
      });
      const afterEscape = await page.evaluate(() => {
        const input = document.querySelector(".pinpoint-search__input");
        return {
          value: input?.value,
          focused: document.activeElement === input,
        };
      });
      check(afterEscape.value === "кишинёв", "Escape closes the list but keeps the query", afterEscape.value);
      check(afterEscape.focused, "focus stays on the input after Escape");

      await page.click(".pinpoint-search__clear");
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__input")?.value === "",
      );

      // Empty state.
      await page.click(".pinpoint-search__input");
      await page.type(".pinpoint-search__input", "zzzz", { delay: 20 });
      await page.waitForFunction(
        () => {
          const status = document.querySelector('.pinpoint-search__status[data-tone="empty"]');
          return Boolean(status && !status.hidden && status.textContent.includes("No places found"));
        },
        { timeout: 15000 },
      );
      const emptyText = await page.$eval(
        '.pinpoint-search__status[data-tone="empty"]',
        (element) => element.textContent.trim(),
      );
      check(emptyText.includes("No places found"), "an unmatched query shows the empty state", emptyText);
      await page.click(".pinpoint-search__clear");
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__input")?.value === "",
      );

      // Error state through the demo's one-shot failure switch.
      await page.evaluate(() => window.__searchDemo.setFailNext(true));
      await page.click(".pinpoint-search__input");
      await page.type(".pinpoint-search__input", "balti", { delay: 20 });
      await page.waitForFunction(
        () => {
          const status = document.querySelector('.pinpoint-search__status[data-tone="error"]');
          return Boolean(status && !status.hidden && status.textContent.includes("Search failed"));
        },
        { timeout: 15000 },
      );
      const errorState = await page.evaluate(() => ({
        text: document
          .querySelector('.pinpoint-search__status[data-tone="error"]')
          ?.textContent.trim(),
        errors: window.__searchDemo.errors.slice(),
        note: document.querySelector("#error-note")?.textContent.trim(),
      }));
      check(
        errorState.text.includes("Search failed. Try again."),
        "a rejected provider shows the generic error state",
        errorState.text ?? "missing",
      );
      check(
        errorState.errors.length === 1 &&
          errorState.errors[0].includes("Simulated offline index failure"),
        "onError receives the provider error",
        JSON.stringify(errorState.errors),
      );
      check(
        String(errorState.note ?? "").includes("Provider error"),
        "the demo diagnostic note shows the provider error",
        errorState.note ?? "missing",
      );
      await page.click(".pinpoint-search__clear");
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__input")?.value === "",
      );

      // Imperative setResults + clear + destroy.
      await page.evaluate(() => {
        window.__searchDemo.ui.setResults([
          {
            name: "Test Place",
            kind: "town",
            lat: 47.1,
            lon: 28.2,
            address: "Test address, Moldova",
            importance: 0.5,
          },
        ]);
      });
      await page.waitForFunction(
        () => {
          const items = [...document.querySelectorAll(".pinpoint-search__item")];
          return items.length === 1 && items[0].textContent.includes("Test Place");
        },
        { timeout: 10000 },
      );
      check(true, "setResults renders caller-provided results");

      await page.click("#theme-toggle");
      await page.waitForFunction(
        () =>
          document.documentElement.dataset.theme === "light" &&
          document.querySelector(".pinpoint-search")?.dataset.theme === "light",
      );
      const lightTheme = await page.evaluate(() => {
        const bar = document.querySelector(".pinpoint-search__bar");
        return bar ? getComputedStyle(bar).backgroundColor : null;
      });
      check(
        lightTheme === "rgb(255, 255, 255)",
        "the light theme switches the component surface to light tokens",
        lightTheme ?? "missing",
      );

      await page.click("#theme-toggle");
      await page.waitForFunction(() => document.documentElement.dataset.theme === "dark");
      const darkTheme = await page.evaluate(() => {
        const bar = document.querySelector(".pinpoint-search__bar");
        return bar ? getComputedStyle(bar).backgroundColor : null;
      });
      check(
        darkTheme === "rgb(15, 23, 42)",
        "the dark theme restores the app surface token",
        darkTheme ?? "missing",
      );

      await page.evaluate(() => window.__searchDemo.ui.clear());
      await page.waitForFunction(
        () => document.querySelector(".pinpoint-search__input")?.value === "",
      );
      check(true, "clear works when called imperatively");

      await page.evaluate(() => {
        window.__searchDemo.ui.destroy();
        window.__searchDemo.ui.destroy();
      });
      await page.waitForFunction(() => !document.querySelector(".pinpoint-search"));
      check(true, "destroy removes the component and is idempotent");

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
  if (browserRan) {
    console.log(`Search UI checks passed. Screenshot: ${SCREENSHOT}`);
    console.log(`Selected-state screenshot: ${SELECTED_SCREENSHOT}`);
  } else {
    console.log("Search UI static checks passed (browser smoke did NOT run).");
  }
  process.exitCode = 0;
} else {
  console.error(`Search UI checks failed: ${failures}`);
  process.exitCode = 1;
}

// Live check for keyboard access to search results: ArrowDown enters the
// list, Escape returns to the box, and ArrowDown + Enter opens the place.
// The search proxy is stubbed so no Nominatim or tiles are needed.
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const base = process.env.PINPOINT_BASE_URL || "http://127.0.0.1:8080";

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    serviceWorkers: "block",
  });
  const page = await context.newPage();

  await page.route("**/map/search*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        results: [
          {
            short_name: "Keyboard place",
            display_name: "Keyboard place, Chișinău",
            latitude: 47.01,
            longitude: 28.86,
            distance_m: 120,
          },
        ],
      }),
    }),
  );

  await page.goto(base, { waitUntil: "networkidle" });

  await page.click("#register-toggle");
  await page.fill("#register-email", `live-keyboard-${Date.now()}@example.invalid`);
  await page.fill("#register-name", "Live Keyboard");
  await page.fill("#register-password", "Live-Keyboard-2026!");
  await page.click('#register-form button[type="submit"]');
  await page.waitForSelector("#app-panel:not(.hidden)", { timeout: 15000 });

  if ((await page.getAttribute("#drawer-toggle", "aria-expanded")) === "true") {
    await page.click("#drawer-toggle");
  }

  await page.fill("#search-input", "keyboard place");
  await page.press("#search-input", "Enter");
  await page.waitForSelector("#search-results button.result", { timeout: 5000 });

  await page.press("#search-input", "ArrowDown");
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains("result")),
    true,
    "ArrowDown moves focus to the first result",
  );

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => document.getElementById("search-results").classList.contains("hidden"),
    { timeout: 5000 },
  );
  assert.equal(
    await page.evaluate(() => document.activeElement?.id),
    "search-input",
    "Escape returns focus to the search box",
  );

  // Search again and open the place from the focused result.
  await page.press("#search-input", "Enter");
  await page.waitForSelector("#search-results button.result", { timeout: 5000 });
  await page.press("#search-input", "ArrowDown");
  await page.keyboard.press("Enter");
  await page.waitForSelector("#place-sheet:not(.hidden)", { timeout: 5000 });
  assert.equal(
    (await page.textContent("#place-name")).trim(),
    "Keyboard place",
    "Enter on the focused result opens that place",
  );

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => document.getElementById("place-sheet").classList.contains("hidden"),
    { timeout: 5000 },
  );

  await browser.close();
  console.log("PASS  live search results keyboard access");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

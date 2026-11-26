// Live check for the place sheet: a map click opens it and Escape closes it.
// The empty-state card is hidden and the reverse-geocode proxy is stubbed, so
// the check needs no tiles or external geocoder.
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

  await page.route("**/map/reverse*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        short_name: "Test place",
        display_name: "Test place, Chișinău",
        latitude: 47.01,
        longitude: 28.86,
        distance_m: 120,
      }),
    }),
  );

  await page.goto(base, { waitUntil: "networkidle" });

  await page.click("#register-toggle");
  await page.fill("#register-email", `live-place-${Date.now()}@example.invalid`);
  await page.fill("#register-name", "Live Place");
  await page.fill("#register-password", "Live-Place-2026!");
  await page.click('#register-form button[type="submit"]');
  await page.waitForSelector("#app-panel:not(.hidden)", { timeout: 15000 });

  if ((await page.getAttribute("#drawer-toggle", "aria-expanded")) === "true") {
    await page.click("#drawer-toggle");
  }

  // The empty-state card only blocks the pointer; the map itself is ready.
  await page.evaluate(() => document.getElementById("map-empty-state").classList.add("hidden"));
  await page.waitForTimeout(200);
  await page.mouse.click(300, 400);
  await page.waitForSelector("#place-sheet:not(.hidden)", { timeout: 5000 });
  assert.ok(await page.isVisible("#place-sheet"), "a map click opens the place sheet");
  assert.equal(
    (await page.textContent("#place-name")).trim(),
    "Test place",
    "the reverse-geocode stub supplied the place",
  );
  assert.equal(
    await page.evaluate(() => document.activeElement?.id),
    "place-close",
    "the place sheet takes focus",
  );

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => document.getElementById("place-sheet").classList.contains("hidden"),
    { timeout: 5000 },
  );
  assert.ok(await page.isHidden("#place-sheet"), "Escape closes the place sheet");
  assert.equal(
    await page.evaluate(() => document.activeElement?.classList.contains("maplibregl-canvas")),
    true,
    "Escape returns focus to the map",
  );

  await browser.close();
  console.log("PASS  live place sheet opens, takes focus and Escape closes it");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

// Live check for the place sheet: a map click opens it, Escape closes it.
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const base = process.env.PINPOINT_BASE_URL || "http://127.0.0.1:8080";

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const page = await context.newPage();
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

  // Load the online basemap if the empty-state card is showing, so a map
  // click can select a place.
  if (await page.isVisible("#map-empty-state")) {
    await page.click("#empty-online");
    await page
      .waitForFunction(
        () => document.getElementById("map-empty-state").classList.contains("hidden"),
        { timeout: 5000 },
      )
      .catch(() => {});
    await page.waitForTimeout(1000);
  }

  await page.mouse.click(300, 400);
  await page.waitForSelector("#place-sheet:not(.hidden)", { timeout: 8000 });
  assert.ok(await page.isVisible("#place-sheet"), "a map click opens the place sheet");

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => document.getElementById("place-sheet").classList.contains("hidden"),
    { timeout: 5000 },
  );
  assert.ok(await page.isHidden("#place-sheet"), "Escape closes the place sheet");

  await browser.close();
  console.log("PASS  live place sheet opens and Escape closes it");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

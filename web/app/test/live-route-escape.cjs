// Live check for the web route sheet: open it from the map menu, then close
// it with Escape (the shared sheet handler).
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const base = process.env.PINPOINT_BASE_URL || "http://127.0.0.1:8080";

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const page = await context.newPage();
  await page.goto(base, { waitUntil: "networkidle" });

  await page.click("#register-toggle");
  await page.fill("#register-email", `live-route-${Date.now()}@example.invalid`);
  await page.fill("#register-name", "Live Route");
  await page.fill("#register-password", "Live-Route-2026!");
  await page.click('#register-form button[type="submit"]');
  await page.waitForSelector("#app-panel:not(.hidden)", { timeout: 15000 });

  if ((await page.getAttribute("#drawer-toggle", "aria-expanded")) === "true") {
    await page.click("#drawer-toggle");
  }

  await page.mouse.click(80, 550, { button: "right" });
  await page.waitForSelector("#map-menu:not(.hidden)", { timeout: 5000 });
  await page.click('[data-map-action="from"]');
  await page.waitForSelector("#route-panel:not(.hidden)", { timeout: 5000 });
  assert.ok(await page.isVisible("#route-panel"), "the map menu opens the route panel");

  await page.keyboard.press("Escape");
  await page.waitForFunction(() => document.getElementById("route-panel").classList.contains("hidden"), { timeout: 5000 });
  assert.ok(await page.isHidden("#route-panel"), "Escape closes the route panel");

  await browser.close();
  console.log("PASS  live route panel opens and Escape closes it");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

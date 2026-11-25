// Live check for the web drawer: opening it, clicking inside it and clicking
// the visible map (which should dismiss it).
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const base = process.env.PINPOINT_BASE_URL || "http://127.0.0.1:8080";

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const page = await context.newPage();
  await page.goto(base, { waitUntil: "networkidle" });

  await page.click("#register-toggle");
  await page.fill("#register-email", `live-drawer-${Date.now()}@example.invalid`);
  await page.fill("#register-name", "Live Drawer");
  await page.fill("#register-password", "Live-Drawer-2026!");
  await page.click('#register-form button[type="submit"]');
  await page.waitForSelector("#app-panel:not(.hidden)", { timeout: 15000 });

  const expanded = () => page.getAttribute("#drawer-toggle", "aria-expanded");

  if ((await expanded()) !== "true") await page.click("#drawer-toggle");
  assert.equal(await expanded(), "true", "drawer should open");

  await page.click("#social-drawer h2");
  assert.equal(await expanded(), "true", "a click inside the drawer keeps it open");

  await page.locator("#map").click({ position: { x: 120, y: 500 } });
  assert.equal(await expanded(), "false", "a click on the map closes the drawer");

  await browser.close();
  console.log("PASS  live web drawer open/inside/map-click");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

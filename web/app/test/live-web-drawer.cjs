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

  // Layout: 1280 is a 380 px right column, 700 is a full-width bottom sheet.
  if ((await expanded()) !== "true") await page.click("#drawer-toggle");
  const wide = await page.evaluate(() => {
    const cs = getComputedStyle(document.getElementById("social-drawer"));
    return { position: cs.position, left: cs.left, right: cs.right, width: cs.width };
  });
  assert.equal(wide.position, "absolute", "wide drawer positioning");
  assert.equal(wide.left, "900px", "wide drawer left edge");
  assert.equal(wide.right, "0px", "wide drawer right edge");
  assert.equal(wide.width, "380px", "wide drawer width");

  await page.setViewportSize({ width: 700, height: 720 });
  await page.waitForTimeout(300);
  const narrow = await page.evaluate(() => {
    const cs = getComputedStyle(document.getElementById("social-drawer"));
    return { position: cs.position, left: cs.left, right: cs.right, width: cs.width, bottom: cs.bottom };
  });
  assert.equal(narrow.position, "absolute", "narrow drawer positioning");
  assert.equal(narrow.left, "0px", "narrow drawer left edge");
  assert.equal(narrow.right, "0px", "narrow drawer right edge");
  assert.equal(narrow.width, "700px", "narrow drawer full width");
  assert.notEqual(narrow.bottom, "auto", "narrow drawer is bottom-anchored");

  await browser.close();
  console.log("PASS  live web drawer open/inside/map-click/layout");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

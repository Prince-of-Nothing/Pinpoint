// Live check for the "/" search shortcut: the pure predicate is unit-tested,
// this drives a real browser against the served client.
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const base = process.env.PINPOINT_BASE_URL || "http://127.0.0.1:8080";

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const page = await context.newPage();
  await page.goto(base, { waitUntil: "networkidle" });

  await page.click("#register-toggle");
  await page.fill("#register-email", `live-shortcut-${Date.now()}@example.invalid`);
  await page.fill("#register-name", "Live Shortcut");
  await page.fill("#register-password", "Live-Shortcut-2026!");
  await page.click('#register-form button[type="submit"]');
  await page.waitForSelector("#app-panel:not(.hidden)", { timeout: 15000 });

  const active = () => page.evaluate(() => document.activeElement?.id || "");
  const expanded = () => page.getAttribute("#drawer-toggle", "aria-expanded");

  // Desktop sign-in opens the drawer; close it for the bare case.
  if ((await expanded()) === "true") await page.click("#drawer-toggle");
  await page.evaluate(() => document.activeElement?.blur());

  await page.keyboard.press("/");
  assert.equal(await active(), "search-input", "bare / should focus search");

  await page.keyboard.type("a");
  await page.keyboard.press("/");
  assert.equal(
    await page.inputValue("#search-input"),
    "a/",
    "/ inside the field should be typed",
  );
  assert.equal(await active(), "search-input", "focus should stay in the field");

  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("Control+/");
  assert.notEqual(await active(), "search-input", "modified / must not focus search");

  if ((await expanded()) !== "true") await page.click("#drawer-toggle");
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("/");
  assert.notEqual(await active(), "search-input", "an open drawer must block /");

  await browser.close();
  console.log("PASS  live / search shortcut gates");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

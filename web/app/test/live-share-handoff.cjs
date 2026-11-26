// Live check for the narrow drawer → share sheet handoff: the drawer closes,
// the sheet takes focus, and the drawer's inert isolation is lifted. Friends
// and shares are stubbed so no second account is needed.
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

const base = process.env.PINPOINT_BASE_URL || "http://127.0.0.1:8080";

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 900, height: 720 },
    serviceWorkers: "block",
  });
  const page = await context.newPage();

  await page.route("**/api/v1/friendships/**", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([
        {
          id: "friend-live-1",
          requester_id: "stub-requester",
          receiver_id: "stub-receiver",
          requester_name: "Live Friend",
          receiver_name: "Live User",
          status: "accepted",
        },
      ]),
    }),
  );
  await page.route("**/api/v1/locations/shares*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: "[]",
    }),
  );

  await page.goto(base, { waitUntil: "networkidle" });
  await page.click("#register-toggle");
  await page.fill("#register-email", `live-share-${Date.now()}@example.invalid`);
  await page.fill("#register-name", "Live Share");
  await page.fill("#register-password", "Live-Share-2026!");
  await page.click('#register-form button[type="submit"]');
  await page.waitForSelector("#app-panel:not(.hidden)", { timeout: 15000 });

  assert.equal(
    await page.getAttribute("#drawer-toggle", "aria-expanded"),
    "false",
    "the drawer starts closed at 900 px",
  );
  await page.click("#drawer-toggle");
  assert.equal(await page.getAttribute("#drawer-toggle", "aria-expanded"), "true");

  const sharing = page.locator("#friends-list").getByRole("button", { name: "Sharing" });
  await sharing.waitFor({ timeout: 5000 });
  await sharing.click();

  await page.waitForSelector("#share-sheet:not(.hidden)", { timeout: 5000 });
  assert.equal(
    await page.getAttribute("#drawer-toggle", "aria-expanded"),
    "false",
    "the handoff closes the narrow drawer",
  );
  assert.equal(
    await page.evaluate(() => document.activeElement?.id),
    "share-enabled",
    "the opened share sheet takes focus",
  );
  assert.equal(
    await page.evaluate(() => document.querySelector(".map-topbar").inert),
    false,
    "the drawer isolation is lifted",
  );

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => document.getElementById("share-sheet").classList.contains("hidden"),
    { timeout: 5000 },
  );

  await browser.close();
  console.log("PASS  live share handoff closes the drawer and focuses the sheet");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

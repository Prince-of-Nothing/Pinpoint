// Live check for the narrow drawer → message sheet handoff: the drawer
// closes, the composer takes focus, and the drawer's inert isolation is
// lifted. Friends and messages are stubbed so no second account is needed.
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
  await page.route("**/api/v1/messages/key", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: "{}",
    }),
  );
  await page.route("**/api/v1/messages/stub-requester*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        peer: { user_id: "stub-requester", full_name: "Live Friend", key: null },
        messages: [],
      }),
    }),
  );

  await page.goto(base, { waitUntil: "networkidle" });
  await page.click("#register-toggle");
  await page.fill("#register-email", `live-message-${Date.now()}@example.invalid`);
  await page.fill("#register-name", "Live Message");
  await page.fill("#register-password", "Live-Message-2026!");
  await page.click('#register-form button[type="submit"]');
  await page.waitForSelector("#app-panel:not(.hidden)", { timeout: 15000 });

  assert.equal(
    await page.getAttribute("#drawer-toggle", "aria-expanded"),
    "false",
    "the drawer starts closed at 900 px",
  );
  await page.click("#drawer-toggle");
  assert.equal(await page.getAttribute("#drawer-toggle", "aria-expanded"), "true");

  const message = page.locator("#friends-list").getByRole("button", { name: "Message" });
  await message.waitFor({ timeout: 5000 });
  await message.click();

  await page.waitForSelector("#message-sheet:not(.hidden)", { timeout: 5000 });
  assert.equal(
    await page.getAttribute("#drawer-toggle", "aria-expanded"),
    "false",
    "the handoff closes the narrow drawer",
  );
  assert.equal(
    await page.evaluate(() => document.activeElement?.id),
    "message-input",
    "the opened message sheet takes focus in the composer",
  );
  assert.equal(
    await page.evaluate(() => document.querySelector(".map-topbar").inert),
    false,
    "the drawer isolation is lifted",
  );

  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () => document.getElementById("message-sheet").classList.contains("hidden"),
    { timeout: 5000 },
  );

  await browser.close();
  console.log("PASS  live message handoff closes the drawer and focuses the composer");
})().catch((error) => {
  console.error(error);
  process.exit(1);
});

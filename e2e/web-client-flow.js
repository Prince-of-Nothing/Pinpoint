/*
 * End-to-end check of the web client in headless Chromium (Task 7 evidence).
 *
 *   docker compose up --build            # or run the API locally on :8000
 *   npm install playwright && npx playwright install chromium
 *   node e2e/web-client-flow.js          # BASE_URL / OUT_DIR / CHROMIUM_PATH optional
 *
 * Two users: friend request -> accept -> exact share -> manual location ->
 * check-in -> recipient sees live marker and check-in after a reload
 * (session resume via HttpOnly refresh cookie + CSRF) -> revoke -> access
 * gone -> logout clears cookies. Also stores an XSS payload as a display
 * name and fails if any alert() fires or a CSP violation is logged.
 * Note: new accounts count against the registration rate limit (5/hour/IP).
 */
const { chromium } = require(process.env.PW_PATH || "playwright");
const BASE = process.env.BASE_URL || "http://localhost:8000/";
const S = process.env.OUT_DIR || ".";
const suffix = Date.now().toString().slice(-6);
const PASS = "correct horse battery staple";

(async () => {
  const browser = await chromium.launch(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {});
  const problems = [];
  async function newUser(name) {
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const page = await ctx.newPage();
    page.on("console", (m) => { if (m.type() === "error" && !m.text().includes("tile.openstreetmap")) problems.push(`${name} console: ${m.text()}`); });
    page.on("dialog", async (d) => { if (d.type() === "alert") problems.push(`${name} ALERT fired: ${d.message()}`); await d.accept(); });
    await page.goto(BASE);
    const username = `${name}${suffix}`;
    await page.fill("#register-form [name=username]", username);
    await page.fill("#register-form [name=email]", `${username}@example.com`);
    await page.fill("#register-form [name=password]", PASS);
    await page.click("#register-form button");
    await page.waitForFunction(() => document.querySelector("#login-form [name=login]").value !== "");
    await page.fill("#login-form [name=password]", PASS);
    await page.click("#login-form button");
    await page.waitForSelector("#app-view:not(.hidden)");
    return { ctx, page, username };
  }
  const alice = await newUser("alice");
  const bob = await newUser("bob");

  // Friend request + accept
  await alice.page.click("[data-tab=friends]");
  await alice.page.fill("#friend-request-form [name=username]", bob.username);
  await alice.page.click("#friend-request-form button");
  await alice.page.waitForSelector("#outgoing-list li:not(.empty)");
  await bob.page.click("[data-tab=friends]");
  await bob.page.waitForSelector("#incoming-list button:has-text('Accept')");
  await bob.page.click("#incoming-list button:has-text('Accept')");
  await bob.page.waitForSelector("#friends-list li:not(.empty)");

  // XSS attempt through the display name
  await alice.page.click("[data-tab=account]");
  await alice.page.fill("#profile-form [name=display_name]", "<img src=x onerror=alert(1)>");
  await alice.page.click("#profile-form button");
  await alice.page.waitForTimeout(500);

  // Share exact location for 1 hour
  await alice.page.click("[data-tab=sharing]");
  await alice.page.waitForSelector("#share-friend option", { state: "attached" });
  await alice.page.selectOption("#share-form [name=precision]", "exact");
  await alice.page.fill("#share-form [name=purpose]", "Walking home");
  await alice.page.click("#share-form button");
  await alice.page.waitForSelector("#outgoing-shares li:not(.empty)");

  // Manual location: click the map
  await alice.page.click("[data-tab=map]");
  await alice.page.selectOption("#report-mode", "manual");
  await alice.page.mouse.click(400, 400);
  await alice.page.waitForFunction(() => document.querySelector("#my-status").textContent.startsWith("Last sent"));

  // Check-in
  await alice.page.click("[data-tab=checkins]");
  await alice.page.fill("#checkin-form [name=note]", "Evening run");
  await alice.page.click("#checkin-form button");
  await alice.page.waitForSelector("#my-checkins li:not(.empty)");

  // Bob sees Alice live on the map, reloads (session resume via refresh cookie + CSRF)
  await bob.page.reload();
  await bob.page.waitForSelector("#app-view:not(.hidden)");
  await bob.page.waitForSelector("#visible-list li:not(.empty)");
  const row = await bob.page.textContent("#visible-list li");
  const markers = await bob.page.locator(".leaflet-interactive").count();
  await bob.page.locator("#visible-list li").first().click();
  const nameShown = await bob.page.textContent("#visible-list li span");
  await bob.page.evaluate(() => document.querySelectorAll("path.leaflet-interactive")[1]?.dispatchEvent(new MouseEvent("click", { bubbles: true })));
  await bob.page.waitForTimeout(500);
  await bob.page.screenshot({ path: `${S}/bob-map.png` });
  await bob.page.click("[data-tab=checkins]");
  await bob.page.waitForSelector("#shared-checkins li:not(.empty)");
  const sharedCheckin = await bob.page.textContent("#shared-checkins li");
  await bob.page.screenshot({ path: `${S}/bob-checkins.png` });

  // Alice revokes; Bob loses access
  await alice.page.click("[data-tab=sharing]");
  await alice.page.click("#outgoing-shares button:has-text('Stop sharing')");
  await alice.page.waitForSelector("#outgoing-shares li.empty");
  await bob.page.click("[data-tab=map]");
  await bob.page.waitForSelector("#visible-list li.empty");
  const afterRevoke = await bob.page.textContent("#visible-list");

  // Logout, then reload stays logged out
  await bob.page.click("#logout-btn");
  await bob.page.waitForSelector("#auth-view:not(.hidden)");
  const cookiesAfterLogout = (await bob.ctx.cookies()).map((c) => c.name);
  await bob.page.reload();
  await bob.page.waitForSelector("#auth-view:not(.hidden)");

  const result = { cookiesAfterLogout, nameShown, row, markers, sharedCheckin, afterRevoke, problems };
  console.log(JSON.stringify(result, null, 1));
  await browser.close();
  // Map tiles may be unreachable offline; everything else must be clean
  const real = problems.filter((p) => !p.includes("ERR_TUNNEL") && !p.includes("ERR_NAME_NOT_RESOLVED"));
  const ok = real.length === 0 && cookiesAfterLogout.length === 0 && markers > 0 && afterRevoke.includes("Nobody");
  console.log(ok ? "E2E PASSED" : "E2E FAILED");
  process.exit(ok ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(1); });

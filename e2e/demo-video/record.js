/*
 * Records the two-user demo used in the presentation video.
 *
 *   (API running on :8000 with fresh demo data: python backend/scripts/seed_demo.py --reset,
 *    and rate-limit counters cleared: redis-cli flushall)
 *   npm install playwright && npx playwright install chromium
 *   node e2e/demo-video/record.js            # writes out/maria.webm, out/alex.webm, out/marks.json
 *   python e2e/demo-video/compose.py         # builds out/always-together-demo.mp4
 *
 * Maria (demo_parent) is on the left, Alex (demo_child) on the right. Every
 * action goes through the real web client and API. A visible cursor is drawn
 * because headless recordings do not show the mouse.
 *
 * Environment: BASE_URL (default http://localhost:8000/), OUT_DIR (default
 * e2e/demo-video/out), CHROMIUM_PATH, PW_PATH, DEMO_PASSWORD.
 */
"use strict";
const fs = require("fs");
const path = require("path");
const { chromium } = require(process.env.PW_PATH || "playwright");

const BASE = process.env.BASE_URL || "http://localhost:8000/";
const API = new URL("/api/v1", BASE).toString();
const OUT = process.env.OUT_DIR || path.join(__dirname, "out");
const PASS = process.env.DEMO_PASSWORD || "together demo 2026";
const SIZE = { width: 960, height: 1080 };

// Drawn in the page with DOM style properties only (the app's CSP blocks inline <style>)
function installCursor() {
  const make = () => {
    if (document.getElementById("__demo_cursor")) return;
    const c = document.createElement("div");
    c.id = "__demo_cursor";
    Object.assign(c.style, {
      position: "fixed", left: "480px", top: "540px", width: "24px", height: "24px",
      borderRadius: "50%", background: "rgba(99,102,241,0.30)", border: "3px solid #6366F1",
      zIndex: "2147483647", pointerEvents: "none", transform: "translate(-50%,-50%)",
      transition: "width .12s, height .12s, background .12s",
    });
    document.documentElement.appendChild(c);
    document.addEventListener("mousemove", (e) => { c.style.left = e.clientX + "px"; c.style.top = e.clientY + "px"; }, true);
    document.addEventListener("mousedown", () => { c.style.width = "38px"; c.style.height = "38px"; c.style.background = "rgba(99,102,241,0.55)"; }, true);
    document.addEventListener("mouseup", () => { c.style.width = "24px"; c.style.height = "24px"; c.style.background = "rgba(99,102,241,0.30)"; }, true);
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", make);
  else make();
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function moveTo(page, x, y) {
  await page.mouse.move(x, y, { steps: 28 });
  await sleep(250);
}

async function clickAt(page, x, y) {
  await moveTo(page, x, y);
  await page.mouse.down();
  await sleep(90);
  await page.mouse.up();
  await sleep(450);
}

async function click(page, target) {
  const loc = typeof target === "string" ? page.locator(target).first() : target;
  await loc.scrollIntoViewIfNeeded();
  let b = await loc.boundingBox();
  await moveTo(page, b.x + b.width / 2, b.y + b.height / 2);
  // Banners can appear or hide while the cursor travels; re-aim before clicking
  b = await loc.boundingBox();
  await clickAt(page, b.x + b.width / 2, b.y + b.height / 2);
}

async function type(page, selector, text) {
  await click(page, selector);
  await page.locator(selector).fill("");
  await page.locator(selector).pressSequentially(text, { delay: 55 });
  await sleep(250);
}

async function choose(page, selector, value) {
  await click(page, selector);
  await page.selectOption(selector, value);
  await sleep(500);
}

async function api(pathname, { method = "GET", token, body } = {}) {
  const res = await fetch(API + pathname, {
    method,
    headers: { "content-type": "application/json", ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  return res.status === 204 ? null : res.json();
}

async function login(page, username) {
  await page.goto(BASE);
  await page.waitForSelector("#login-form");
  await sleep(600);
  await type(page, "#login-form [name=login]", username);
  await type(page, "#login-form [name=password]", PASS);
  await click(page, "#login-form button");
  await page.waitForSelector("#app-view:not(.hidden)");
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });

  // Story setup: Alex is not sharing with Maria yet
  const alexTokens = await api("/auth/login", { method: "POST", body: { login: "demo_child", password: PASS, token_delivery: "body" } });
  const shares = await api("/location-shares", { token: alexTokens.access_token });
  for (const s of shares.outgoing) await api(`/location-shares/${s.id}`, { method: "DELETE", token: alexTokens.access_token });
  await api("/auth/logout", { method: "POST", token: alexTokens.access_token });

  const browser = await chromium.launch(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {});
  const newPage = async () => {
    const ctx = await browser.newContext({ viewport: SIZE, recordVideo: { dir: OUT, size: SIZE } });
    await ctx.addInitScript(installCursor);
    const page = await ctx.newPage();
    return { ctx, page, created: Date.now() };
  };
  const maria = await newPage();
  const alex = await newPage();
  const T0 = Math.max(maria.created, alex.created);
  const marks = [];
  const mark = (id) => marks.push({ id, t: (Date.now() - T0) / 1000 });

  // 1. Both log in. Friends, but Maria cannot see Alex.
  mark("friends");
  await Promise.all([login(maria.page, "demo_parent"), login(alex.page, "demo_child")]);
  await maria.page.waitForSelector("#visible-list li:not(.empty)");
  await sleep(2500);

  // 2. Alex shares with Maria: exact, 1 hour, "Walking home"
  mark("share");
  await click(alex.page, "[data-tab=sharing]");
  await alex.page.waitForSelector("#share-friend option", { state: "attached" });
  await choose(alex.page, "#share-form [name=precision]", "exact");
  await choose(alex.page, "#share-form [name=expires_in_minutes]", "60");
  await type(alex.page, "#share-form [name=purpose]", "Walking home");
  await click(alex.page, "#share-form button");
  await alex.page.waitForSelector("#outgoing-shares li:not(.empty)");
  await sleep(1500);

  // 3. Maria refreshes and sees Alex live
  mark("live");
  await click(maria.page, "[data-tab=map]");
  await maria.page.waitForFunction(() => document.querySelector("#visible-list").textContent.includes("demo_child"));
  await sleep(800);
  // Clicking a friend in the list centres the map on them and opens their popup
  await click(maria.page, maria.page.locator("#visible-list li", { hasText: "demo_child" }));
  await sleep(3200);

  // 4. Alex moves (manual position for the demo); Maria's marker follows
  mark("move");
  await click(alex.page, "[data-tab=map]");
  await choose(alex.page, "#report-mode", "manual");
  const box = await alex.page.locator("#map").boundingBox();
  await clickAt(alex.page, box.x + box.width * 0.56, box.y + box.height * 0.40);
  await alex.page.waitForFunction(() => document.querySelector("#my-status").textContent.startsWith("Last sent"));
  await sleep(1000);
  await click(maria.page, "[data-tab=map]");
  await sleep(1200);
  await click(maria.page, maria.page.locator("#visible-list li", { hasText: "demo_child" }));
  await sleep(3200);

  // 5. Check-ins: Maria sees Jordan missed; Alex taps "I'm OK"
  mark("checkins");
  await click(maria.page, "[data-tab=checkins]");
  await maria.page.waitForSelector("#shared-checkins li:not(.empty)");
  await click(alex.page, "[data-tab=checkins]");
  await alex.page.waitForSelector("#my-checkins li:not(.empty)");
  await sleep(1500);
  await click(alex.page, "#my-checkins button:has-text(\"I'm OK\")");
  await sleep(1200);
  await click(maria.page, "[data-tab=checkins]");
  await maria.page.waitForFunction(() => document.querySelector("#shared-checkins").textContent.includes("completed"));
  await sleep(2500);

  // 6. Alex stops sharing; Maria loses access immediately
  mark("stop");
  await click(alex.page, "[data-tab=sharing]");
  await alex.page.waitForSelector("#outgoing-shares li:not(.empty)");
  await sleep(600);
  await click(alex.page, "#outgoing-shares button:has-text('Stop sharing')");
  await alex.page.waitForSelector("#outgoing-shares li.empty");
  await sleep(800);
  await click(maria.page, "[data-tab=map]");
  await maria.page.waitForFunction(() => !document.querySelector("#visible-list").textContent.includes("demo_child"));
  await sleep(3000);
  mark("end");

  const files = {};
  for (const [name, who] of [["maria", maria], ["alex", alex]]) {
    const video = who.page.video();
    await who.ctx.close();
    const target = path.join(OUT, `${name}.webm`);
    await video.saveAs(target);
    await video.delete();
    files[name] = { file: target, offset: (T0 - who.created) / 1000 };
  }
  await browser.close();
  fs.writeFileSync(path.join(OUT, "marks.json"), JSON.stringify({ files, marks }, null, 2));
  console.log(JSON.stringify({ files, marks }, null, 2));
})().catch((e) => { console.error(e); process.exit(1); });

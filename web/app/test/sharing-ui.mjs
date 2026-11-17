// Pure checks for the sharing consent helpers plus static checks that the UI
// keeps the confirmation, expiry-preserving save and DELETE revoke paths.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = process.cwd();
const sharingUrl = pathToFileURL(path.resolve("web/app/sharing.js")).href;
const { friendName, precisionLabel, shareStatus, expiryText } = await import(sharingUrl);
const sharingSource = await readFile(path.join(root, "web/app/sharing.js"), "utf8");
const appSource = await readFile(path.join(root, "web/app/app.js"), "utf8");
const pageSource = await readFile(path.join(root, "web/app/index.html"), "utf8");

let passed = 0;
function check(name, fn) {
  try {
    fn();
    console.log(`PASS  ${name}`);
    passed += 1;
  } catch (error) {
    console.error(`FAIL  ${name}: ${error.message}`);
    process.exitCode = 1;
  }
}

const outgoing = {
  id: 7,
  requester_id: 1,
  receiver_id: 2,
  requester_name: "Me",
  receiver_name: "Alice",
};
const incoming = {
  id: 8,
  requester_id: 2,
  receiver_id: 1,
  requester_name: "Bob",
  receiver_name: "Me",
};

check("friendName resolves the other side of the friendship", () => {
  assert.equal(friendName(outgoing, 1), "Alice");
  assert.equal(friendName(incoming, 1), "Bob");
});

check("friendName falls back to the friendship id", () => {
  assert.equal(friendName({ id: 12 }, 1), "friendship #12");
  assert.equal(friendName(null, 1), "this friend");
});

check("precisionLabel uses readable labels", () => {
  assert.equal(precisionLabel("exact"), "Exact");
  assert.equal(precisionLabel("approximate"), "Approximate");
  assert.equal(precisionLabel("city_only"), "City only");
  assert.equal(precisionLabel("unexpected"), "unexpected");
});

const now = new Date("2026-11-17T12:00:00Z");
const future = new Date("2026-11-18T12:00:00Z").toISOString();
const past = new Date("2026-11-16T12:00:00Z").toISOString();

check("shareStatus separates active, paused and expired", () => {
  assert.equal(shareStatus({ is_active: true, expires_at: future }, now), "active");
  assert.equal(shareStatus({ is_active: false, expires_at: future }, now), "paused");
  assert.equal(shareStatus({ is_active: true, expires_at: past }, now), "expired");
  assert.equal(shareStatus({ is_active: true }, now), "active");
  assert.equal(shareStatus({ is_active: false }, now), "paused");
});

check("expiryText distinguishes expiry from no expiry", () => {
  assert.equal(expiryText({}), "no expiry");
  assert.match(expiryText({ expires_at: future }), /^expires /);
  assert.match(expiryText({ expires_at: past }), /^expired /);
});

check("sharing sheet confirms before granting and keeps existing expiry", () => {
  assert.match(sharingSource, /window\.confirm\(summary\(\)\)/);
  assert.match(sharingSource, /select\.querySelector\('option\[value="keep"\]'\)/);
  assert.match(sharingSource, /body\.expires_at = current\.expires_at/);
  assert.match(sharingSource, /method: "DELETE"/);
  assert.match(sharingSource, /Sharing revoked\./);
});

check("shares card uses the consent heading, hint and live summary", () => {
  assert.match(pageSource, /<h2>Who can see me<\/h2>/);
  assert.match(pageSource, /id="share-summary" class="muted" aria-live="polite"/);
  assert.match(pageSource, /pause or revoke any time/i);
});

check("share rows revoke with DELETE and only pause/resume non-expired rows", () => {
  assert.match(appSource, /renderShares\(Array\.isArray\(shares\) \? shares : \[\], friends\)/);
  assert.match(appSource, /revokeShare\(s\.id\)/);
  assert.match(appSource, /method: "DELETE"/);
  assert.match(appSource, /if \(status === "expired"\)/);
});

console.log(`sharing UI checks passed: ${passed}`);

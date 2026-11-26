// Account → Server: which addresses the web app accepts as another Pinpoint
// server, and how it remembers recent ones (web/app/server-address.js).
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const { normalizeServerUrl, rememberServer, isLoopbackHost, MAX_RECENT_SERVERS } = await import(
  pathToFileURL(path.resolve("web/app/server-address.js")).href
);

const here = "https://pinpoint.example.org";
const ok = (raw) => normalizeServerUrl(raw, here);

// A bare host means https; paths, queries and fragments reduce to the origin.
assert.deepEqual(ok("other.example.org"), { ok: true, origin: "https://other.example.org", host: "other.example.org", same: false });
assert.equal(ok("https://other.example.org/app/?x=1#y").origin, "https://other.example.org");
assert.equal(ok("  https://other.example.org:8443  ").origin, "https://other.example.org:8443");

// Plain http only for this computer (same rule as the native client, ADR-0006).
assert.equal(ok("http://localhost:8080").origin, "http://localhost:8080");
assert.equal(ok("http://127.0.0.1:8000").ok, true);
assert.equal(ok("http://[::1]:8000").ok, true);
assert.equal(ok("http://dev.localhost:3000").ok, true);
assert.match(ok("http://other.example.org").error, /Use https/);
assert.match(ok("http://192.168.1.10:8080").error, /Use https/);
assert.equal(isLoopbackHost("LOCALHOST"), true);
assert.equal(isLoopbackHost("localhost.example.org"), false);

// Refused outright.
assert.match(ok("").error, /Enter a server address/);
assert.match(ok("ftp://files.example.org").error, /https/);
assert.match(ok("javascript:alert(1)").error, /valid address|https/);
assert.match(ok("https://user:secret@other.example.org").error, /user name or password/);
assert.match(ok("https://").error, /valid address/);
assert.match(ok(`https://${"a".repeat(300)}.org`).error, /too long/);

// The current server is recognised, so the app can say so instead of reloading.
assert.equal(ok("pinpoint.example.org/").same, true);

// Recent servers: most recent first, no duplicates, bounded.
let recent = [];
for (const n of [1, 2, 3, 2, 4, 5, 6, 7]) recent = rememberServer(recent, `https://s${n}.example.org`);
assert.deepEqual(recent, ["https://s7.example.org", "https://s6.example.org", "https://s5.example.org", "https://s4.example.org", "https://s2.example.org"]);
assert.equal(recent.length, MAX_RECENT_SERVERS);
assert.deepEqual(rememberServer("not a list", "https://a.example.org"), ["https://a.example.org"]);

// Wiring: the card, validation before leaving, and an explicit confirmation.
const page = await readFile("web/app/index.html", "utf8");
const app = await readFile("web/app/app.js", "utf8");
for (const id of ["server-card", "server-current", "server-form", "server-url", "server-error", "server-recent"]) {
  assert.ok(page.includes(`id="${id}"`), `missing #${id}`);
}
assert.match(page, /id="server-card" data-sections="account"/);
assert.match(app, /normalizeServerUrl\(\$\("server-url"\)\.value, window\.location\.origin\)/);
assert.match(app, /if \(!window\.confirm\(`Open Pinpoint on \$\{host\}\?/);
assert.match(app, /window\.location\.assign\(`\$\{origin\}\/`\)/);

console.log("PASS  server address: https (or loopback http), origin only, recent list, confirmed switch");

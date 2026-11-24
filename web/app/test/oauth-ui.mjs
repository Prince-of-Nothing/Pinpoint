// OAuth UI wiring: the provider buttons only appear when the server reports a
// configured provider, the start route is used, and callback errors are shown.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile("web/app/app.js", "utf8");
const page = await readFile("web/app/index.html", "utf8");
const css = await readFile("web/app/app.css", "utf8");

assert.ok(page.includes('id="oauth-buttons"'), "missing the OAuth button container");
assert.match(app, /\/api\/v1\/auth\/providers/);
assert.match(
  app,
  /\/api\/v1\/auth\/oauth\/\$\{encodeURIComponent\(provider\)\}\/start/,
  "the button must open the provider start route",
);
assert.match(app, /oauth_error/);
assert.match(app, /github: "GitHub"/, "GitHub must render with its official capitalisation");
for (const code of [
  "provider_denied",
  "state_mismatch",
  "email_unverified",
  "email_exists",
  "account_disabled",
]) {
  assert.match(app, new RegExp(`${code}:`), `no message for ${code}`);
}
assert.ok(css.includes(".oauth-buttons"), "missing the OAuth button styles");

console.log("PASS  OAuth provider buttons and callback errors are wired");

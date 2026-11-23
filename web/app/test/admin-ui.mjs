// Static wiring checks for the admin users module: the create and reset forms
// exist, the reset form posts to the role-gated endpoint, and the password is
// a masked field rather than a browser prompt.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";

const root = process.cwd();
const source = await readFile(path.join(root, "web/app/admin/users.js"), "utf8");
const css = await readFile(path.join(root, "web/app/app.css"), "utf8");

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

check("create and reset forms expose the expected fields", () => {
  assert.match(source, /id="admin-create-user"/);
  assert.match(source, /id="admin-reset-target"/);
  assert.match(source, /id="admin-reset-password" type="password"/);
  assert.match(source, /id="admin-reset-mfa"/);
});

check("the reset form posts to the admin password endpoint", () => {
  assert.match(source, /\/api\/v1\/admin\/users\/\$\{id\}\/password/);
  assert.match(source, /reset_mfa: resetMfa\.checked/);
});

check("the reset action opens the form instead of a browser prompt", () => {
  assert.doesNotMatch(source, /window\.prompt/);
  assert.match(source, /details\.open = true/);
  assert.match(source, /resetPassword\.focus\(\)/);
});

check("the reset target excludes the signed-in administrator", () => {
  assert.match(source, /users\.filter\(\(u\) => u\.id !== me\)/);
});

check("the rotation action patches must_change_password", () => {
  assert.match(source, /data-action="rotation"/);
  assert.match(source, /must_change_password: !user\.must_change_password/);
});

check("admin inputs stop at the server limits", () => {
  assert.match(source, /id="admin-create-email"[^>]*maxlength="254"/);
  assert.match(source, /id="admin-create-name"[^>]*maxlength="100"/);
  assert.match(source, /id="admin-create-password"[^>]*maxlength="128"/);
  assert.match(source, /id="admin-reset-password"[^>]*maxlength="128"/);
});

check("the reset form is styled like the create form", () => {
  assert.match(css, /\.admin-create, \.admin-reset/);
  assert.match(css, /\.admin-reset label\.admin-reset-mfa/);
});

console.log(`admin UI checks passed: ${passed}`);

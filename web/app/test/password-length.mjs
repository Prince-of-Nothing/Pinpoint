// Password minimum: the web and admin forms must count Unicode characters
// like the server, not UTF-16 units like HTML minlength.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const app = await readFile("web/app/app.js", "utf8");
const admin = await readFile("web/app/admin/users.js", "utf8");
const page = await readFile("web/app/index.html", "utf8");

// HTML minlength counts UTF-16 units, so it must not gate these inputs.
assert.ok(!page.includes('minlength="8"'), "index.html must not use minlength=8");
assert.ok(!admin.includes('minlength="8"'), "admin forms must not use minlength=8");

for (const [name, source] of [
  ["app.js", app],
  ["admin/users.js", admin],
]) {
  const body = source.match(
    /function passwordTooShort\(value\) \{\s*return ([^;]+);\s*\}/,
  )?.[1];
  assert.ok(body, `${name} must define passwordTooShort`);
  assert.match(body, /Array\.from\(value\)\.length/, `${name} must count characters`);

  const tooShort = new Function("value", `return ${body};`);
  assert.equal(tooShort("😀😀😀😀"), true, "4 emoji = 8 UTF-16 units but 4 characters");
  assert.equal(tooShort("😀😀😀😀😀"), true, "5 emoji are under the minimum");
  assert.equal(tooShort("abcdefg"), true);
  assert.equal(tooShort("😀😀😀😀😀😀😀😀"), false, "8 emoji are 8 characters");
  assert.equal(tooShort("éééééééé"), false);
  assert.equal(tooShort("abcdefgh"), false);
}

assert.match(app, /passwordTooShort\(password\)/, "register must check the password");
assert.match(app, /passwordTooShort\(next\)/, "password change must check the new password");
assert.equal(
  admin.match(/passwordTooShort\(password\)/g)?.length,
  2,
  "admin create and reset must both check the password",
);
for (const source of [app, admin]) {
  assert.ok(
    source.includes("Password must be at least 8 characters."),
    "the message must match the server wording",
  );
}

console.log("PASS  password minimum counts characters in the web and admin forms");

// "/" focuses the place search only when the user is not typing, not using a
// modifier, and no drawer/sheet/dialog is open.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { shouldFocusSearch } from "../search-shortcut.js";

const base = { key: "/", inField: false, modifier: false, blocked: false };

assert.equal(shouldFocusSearch(base), true, "bare / focuses search");
assert.equal(
  shouldFocusSearch({ ...base, inField: true }),
  false,
  "typing / in a field stays in the field",
);
assert.equal(
  shouldFocusSearch({ ...base, modifier: true }),
  false,
  "modified / is left to the browser",
);
assert.equal(
  shouldFocusSearch({ ...base, blocked: true }),
  false,
  "open drawer/sheet/dialog keeps focus",
);
assert.equal(
  shouldFocusSearch({ ...base, key: "?" }),
  false,
  "shift+/ produces ? and must not focus",
);
assert.equal(
  shouldFocusSearch({ ...base, key: "s" }),
  false,
  "other keys are ignored",
);

const page = await readFile("web/app/index.html", "utf8");
const input = page.match(/<input[^>]*id="search-input"[^>]*>/s)?.[0] ?? "";
assert.ok(input, "search input is missing");
assert.ok(
  input.includes('title="Press / to search"'),
  "the search input needs the title hint",
);
assert.ok(
  input.includes('aria-keyshortcuts="/"'),
  "the search input needs aria-keyshortcuts",
);

console.log("PASS  search shortcut truth table and hint");

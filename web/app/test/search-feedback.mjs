// Place search must tell a failed request apart from an empty answer: an
// unreachable server or a lost connection shows the reason and a Retry button
// instead of "No places found". Runs the real nav.js search functions against
// a minimal fake DOM.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const source = await readFile("web/app/nav.js", "utf8");

function extract(signature) {
  const start = source.indexOf(signature);
  assert.ok(start > 0, `missing ${signature}`);
  const rest = source.slice(start);
  const close = rest.match(/\r?\n {2}\}\r?\n/);
  assert.ok(close, `${signature} must end with a closing brace at two spaces`);
  return rest.slice(0, close.index + close[0].length);
}

const body = [
  "async function runSearch(query)",
  "function searchErrorText(err)",
  "function renderSearchNote(text",
  "function renderSearchError(text, query)",
  "function renderResults(items",
]
  .map(extract)
  .join("\n");

assert.doesNotMatch(source, /No places found near here/, "the bias is not a radius; do not say 'near here'");

class FakeNode {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.attrs = {};
    this.dataset = {};
    this.listeners = {};
    this.className = "";
    this.textContent = "";
  }
  set innerHTML(value) {
    assert.equal(value, "", "results must be built with DOM nodes, not markup");
    this.children = [];
  }
  appendChild(child) {
    this.children.push(child);
    return child;
  }
  append(...nodes) {
    this.children.push(...nodes);
  }
  setAttribute(name, value) {
    this.attrs[name] = value;
  }
  addEventListener(type, fn) {
    this.listeners[type] = fn;
  }
  get text() {
    return [this.textContent, ...this.children.map((c) => c.text)].join("");
  }
}

function harness({ online = true, fetchImpl, offline = null }) {
  const results = new FakeNode("div");
  const calls = { fetch: 0, shown: [] };
  const doc = { createElement: (tag) => new FakeNode(tag) };
  const fetchStub = async (url, init) => {
    calls.fetch += 1;
    return fetchImpl(url, init, calls.fetch);
  };
  const make = new Function(
    "MIN_QUERY", "setStatus", "mapApi", "getOfflineSearch", "toSearchPlace", "fetch",
    "navigator", "results", "showResults", "document", "formatDistance",
    `let searchController = null; let currentResults = [];\n${body}\nreturn { runSearch, current: () => currentResults };`,
  );
  const api = make(
    3,
    () => {},
    { map: { getCenter: () => ({ lat: 46.88, lng: 29.1 }) } },
    async () => offline,
    (r) => ({ short_name: r.name, display_name: r.name, latitude: r.lat, longitude: r.lon }),
    fetchStub,
    { onLine: online },
    results,
    (open) => calls.shown.push(open),
    doc,
    (m) => `${m} m`,
  );
  return { ...api, results, calls };
}

const json = (status, value) => ({ ok: status >= 200 && status < 300, status, json: async () => value });
const only = (h) => {
  assert.equal(h.results.children.length, 1);
  return h.results.children[0];
};

// Server unreachable: reason + Retry, not "No places found"; Retry searches again.
{
  const h = harness({ fetchImpl: () => Promise.reject(new TypeError("Failed to fetch")) });
  await h.runSearch("Albina");
  const note = only(h);
  assert.match(note.className, /search-error/);
  assert.equal(note.attrs.role, "alert");
  assert.match(note.text, /Can't reach the Pinpoint server/);
  assert.doesNotMatch(note.text, /No places/);
  const retry = note.children.find((c) => c.tag === "button");
  assert.ok(retry && /Retry/.test(retry.textContent), "missing Retry button");
  await retry.listeners.click();
  assert.equal(h.calls.fetch, 2, "Retry must run the search again");
  assert.equal(h.calls.shown.at(-1), true);
}

// Browser offline: say so.
{
  const h = harness({ online: false, fetchImpl: () => Promise.reject(new TypeError("Failed to fetch")) });
  await h.runSearch("Albina");
  assert.match(only(h).text, /You're offline/);
}

// Server error: its message is shown with Retry.
{
  const h = harness({ fetchImpl: () => json(429, { error: { message: "Too many searches. Try again in 30 s." } }) });
  await h.runSearch("Albina");
  assert.match(only(h).text, /Too many searches/);
  assert.match(only(h).className, /search-error/);
}

// A real empty answer names the query and is not an error.
{
  const h = harness({ fetchImpl: () => json(200, { results: [] }) });
  await h.runSearch("Zzyzx");
  const note = only(h);
  assert.equal(note.textContent, "No places match “Zzyzx”.");
  assert.doesNotMatch(note.className, /search-error/);
  assert.deepEqual(h.current(), []);
}

// Results render as buttons and become the current results.
{
  const place = { short_name: "Albina", display_name: "Albina, Cimișlia", distance_m: 40 };
  const h = harness({ fetchImpl: () => json(200, { results: [place] }) });
  await h.runSearch("Albina");
  const btn = only(h);
  assert.equal(btn.tag, "button");
  assert.equal(btn.className, "result");
  assert.deepEqual(h.current(), [place]);
}

// Installed offline index: no network, and the empty text says where it looked.
{
  const h = harness({
    offline: { search: () => [] },
    fetchImpl: () => assert.fail("offline search must not hit the network"),
  });
  await h.runSearch("Paris");
  assert.equal(only(h).textContent, "No places in the offline map match “Paris”.");
}

// A superseded request that fails later must not replace the newer answer.
{
  let rejectFirst;
  const h = harness({
    fetchImpl: (_url, _init, n) =>
      n === 1
        ? new Promise((_, reject) => (rejectFirst = reject))
        : json(200, { results: [{ short_name: "Mereni", display_name: "Mereni" }] }),
  });
  const first = h.runSearch("Merenii");
  await new Promise((r) => setTimeout(r, 0));
  await h.runSearch("Mereni");
  rejectFirst(new TypeError("Failed to fetch"));
  await first;
  assert.equal(only(h).tag, "button", "the stale failure must not overwrite the newer results");
}

console.log("PASS  search shows failures with a reason and Retry, and honest empty results");

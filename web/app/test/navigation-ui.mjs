// The tab bar must switch real screens instead of scrolling one long drawer,
// and the map must expose its basemap and layer controls.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const page = await readFile("web/app/index.html", "utf8");
const app = await readFile("web/app/app.js", "utf8");
const css = await readFile("web/app/app.css", "utf8");
const map = await readFile("web/app/map.js", "utf8");
const { lineLengthMeters } = await import(
  pathToFileURL(path.resolve("web/app/measure.js")).href
);

const screens = ["friends", "sharing", "today", "chats", "account"];

const tabs = [...page.matchAll(/data-tab="([a-z]+)"/g)].map((m) => m[1]);
assert.deepEqual(
  [...new Set(tabs)].sort(),
  ["account", "chats", "friends", "map", "sharing", "today"],
  "the tab bar must offer map plus the five sections",
);

assert.match(page, /id="social-drawer"[^>]*data-screen="friends"/);
const sections = [...page.matchAll(/data-sections="([^"]+)"/g)].map((m) => m[1]);
for (const name of screens) {
  assert.ok(sections.includes(name), `no card is assigned to ${name}`);
  assert.ok(
    css.includes(
      `.drawer[data-screen="${name}"] .card:not([data-sections~="${name}"])`,
    ),
    `missing the ${name} screen rule in app.css`,
  );
  assert.match(
    app,
    new RegExp(`${name}: \\{ eyebrow:`),
    `no screen title for ${name}`,
  );
}

assert.match(app, /function setScreen\(name\)/);
assert.match(app, /drawer\.dataset\.screen = name/);

for (const id of [
  "map-layers-toggle",
  "map-controls",
  "basemap-offline",
  "basemap-streets",
  "layer-friends",
  "layer-trail",
  "layer-notes",
]) {
  assert.ok(page.includes(`id="${id}"`), `missing #${id}`);
}

assert.match(app, /state\.layers\.friends \? state\.locations : \[\]/);
assert.match(app, /state\.layers\.notes \? state\.notes : \[\]/);
for (const kind of ["offline", "streets", "satellite"]) {
  assert.match(app, new RegExp(`chooseBasemap\\("${kind}"\\)`), `no ${kind} map type`);
}
assert.match(
  app,
  /if \(!event\.target\.checked && mapApi\?\.getLayers\?\.\(\)\.satellite\)/,
  "turning the online map off must drop satellite imagery too",
);
assert.match(app, /setTrail\(event\.target\.checked\)/);
assert.match(app, /pinpoint:theme/);
assert.match(
  app,
  /const theme = document\.documentElement\.dataset\.theme/,
  "the map must start with the saved theme",
);

// Desktop: the section cards are a right column, not a bottom sheet, and the
// map chrome must not sit under them.
assert.match(css, /@media \(min-width: 981px\)/);
assert.match(
  css,
  /body:has\(\.drawer\.open\) \.app-shell \.map \{/,
  "the open drawer must shrink the map instead of overlaying it",
);
assert.match(
  css,
  /\.app-shell \.maplibregl-ctrl-bottom-right/,
  "map controls need a mobile offset above the search and tab bars",
);
assert.match(
  app,
  /matchMedia\?\.\("\(min-width: 981px\)"\)/,
  "desktop sign-in must open the section column",
);

// The drawer itself must move from the bottom sheet to the right column at the
// same breakpoint, not just the map.
const wideCssStart = css.indexOf("@media (min-width: 981px)");
assert.ok(wideCssStart > 0, "missing the 981 px desktop breakpoint");
const mobileDrawer =
  css.slice(0, wideCssStart).match(/\.drawer \{[^}]*\}/s)?.[0] ?? "";
const wideDrawer =
  css.slice(wideCssStart).match(/\.drawer \{[^}]*\}/s)?.[0] ?? "";
assert.match(
  mobileDrawer,
  /bottom: calc\(4\.6rem \+ var\(--safe-bottom\)\);/,
  "the narrow drawer must stay bottom-anchored",
);
assert.match(wideDrawer, /left: auto;/, "the wide drawer must leave the left edge");
assert.match(wideDrawer, /right: 0;/, "the wide drawer must pin to the right edge");
assert.match(wideDrawer, /width: 380px;/, "the wide drawer must be a 380 px column");
assert.match(
  wideDrawer,
  /border-left: 1px solid var\(--border\);/,
  "the wide drawer must draw a column divider",
);
assert.doesNotMatch(
  wideDrawer,
  /bottom: calc\(4\.6rem/,
  "the wide drawer must not keep the bottom-sheet anchor",
);
// Overlays centred on the map must centre on the part the column leaves
// visible, and the navigation banner must not cover the top search bar.
const wideCss = css.slice(wideCssStart);
assert.match(
  wideCss,
  /body:has\(\.drawer\.open\) \.map-empty-state \{ right: 380px; \}/,
  "the empty-map card must stay clear of the open column",
);
for (const overlay of ["\\.nav-banner", "#status"]) {
  const rule =
    wideCss.match(new RegExp(`body:has\\(\\.drawer\\.open\\) ${overlay} \\{[^}]*\\}`, "s"))?.[0] ?? "";
  assert.match(
    rule,
    /left: calc\(\(100% - 380px\) \/ 2\);/,
    `${overlay} must centre on the visible map while the column is open`,
  );
  assert.match(rule, /380px - 2rem/, `${overlay} must not reach under the column`);
}
assert.match(
  wideCss,
  /\.nav-banner \{ top: 8rem; \}/,
  "the desktop navigation banner must sit below the top search bar",
);
assert.match(map, /showCompass: true/);
assert.match(map, /visualizePitch: true/);
assert.match(map, /new maplibregl\.FullscreenControl\(\)/);

// Map actions and the measure tool.
for (const id of ["map-menu", "measure-hud", "measure-text", "measure-undo", "measure-clear"]) {
  assert.ok(page.includes(`id="${id}"`), `missing #${id}`);
}
for (const action of ["from", "to", "measure", "note", "copy"]) {
  assert.ok(
    page.includes(`data-map-action="${action}"`),
    `missing map action ${action}`,
  );
}
assert.match(app, /onContextMenu: \(point\) => showMapMenu\(point\)/);
assert.match(app, /function startMeasure\(point\)/);
assert.match(app, /mapApi\?\.setMeasurePoints\(measurePoints\)/);
assert.match(map, /container\.addEventListener\("contextmenu", handleContextMenu\)/);
assert.match(map, /function setMeasurePoints\(points\)/);
assert.match(map, /id: MEASURE_LAYER/);
assert.match(map, /function applyFriendLabels/);
assert.match(map, /element\.className = "friend-label"/);
assert.match(css, /\.friend-label \{/);

const oneSegment = lineLengthMeters([[0, 0], [0, 0.001]]);
assert.ok(oneSegment > 100 && oneSegment < 115, `one segment was ${oneSegment} m`);
const twoSegments = lineLengthMeters([[0, 0], [0, 0.001], [0, 0.002]]);
assert.ok(Math.abs(twoSegments - oneSegment * 2) < 1e-6);
assert.equal(lineLengthMeters([]), 0);
assert.equal(lineLengthMeters([["bad", 0]]), 0);

console.log("PASS  tabs switch real screens and the map exposes basemap and layer controls");

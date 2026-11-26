// Pure checks for the sharing consent helpers plus static checks that the UI
// keeps the confirmation, pause-only, expiry-preserving and DELETE paths.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = process.cwd();
const sharingUrl = pathToFileURL(path.resolve("web/app/sharing.js")).href;
const {
  avatarHue,
  expiryText,
  freshnessText,
  friendName,
  friendRowModel,
  compactAge,
  initials,
  initSharing,
  isLive,
  liveStripModel,
  locationSentence,
  shareSentence,
  precisionLabel,
  shareRequestBody,
  shareRowModel,
  shareStatus,
  shareWidens,
} = await import(sharingUrl);
const sharingSource = await readFile(path.join(root, "web/app/sharing.js"), "utf8");
const appSource = await readFile(path.join(root, "web/app/app.js"), "utf8");
const pageSource = await readFile(path.join(root, "web/app/index.html"), "utf8");

let passed = 0;
async function check(name, fn) {
  try {
    await fn();
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

const now = new Date("2026-11-17T12:00:00Z");
const at = (hours) => new Date(now.getTime() + hours * 3_600_000).toISOString();
const past = "2000-01-01T00:00:00Z";
const future = "2099-01-01T00:00:00Z";

function share(overrides = {}) {
  return {
    id: 1,
    friendship_id: 7,
    owner_id: 1,
    direction: "to_friend",
    precision_level: "exact",
    is_active: true,
    expires_at: null,
    ...overrides,
  };
}

function form(overrides = {}) {
  return {
    friendshipId: 7,
    enabled: true,
    precision: "exact",
    duration: "keep",
    ...overrides,
  };
}

await check("friendName resolves the other side of the friendship", () => {
  assert.equal(friendName(outgoing, 1), "Alice");
  assert.equal(friendName(incoming, 1), "Bob");
});

await check("friendName falls back to the friendship id", () => {
  assert.equal(friendName({ id: 12 }, 1), "friendship #12");
  assert.equal(friendName(null, 1), "this friend");
});

await check("precisionLabel uses readable labels", () => {
  assert.equal(precisionLabel("exact"), "Exact");
  assert.equal(precisionLabel("approximate"), "Approximate");
  assert.equal(precisionLabel("city_only"), "City only");
  assert.equal(precisionLabel("unexpected"), "unexpected");
});

await check("shareStatus separates active, paused and expired", () => {
  assert.equal(shareStatus({ is_active: true, expires_at: future }, now), "active");
  assert.equal(shareStatus({ is_active: false, expires_at: future }, now), "paused");
  assert.equal(shareStatus({ is_active: true, expires_at: past }, now), "expired");
  assert.equal(shareStatus({ is_active: true }, now), "active");
  assert.equal(shareStatus({ is_active: false }, now), "paused");
});

await check("expiryText distinguishes expiry from no expiry", () => {
  assert.equal(expiryText({}, now), "no expiry");
  assert.match(expiryText({ expires_at: at(1) }, now), /^expires /);
  assert.match(expiryText({ expires_at: past }, now), /^expired /);
});

await check("freshnessText labels the age of a friend's location", () => {
  const ago = (ms) => new Date(now.getTime() - ms).toISOString();
  assert.equal(freshnessText(ago(20_000), now), "just now");
  assert.equal(freshnessText(ago(5 * 60_000), now), "5m ago");
  assert.equal(freshnessText(ago(3 * 3_600_000), now), "3h ago");
  assert.equal(freshnessText(ago(2 * 86_400_000), now), "2d ago");
  assert.equal(freshnessText("not a date", now), "unknown");
  // A clock-skewed future timestamp is not reported as negative age.
  assert.equal(freshnessText(ago(-60_000), now), "just now");
});

await check("keep preserves the stored expiry and mutual direction", () => {
  const body = shareRequestBody(
    share({ direction: "mutual", precision_level: "approximate", expires_at: future }),
    form({ precision: "exact", duration: "keep" }),
  );
  assert.deepEqual(body, {
    friendship_id: 7,
    direction: "mutual",
    precision_level: "exact",
    is_active: true,
    expires_at: future,
  });
  assert.ok(!("duration_hours" in body));
});

await check("new shares use to_friend and the chosen duration", () => {
  const oneHour = shareRequestBody(null, form({ precision: "approximate", duration: "1" }));
  assert.deepEqual(oneHour, {
    friendship_id: 7,
    direction: "to_friend",
    precision_level: "approximate",
    is_active: true,
    duration_hours: 1,
  });
  const sevenDays = shareRequestBody(
    share({ expires_at: future }),
    form({ precision: "exact", duration: "168" }),
  );
  assert.equal(sevenDays.duration_hours, 168);
  assert.ok(!("expires_at" in sevenDays));
});

await check("expired unchecked sends nothing; expired re-grant defaults to 24h", () => {
  const expired = share({ is_active: false, expires_at: past });
  assert.equal(shareRequestBody(expired, form({ enabled: false, duration: "24" })), null);
  const regrant = shareRequestBody(expired, form({ enabled: true, duration: "24" }));
  assert.equal(regrant.is_active, true);
  assert.equal(regrant.duration_hours, 24);
});

await check("pause sends only is_active and never precision or expiry", () => {
  const body = shareRequestBody(
    share({ precision_level: "approximate", expires_at: future }),
    form({ enabled: false, precision: "city_only", duration: "168" }),
  );
  assert.deepEqual(body, { is_active: false });
});

await check("precision widening ranks city_only < approximate < exact", () => {
  const cityOnly = share({ precision_level: "city_only", expires_at: future });
  assert.equal(
    shareWidens(cityOnly, form({ precision: "approximate", duration: "keep" }), now),
    true,
  );
  assert.equal(
    shareWidens(
      share({ precision_level: "approximate", expires_at: future }),
      form({ precision: "exact", duration: "keep" }),
      now,
    ),
    true,
  );
  assert.equal(
    shareWidens(
      share({ precision_level: "exact", expires_at: future }),
      form({ precision: "approximate", duration: "keep" }),
      now,
    ),
    false,
  );
});

await check("a later expiry counts as a widen even with the same precision", () => {
  assert.equal(
    shareWidens(
      share({ precision_level: "exact", expires_at: at(1) }),
      form({ precision: "exact", duration: "168" }),
      now,
    ),
    true,
  );
  assert.equal(
    shareWidens(
      share({ precision_level: "exact", expires_at: at(168) }),
      form({ precision: "exact", duration: "1" }),
      now,
    ),
    false,
  );
  assert.equal(
    shareWidens(
      share({ precision_level: "exact", expires_at: at(1) }),
      form({ precision: "exact", duration: "0" }),
      now,
    ),
    true,
  );
  assert.equal(
    shareWidens(
      share({ precision_level: "exact", expires_at: at(1) }),
      form({ precision: "exact", duration: "keep" }),
      now,
    ),
    false,
  );
});

await check("widen detection ignores the checkbox state", () => {
  const cityOnly = share({ precision_level: "city_only", expires_at: future });
  assert.equal(
    shareWidens(cityOnly, form({ enabled: false, precision: "approximate" }), now),
    true,
  );
});

await check("shareRowModel marks grants, states and actions", () => {
  const active = shareRowModel(
    share({ precision_level: "approximate", expires_at: at(168) }),
    now,
  );
  assert.equal(active.isGrant, true);
  assert.equal(active.status, "active");
  assert.equal(active.precision, "Approximate");
  assert.match(active.expiry, /^expires /);
  assert.equal(active.canPauseResume, true);
  assert.equal(active.canShareAgain, false);

  const paused = shareRowModel(share({ is_active: false, expires_at: at(1) }), now);
  assert.equal(paused.status, "paused");
  assert.equal(paused.canPauseResume, true);

  const expired = shareRowModel(share({ is_active: false, expires_at: past }), now);
  assert.equal(expired.status, "expired");
  assert.equal(expired.canPauseResume, false);
  assert.equal(expired.canShareAgain, true);

  const incomingRow = shareRowModel(share({ direction: "from_friend" }), now);
  assert.equal(incomingRow.isGrant, false);
});

await check("a canceled confirm sends no request", async () => {
  const ids = [
    "share-sheet",
    "share-summary",
    "share-title",
    "share-subtitle",
    "share-incoming",
    "share-enabled",
    "share-precision",
    "share-label",
    "share-duration",
    "share-revoke",
    "share-save",
    "share-close",
    "place-sheet",
    "route-panel",
  ];
  const elements = new Map();
  function makeElement(id) {
    const element = {
      id,
      value: "",
      checked: false,
      textContent: "",
      firstChild: null,
      listeners: new Map(),
      classList: { add() {}, remove() {}, toggle() {} },
      addEventListener(type, fn) {
        element.listeners.set(type, [...(element.listeners.get(type) || []), fn]);
      },
      querySelector() {
        return null;
      },
      insertBefore(node) {
        element.firstChild = node;
      },
      appendChild(node) {
        return node;
      },
    };
    return element;
  }
  for (const id of ids) elements.set(id, makeElement(id));

  const calls = [];
  const active = share({ precision_level: "approximate", expires_at: future });
  const api = async (endpoint, options = {}) => {
    calls.push({ endpoint, options });
    if (endpoint === "/api/v1/locations/shares" && !options.method) return [active];
    throw new Error(`unexpected request: ${endpoint} ${options.method || "GET"}`);
  };
  let asked = "";
  globalThis.document = {
    getElementById: (id) => elements.get(id),
    createElement: (tag) => makeElement(tag),
  };
  globalThis.window = {
    confirm(text) {
      asked = text;
      return false;
    },
  };
  try {
    const sharing = initSharing({
      api,
      setStatus() {},
      getUser: () => ({ id: 1 }),
      onChanged() {
        throw new Error("a canceled confirm must not change anything");
      },
    });
    await sharing.open(outgoing);
    elements.get("share-enabled").checked = true;
    elements.get("share-precision").value = "exact";
    await elements.get("share-save").listeners.get("click")[0]();
    assert.equal(calls.length, 1, "only the initial GET should be sent");
    assert.match(asked, /^Share exact location with Alice until .+\?$/);
  } finally {
    delete globalThis.document;
    delete globalThis.window;
  }
});

await check("sharing sheet confirms widens, keeps expiry and revokes with DELETE", () => {
  assert.match(sharingSource, /window\.confirm\(confirmText\(\)\)/);
  assert.match(sharingSource, /select\.querySelector\('option\[value="keep"\]'\)/);
  assert.match(sharingSource, /body\.expires_at = current\.expires_at/);
  assert.match(sharingSource, /return \{ is_active: false \}/);
  assert.match(sharingSource, /method: "DELETE"/);
  assert.match(sharingSource, /Sharing revoked\./);
});

await check("shares card uses the consent heading, hint and live summary", () => {
  assert.match(pageSource, /<h2>Who can see me<\/h2>/);
  assert.match(pageSource, /id="share-summary" class="muted" aria-live="polite"/);
  assert.match(pageSource, /pause or revoke any time/i);
});

await check("share rows skip from_friend grants and only pause live rows", () => {
  assert.match(appSource, /renderShares\(Array\.isArray\(shares\) \? shares : \[\], friends\)/);
  assert.match(appSource, /shareRowModel\(share\)/);
  assert.match(appSource, /\.filter\(\(\{ model \}\) => model\.isGrant\)/);
  assert.match(appSource, /model\.canShareAgain/);
  assert.match(appSource, /revokeShare\(s\.id\)/);
  assert.match(appSource, /method: "DELETE"/);
});

await check("a share label is sent, shown in the row and editable", () => {
  const body = shareRequestBody(
    share({ expires_at: future }),
    form({ label: "  Work trip  " }),
  );
  assert.equal(body.label, "Work trip");
  assert.ok(!("label" in shareRequestBody(share(), form({ label: "   " }))));
  assert.equal(shareRowModel(share({ label: "Work trip" }), now).label, "Work trip");
  assert.match(pageSource, /id="share-label"/);
  assert.match(sharingSource, /share-label"\)\.value = current\?\.label/);
  // The row shows the label through shareSentence ("Work trip: sharing …").
  assert.match(appSource, /personText\(name, shareSentence\(s\)\)/);
  assert.match(shareSentence(share({ label: "Work trip", expires_at: future }), now), /^Work trip: /);
});

await check("clearing a label removes it only for an existing labeled share", () => {
  const cleared = shareRequestBody(
    share({ label: "Work trip", expires_at: future }),
    form({ label: "   " }),
  );
  assert.equal(cleared.label, "");
  assert.ok(!("label" in shareRequestBody(null, form({ label: "" }))));
  assert.ok(!("label" in shareRequestBody(share({ expires_at: future }), form({ label: "" }))));
});

await check("avatars use initials and a stable per-user hue", () => {
  assert.equal(initials("Bogdan Friend"), "BF");
  assert.equal(initials("bright-badger-2"), "BB");
  assert.equal(initials("@ana"), "A");
  assert.equal(initials("Élise  van der Berg"), "ÉB");
  assert.equal(initials(""), "?");
  assert.equal(avatarHue(42), avatarHue(42));
  assert.ok(avatarHue(42) >= 0 && avatarHue(42) < 360);
  assert.notEqual(avatarHue(1), avatarHue(2));
});

await check("friend rows show a person and a status, not requester → receiver", () => {
  const now = Date.parse("2026-11-26T12:00:00Z");
  const base = { id: 9, requester_id: 5, receiver_id: 1, requester_name: "bright-badger-2", receiver_name: "brave-badger-1" };
  const incoming = friendRowModel({ ...base, status: "pending" }, 1, [], now);
  assert.equal(incoming.kind, "incoming");
  assert.equal(incoming.status, "Wants to be friends");
  assert.equal(incoming.name, "bright-badger-2");
  assert.equal(incoming.handle, "", "no duplicate @handle when the name is the handle");
  assert.equal(incoming.friendId, 5);

  const outgoing = friendRowModel({ ...base, requester_id: 1, receiver_id: 5, status: "pending" }, 1, [], now);
  assert.equal(outgoing.kind, "outgoing");
  assert.equal(outgoing.status, "Request sent");
  assert.equal(outgoing.name, "brave-badger-1");

  const location = { user_id: 5, user_name: "Bogdan Friend", updated_at: "2026-11-26T11:55:00Z" };
  const friend = friendRowModel({ ...base, status: "accepted" }, 1, [location], now);
  assert.equal(friend.kind, "friend");
  assert.equal(friend.name, "Bogdan Friend");
  assert.equal(friend.handle, "@bright-badger-2");
  assert.equal(friend.status, "Sharing their location, updated 5m ago");
  assert.equal(friend.initials, "BF");
  assert.equal(friend.location, location);

  assert.equal(friendRowModel({ ...base, status: "accepted" }, 1, [], now).status, "Friends");
  assert.equal(friendRowModel({ ...base, status: "rejected" }, 1, [], now).kind, "rejected");
});

await check("people lists are built without raw coordinates or the arrow pair", () => {
  assert.doesNotMatch(appSource, /requester_name\)\} → \$\{escapeHtml\(f\.receiver_name\)/);
  assert.match(appSource, /friendRowModel\(f, me, state\.locations\)/);
  assert.doesNotMatch(appSource, /li\.textContent = `\$\{loc\.user_name\} · \$\{Number\(loc\.latitude\)/);
  assert.match(appSource, /rowButton\("Show on map", \(\) => showFriendOnMap\(loc\)\)/);
  assert.match(pageSource, /id="publish-gps"/, "missing the GPS share button");
  // A real button (not <summary>) so the narrow drawer's focus trap reaches it.
  assert.match(
    pageSource,
    /<button type="button" id="publish-manual-toggle"[^>]*aria-expanded="false" aria-controls="publish-form">Enter coordinates instead<\/button>\s*<form id="publish-form" class="publish-manual hidden">/,
    "coordinates must fold behind a toggle button",
  );
  assert.match(appSource, /\$\("publish-manual-toggle"\)\?\.addEventListener\("click"/);
  assert.match(appSource, /publish\(fix\.lat, fix\.lon, publishAccuracy\(fix\.accuracy\)\)/);
});

await check("shares and locations read as sentences, not dot-joined fields", () => {
  const now = Date.parse("2026-11-26T12:00:00Z");
  const until = "2026-11-27T01:40:00Z";
  const when = new Date(until).toLocaleString([], { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
  const active = { is_active: true, precision_level: "exact", expires_at: until };
  assert.equal(shareSentence(active, now), `Sharing your exact location until ${when}`);
  assert.equal(shareSentence({ ...active, label: "Weekend" }, now), `Weekend: sharing your exact location until ${when}`);
  assert.equal(
    shareSentence({ ...active, is_active: false, precision_level: "city_only" }, now),
    `Paused, would share your city until ${when}`,
  );
  assert.equal(shareSentence({ is_active: true, precision_level: "approximate", expires_at: null }, now), "Sharing your approximate location with no end time");
  assert.equal(shareSentence({ ...active, expires_at: "2026-11-25T10:00:00Z" }, now).startsWith("Ended "), true);
  assert.equal(
    locationSentence({ precision_level: "approximate", updated_at: "2026-11-26T11:58:00Z" }, now),
    "Approximate location, updated 2m ago",
  );
  assert.doesNotMatch(appSource, /status\.join\(" · "\)/, "no dot-joined status strings");
});

await check("the Sharing with you strip lists live friends first and pulses only on change", async () => {
  const now = Date.parse("2026-11-26T12:00:00Z");
  const at = (min) => new Date(now - min * 60_000).toISOString();
  assert.equal(isLive(at(14), now), true);
  assert.equal(isLive(at(16), now), false);
  assert.equal(isLive("bad", now), false);
  assert.equal(compactAge(at(0.2), now), "now");
  assert.equal(compactAge(at(5), now), "5 min");
  assert.equal(compactAge(at(180), now), "3 h");
  assert.equal(compactAge(at(2 * 1440), now), "2 d");

  const bogdan = { user_id: 5, user_name: "Bogdan Friend", updated_at: at(2) };
  const carla = { user_id: 6, user_name: "Carla Popescu", updated_at: at(200) };
  const first = liveStripModel([carla, bogdan], new Map(), now);
  assert.deepEqual(first.map((e) => e.name), ["Bogdan Friend", "Carla Popescu"], "freshest first");
  assert.deepEqual(first.map((e) => e.live), [true, false]);
  assert.deepEqual(first.map((e) => e.initials), ["BF", "CP"]);
  assert.ok(first.every((e) => !e.pulse), "no pulse on first render");

  const seen = new Map([[5, bogdan.updated_at], [6, carla.updated_at]]);
  const moved = { ...bogdan, updated_at: at(0) };
  const next = liveStripModel([moved, carla], seen, now);
  assert.equal(next.find((e) => e.userId === 5).pulse, true, "a changed location pulses once");
  assert.equal(next.find((e) => e.userId === 6).pulse, false);
  assert.deepEqual(liveStripModel([], seen, now), []);

  const css = await readFile(path.join(root, "web/app/app.css"), "utf8");
  assert.match(pageSource, /<section id="live-strip" class="live-strip hidden"/);
  assert.match(appSource, /function renderLocations\(items\) \{\s*renderLiveStrip\(items\);/);
  assert.match(appSource, /button\.addEventListener\("click", \(\) => showFriendOnMap\(entry\.location\)\);/);
  assert.match(appSource, /liveSeen\.clear\(\);\s*show\(\$\("live-strip"\), false\);/, "sign-out forgets the strip");
  assert.match(css, /\.live-person\.is-live \.avatar \{ box-shadow: [^}]*var\(--live\)/, "live ring uses the Sunflower token");
  assert.match(css, /prefers-reduced-motion: reduce\) \{\s*\.live-person\.pulse \.avatar \{ animation: none; \}/);
});

console.log(`sharing UI checks passed: ${passed}`);

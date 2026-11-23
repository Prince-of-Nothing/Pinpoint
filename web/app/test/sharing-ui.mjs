// Pure checks for the sharing consent helpers plus static checks that the UI
// keeps the confirmation, pause-only, expiry-preserving and DELETE paths.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = process.cwd();
const sharingUrl = pathToFileURL(path.resolve("web/app/sharing.js")).href;
const {
  expiryText,
  friendName,
  initSharing,
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
  assert.match(appSource, /model\.label/);
});

console.log(`sharing UI checks passed: ${passed}`);

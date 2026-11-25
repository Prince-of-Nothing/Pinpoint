// The 30-second friends/sharing poll: friend requests, shares and friend
// locations must refresh while the page is visible, without stealing keyboard
// focus, wiping the screen when offline, or rendering another account's data.
// Runs the real pollSocial body from app.js against stubbed DOM and API.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const source = await readFile("web/app/app.js", "utf8");

assert.match(source, /const SOCIAL_POLL_MS = 30_000;/, "poll every 30 s like the native client");
assert.match(source, /setInterval\(pollSocial, SOCIAL_POLL_MS\);/, "the poll must be scheduled");
assert.match(
  source,
  /addEventListener\("visibilitychange", \(\) => \{\s*if \(!document\.hidden\) pollAll\(\);/,
  "returning to the tab must poll at once",
);
assert.match(source, /window\.addEventListener\("online", pollAll\);/, "reconnecting must poll at once");
assert.match(
  source,
  /if \(name === "friends" \|\| name === "sharing"\) pollSocial\(\);/,
  "opening Friends or Sharing must poll at once",
);

const start = source.indexOf("async function pollSocial()");
assert.ok(start > 0, "missing pollSocial");
const rest = source.slice(start);
const close = rest.match(/\r?\n\}\r?\n/);
assert.ok(close, "pollSocial must end with a top-level closing brace");
const body = rest.slice(0, close.index + close[0].length);

function harness({ hidden = false, focusIn = null, fail = false, friendsLayer = true } = {}) {
  const calls = [];
  const rendered = { friends: null, shares: null, locations: null, markers: null };
  const state = { user: { id: 7 }, locations: ["old"], layers: { friends: friendsLayer } };
  const lists = {};
  for (const id of ["friends-list", "shares-list"]) {
    lists[id] = { contains: (el) => el === `inside:${id}` };
  }
  const doc = { hidden, activeElement: focusIn ? `inside:${focusIn}` : "body" };
  const data = {
    "/api/v1/friendships/": [{ id: 1, status: "pending" }],
    "/api/v1/locations/shares": [{ id: 2 }],
    "/api/v1/locations/friends": [{ user_id: 9, latitude: 47, longitude: 28 }],
  };
  let release;
  const gate = new Promise((resolve) => (release = resolve));
  const api = async (path) => {
    calls.push(path);
    await gate;
    if (fail) throw new Error("offline");
    return data[path];
  };
  const pollSocial = new Function(
    "state", "api", "$", "document", "renderFriends", "renderShares", "renderLocations", "mapApi",
    `let socialPollBusy = false;\n${body}\nreturn pollSocial;`,
  )(
    state,
    api,
    (id) => lists[id],
    doc,
    (items) => (rendered.friends = items),
    (items, friends) => (rendered.shares = { items, friends }),
    (items) => (rendered.locations = items),
    { setFriendMarkers: (items) => (rendered.markers = items) },
  );
  return { pollSocial, calls, rendered, state, release };
}

// Hidden tab: no requests at all.
{
  const h = harness({ hidden: true });
  await h.pollSocial();
  assert.deepEqual(h.calls, []);
}

// Visible tab: the three lists and the friend pins refresh.
{
  const h = harness();
  const run = h.pollSocial();
  h.release();
  await run;
  assert.deepEqual(h.calls.sort(), [
    "/api/v1/friendships/",
    "/api/v1/locations/friends",
    "/api/v1/locations/shares",
  ]);
  assert.deepEqual(h.rendered.friends, [{ id: 1, status: "pending" }]);
  assert.deepEqual(h.rendered.shares.friends, h.rendered.friends, "shares need friend names");
  assert.equal(h.state.locations.length, 1);
  assert.equal(h.rendered.markers, h.state.locations);
}

// A second tick while one is in flight is skipped.
{
  const h = harness();
  const first = h.pollSocial();
  await h.pollSocial();
  assert.equal(h.calls.length, 3, "overlapping polls must not double the requests");
  h.release();
  await first;
}

// Keyboard focus in the friends list: leave that list alone, refresh the rest.
{
  const h = harness({ focusIn: "friends-list" });
  const run = h.pollSocial();
  h.release();
  await run;
  assert.equal(h.rendered.friends, null, "focused list must not be rebuilt");
  assert.ok(h.rendered.shares, "other lists still refresh");
  assert.ok(h.rendered.locations);
}

// Offline: keep what is on screen, and the next tick may run again.
{
  const h = harness({ fail: true });
  const run = h.pollSocial();
  h.release();
  await run;
  assert.deepEqual(h.state.locations, ["old"]);
  assert.equal(h.rendered.friends, null);
  assert.equal(h.rendered.markers, null);
  await h.pollSocial();
  assert.equal(h.calls.length, 6, "a failed tick must release the busy flag");
}

// Signed out or switched account mid-request: drop the stale answer.
{
  const h = harness();
  const run = h.pollSocial();
  h.state.user = { id: 8 };
  h.release();
  await run;
  assert.equal(h.rendered.friends, null);
  assert.deepEqual(h.state.locations, ["old"]);
}

// Friends layer off: lists refresh, pins stay hidden.
{
  const h = harness({ friendsLayer: false });
  const run = h.pollSocial();
  h.release();
  await run;
  assert.deepEqual(h.rendered.markers, []);
  assert.equal(h.rendered.locations.length, 1);
}

console.log("PASS  friends/sharing poll refreshes visible data and keeps focus and offline state");

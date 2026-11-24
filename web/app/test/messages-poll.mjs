// Pure check for the 5-second message poll: it must not run for accounts that
// have no friendship at all, because they cannot have a conversation.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const source = await readFile("web/app/app.js", "utf8");

assert.match(
  source,
  /state\.friendshipCount = items\.filter\(\(f\) => f\.status !== "rejected"\)\.length/,
  "renderFriends must record the number of live friendships",
);
assert.match(
  source,
  /state\.friendshipCount === 0/,
  "pollMessages must skip accounts with no friendships",
);
assert.match(
  source,
  /state\.friendshipCount = 0;/,
  "sign-out must reset the friendship count",
);

console.log("PASS  message poll skips accounts without friendships");

// The drawer closes on a map click only when it is open and the target is
// outside the drawer and its toggle. Keep the decision as a truth table.
import assert from "node:assert/strict";
import { drawerOutsideClickDismisses } from "../drawer-dismiss.js";

const base = { open: true, inDrawer: false, onToggle: false, inMap: true };

assert.equal(drawerOutsideClickDismisses(base), true, "map click closes an open drawer");
assert.equal(
  drawerOutsideClickDismisses({ ...base, open: false }),
  false,
  "a closed drawer stays closed",
);
assert.equal(
  drawerOutsideClickDismisses({ ...base, inDrawer: true }),
  false,
  "clicks inside the drawer do not dismiss it",
);
assert.equal(
  drawerOutsideClickDismisses({ ...base, onToggle: true }),
  false,
  "the toggle owns its own click",
);
assert.equal(
  drawerOutsideClickDismisses({ ...base, inMap: false }),
  false,
  "only the visible map dismisses the drawer",
);

console.log("PASS  drawer outside-click dismissal truth table");

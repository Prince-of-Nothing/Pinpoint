# Cache and Permission Model

A cache is a location-bound note, photo, or file with an owner, area, audience,
expiry, and access history.

Access choices:

- named friends;
- members of a trusted group;
- people who enter a user-defined area;
- a public or community cache only when the creator deliberately enables it.

Creators can preview, edit, revoke, expire, delete, and report caches. Visiting a
location never grants access to the visitor's own location.

## Location-locked and public caches

- A cache may unlock when a user's coarse visited-region record intersects the
  creator's defined area, or after a verified visit to that area.
- Eligibility is evaluated by the backend or privacy-preserving device logic; cache
  creators do not receive the user's full trail, exact history, or identity unless
  separately permitted.
- Public caches may appear as undisclosed content or map hints. The creator can add
  a riddle, code, or clue that must be solved after the location condition is met.
- Public visibility never removes reporting, moderation, content scanning, expiry,
  blocking, or safety controls.
- The app should distinguish **visited**, **nearby**, and **eligible to unlock** so
  users do not confuse a cache rule with proof of another person's presence.

---
*Essential feature. Part of [§5 Building Block View](./building-block-view.md).*

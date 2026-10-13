# 12. Glossary

Define domain and technical terms consistently so contributors and
documentation share a common vocabulary. Keep entries alphabetical. See
[Domain Data Model](../reference/domain-data-model.md) for the full entity list these
terms are drawn from.

| Term | Definition |
|------|------------|
| Cache | A location-bound note, photo, or file with an owner, area, audience, expiry, and access history (deferred feature) |
| CacheUnlockRule | Entity storing a cache's geographic range, visit/intersection requirement, audience, expiry, and riddle requirement |
| Challenge | A timed, structured social activity (photo/writing/puzzle/etc.) hosted inside a group chat, with its own prompt, deadline, submissions, and scoring (deferred feature) |
| Check-in | A manual, scheduled, or trip-based safety confirmation with reminders and an escalation path if missed |
| ConsentGrant | The record of a user's explicit, directional, revocable agreement to share a specific kind of data with a specific recipient |
| Directional consent | Location/data sharing that flows one way (owner → recipient) and does not imply the reverse, even between friends |
| Friend / FriendRelationship | A mutual relationship between two users; does **not** by itself grant location access |
| FriendMapLayer | A friend's coarse, consented contribution to another user's [Friend Mosaic Map](../architecture/friend-mosaic-maps.md) (deferred feature) |
| Lite mode | The safety-first MVP mode of the app: sharing, check-ins, stale-state display, and maps — no social/creative features |
| LocationShare | An active, directional, expirable grant of location visibility from one user to a specific recipient |
| LocationUpdate | A single reported position/timestamp/accuracy event from a user's device |
| MissingPersonEvent | A record of who activated a search/missing-person workflow and when (deferred feature) |
| MovementTrail | A consented, opt-in sequence of location points over time — treated as more sensitive than a single live share (deferred feature) |
| Precision (exact / approximate) | The granularity of a shared location — the sharer chooses whether a recipient sees an exact point or a coarser approximation |
| Full mode | The extended mode of the app that adds messaging, groups, prompts, challenges, and other social/creative features on top of Lite mode |
| SearchShare | Pre-authorized recipients, activation reason, expiry, and access log for a missing-person search corridor (deferred feature) |
| Stale / dead state | UI indication that a location update is no longer recent (stale) or has stopped entirely (dead) — must never be shown as live |
| TrajectoryEstimate | A calculated likely travel corridor with confidence/uncertainty, used only for pre-authorized search sharing, never as a guaranteed pin (deferred feature) |
| UserHandle | The unique, changeable username (subject to a cooldown), stored separately from the immutable internal user ID |
| VisitedRegion | A coarse, purpose-limited summary of an area a user has visited, used for private maps and cache-unlock eligibility (deferred feature) |

---
*Part of the [architecture documentation](./README.md) (arc42 §12).*

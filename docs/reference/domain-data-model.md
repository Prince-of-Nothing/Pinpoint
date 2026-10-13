# Domain Data Model

## Core entities

`User`, `UserHandle`, `Profile`, `UserStatus`, `Device`, `Session`,
`FriendRelationship`, `ConsentGrant`, `LocationShare`, `LocationUpdate`,
`MovementTrail`, `VisitedRegion`, `FriendMapLayer`, `FriendMapPreference`,
`TrajectoryEstimate`, `SearchShare`, `MissingPersonEvent`, `Cache`,
`CachePermission`, `CacheUnlockRule`, `Riddle`, `UnlockEvent`, `Group`,
`GroupMembership`, `Message`, `Call`, `Prompt`, `Challenge`,
`ChallengeParticipant`, `ChallengeSubmission`, `Scoreboard`, `Submission`,
`Rating`, `Report`, `ModerationAction`, `Notification`, `AuditEvent`.

Relationships must encode purpose, scope, expiry, revocation, and visibility. A
social membership must not imply a `LocationShare`. Precise `LocationUpdate`
records require stricter retention than public prompt submissions.

## Identity and profile

`User` is identified internally by an immutable ID. `UserHandle` stores the
unique username, previous-handle policy, and next-allowed-change time.
`Profile` stores optional nickname, name, image, and presentation preferences.
`UserStatus` stores content, audience, expiry, creation time, and moderation
state. API responses must be filtered by the viewer's relationship before
profile data is returned. See
[Actors and Accounts](./actors-and-accounts.md) and
[Profile and Visibility Model](./profile-and-visibility-model.md).

## Movement, trails, and search (deferred features)

`MovementTrail` stores consented location points with timestamps and
accuracy. `TrajectoryEstimate` stores the method, time window, likely
corridor, confidence, and uncertainty. `SearchShare` stores the
pre-authorized recipients, activation reason, expiry, revocation state, and
access log. `MissingPersonEvent` records who activated the workflow and when,
without treating the estimate as confirmed fact. See
[Movement Trails and Search Corridors](../architecture/movement-trails-and-search-corridors.md).

## Visited regions and caches

`VisitedRegion` stores a coarse area summary, confidence, visit window, and
retention policy. `CacheUnlockRule` stores the geographic range, whether a
verified visit or historical intersection is required, audience, expiry, and
riddle requirement. `UnlockEvent` records eligibility and content access
without exposing the visitor's trail to the creator. See
[Cache and Permission Model](../architecture/cache-and-permission-model.md).

## Friend maps

`FriendMapLayer` stores a coarse, consented contribution. `FriendMapPreference`
stores whether the owner shares a layer and whether a viewer hides it
locally. The composition is calculated from the viewer's authorized layers
and must not use raw location points as a client-visible API response. See
[Friend Mosaic Maps](../architecture/friend-mosaic-maps.md).

## Challenges

`Challenge` stores the prompt, source group chat, rules, deadline, scoring
method, visibility, and moderation state. `ChallengeParticipant` stores
invitation, acceptance, withdrawal, and display preferences.
`ChallengeSubmission` and `Scoreboard` store entries and results without
changing the underlying group or friend permissions. See
[Friend Challenges and Group Prompts](../explanation/friend-challenges-and-group-prompts.md).

---
*See also [§12 Glossary](../architecture/glossary.md) for short definitions
of each term, and [API Contract](./api-contract.md) for the wire representation.*

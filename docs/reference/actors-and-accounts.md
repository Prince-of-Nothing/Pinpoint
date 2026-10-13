# Actors and Accounts

## Roles

- **User:** owns a profile, devices, permissions, caches, media, and
  memberships.
- **Trusted friend:** receives location only after explicit consent.
- **Group member:** participates in a chat or community group without
  receiving location by default.
- **Creator:** owns group rules, membership settings, prompts, or challenges.
- **Moderator:** reviews reports, abusive content, and group violations.
- **Platform operator:** manages reliability and security without routine
  access to precise private location.

Accounts support pseudonymous display names, account recovery, device/session
management, blocking, reporting, and deletion.

## Identity and visibility

- Every account has an immutable internal user ID. Usernames and display
  names are never database relationship keys.
- A **username** is a unique, changeable handle used for login or discovery.
  A cooldown prevents frequent changes; two weeks is a reasonable starting
  point, with one month possible for abuse-heavy communities.
- A **name** or **nickname** is optional display text and does not need to be
  a legal or real name. It may be changed more freely, subject to moderation
  and history rules.
- A user status is a short update with an explicit audience, expiry, and
  privacy setting. It is not public by default.
- Non-friends receive an empty profile by default: no profile image, status,
  location, friend list, media, or activity updates. A minimal account result
  may exist only when needed for a search, request, report, or moderation
  action.
- Friendship, group membership, and location-sharing consent are separate
  permissions. Becoming a friend never automatically grants live location.

See [Profile and Visibility Model](./profile-and-visibility-model.md) for the
relationship-based view rules, and
[Domain Data Model](./domain-data-model.md) for the underlying entities (`User`,
`UserHandle`, `Profile`, `UserStatus`, `Device`, `Session`,
`FriendRelationship`).

---
*Referenced from [§8 Crosscutting Concepts](../architecture/crosscutting-concepts.md).*

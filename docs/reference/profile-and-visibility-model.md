# Profile and Visibility Model

## Default rule

People who are not friends should not be able to inspect one another. A
non-friend profile is empty by default: no image, name, nickname, statuses,
location, friend list, media, or activity history.

The app may show a minimal search result when necessary to send a friend
request, prevent impersonation, process a report, or support moderation. That
result must not become a browsable profile.

## Identity fields

- **Immutable user ID:** internal database identity used by every
  relationship, message, permission, and audit event.
- **Username:** unique handle for discovery or sign-in. It can change after a
  cooldown, with two weeks as a practical starting value and one month as a
  stricter alternative.
- **Name/nickname:** optional pseudonymous presentation text. It does not
  need to be a legal name and is not used as a database key.
- **Profile image:** optional and hidden from non-friends by default.
- **User status:** short text or media update with an audience and expiry.
  Suggested audiences are selected friends, trusted groups, a chat, or
  private-only.

## Relationship-based views

1. **Self:** full view of the user's own profile and settings.
2. **Friend:** fields explicitly allowed by the user's privacy settings;
   location still requires separate consent.
3. **Group member:** group-scoped identity only, with no automatic access to
   private profile or location data.
4. **Non-friend:** empty profile or minimal action-specific result.
5. **Blocked user:** no profile discovery, messaging, status, or location
   access except the minimum information needed for safety and moderation.

## Backend responsibility

The mobile app controls presentation, but the backend must enforce
visibility. It must authorize every profile, status, media, location, and
relationship request based on the authenticated viewer and the target's
current permissions. Hiding fields only in the app is not sufficient — see
[Security Architecture §"Control sequence for a location read"](../architecture/security-architecture-and-data-protection.md#control-sequence-for-a-location-read).

---
*Referenced from [Actors and Accounts](./actors-and-accounts.md) and
[§8 Crosscutting Concepts](../architecture/crosscutting-concepts.md).*

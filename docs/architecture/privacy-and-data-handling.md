# Privacy & Data Handling

Every comparable real-time location product researched (OwnTracks, Traccar, OsmAnd,
Mastodon) treats privacy and data handling as a **standalone, explicit, user-facing
document** rather than a line item in internal design notes, because location data
is sensitive and users/regulators need to find this information directly. This
document is that page for Always Together.

## Core principles

- Collect precise location only during an active, consented purpose.
- Separate trusted-location data from public social, group, prompt, and
  experimental data.
- Encrypt data in transit and at rest; protect keys separately.
- Make retention, deletion, export, and account closure understandable.
- Keep precise history off by default unless the user enables it.
- Avoid exposing IP addresses, hidden metadata, or private identity through
  public posts.
- Record access and permission changes without creating unnecessary
  surveillance logs.
- Provide privacy settings per feature, not one global switch.

## Identity and profile defaults

- Default non-friend profile visibility to empty: no image, status, updates,
  location, friend list, or activity.
- Users choose whether a status is visible to selected friends, a trusted
  group, a specific chat, or nobody; statuses support expiry.
- Usernames are stored separately from the immutable internal user ID. A
  username cooldown prevents impersonation and rapid identity cycling; display
  names and nicknames remain presentation-only fields. See
  [Actors and Accounts](../reference/actors-and-accounts.md) and
  [Profile and Visibility Model](../reference/profile-and-visibility-model.md).

## Who can access location data

- Location is visible only to friends the user has explicitly approved — never
  public by default (contrast with, e.g., Mastodon posts, which are public
  unless configured otherwise).
- Sharing is always directional and consent-based: the recipient must accept,
  and either side can stop, revoke, block, or report at any time. See
  [Location Sharing Lifecycle](./location-sharing-lifecycle.md).
- Backend operators/admins do not have standing access to raw location data.
  Any operator access is exceptional, logged, time-limited, and reviewed (see
  [Security Architecture §"Operator boundary"](./security-architecture-and-data-protection.md#operator-boundary)).

## Movement trails, search corridors, and friend maps (deferred features)

- Movement trails and trajectory estimates are treated as **more sensitive**
  than ordinary live sharing. They require separate opt-in consent, selected
  recipients, short retention, access logs, and immediate revocation.
- A trajectory estimate is never exposed publicly or to random groups. A
  missing-person share shows timestamps, source accuracy, and uncertainty
  rather than a falsely precise pin.
- Visited regions are stored as coarse, purpose-limited summaries where
  possible; the personal trail is not turned into a public social map by
  default.
- Cache-area intersection is evaluated without revealing the visitor's full
  movement history to the cache creator. Public cache discovery must not
  expose who visited an area.
- Friend-map layers are separate consent grants. A friend may disable their
  layer from their profile, and viewers may hide it locally without changing
  the friendship.
- The backend serves coarse region tiles or a filtered composite rather than
  raw trails. The mosaic never exposes a person's friend graph, home area,
  sensitive visits, or exact timestamps.

See [Movement Trails and Search Corridors](./movement-trails-and-search-corridors.md)
and [Friend Mosaic Maps](./friend-mosaic-maps.md) for the full feature design.

## Retention & deletion

- Location updates are superseded and not retained as history unless a user
  explicitly opts into movement-trail features, which then apply their own
  short retention and consent rules.
- Revoking a friend's access or deleting an account stops sharing/collection
  immediately, not on the next sync cycle.
- Users can request data export and deletion (GDPR-style rights); see
  [Requirements Traceability Matrix](../reference/requirements-traceability-matrix.md)
  for how this maps to acceptance criteria.

## Consent & controls

- Location sharing must be opt-in and revocable at any time.
- Users can see who currently has access to their location.
- Users can pause/mute sharing temporarily without removing a friend.

## Regulatory considerations

- If serving EU users, GDPR obligations apply (lawful basis for processing
  location data, data subject access/erasure rights, data minimization). See
  [Mastodon's admin/privacy guidance](https://docs.joinmastodon.org/admin/practices/)
  for a reference on how another social platform documents this
  responsibility.
- Precise geolocation is treated as sensitive personal data in many
  jurisdictions — confirm applicable requirements before launch and record the
  resulting approach as an ADR.

## Security measures

- Transport encryption (TLS) for all location updates in transit; encryption
  at rest for stored location and sensitive records.
- Access control enforcement at the API layer, not just the UI — see the
  [control sequence for a location read](./security-architecture-and-data-protection.md#control-sequence-for-a-location-read).
- Related: [§8 Crosscutting Concepts](./crosscutting-concepts.md) for
  authentication/authorization design, and
  [Security Control Register](../reference/security-control-register.md) for
  control-level test evidence.

---
*Referenced from [§8 Crosscutting Concepts](./crosscutting-concepts.md) and
linked from [docs/reference/](../reference) as the canonical privacy reference.
Source: vault "14 Privacy and data".*

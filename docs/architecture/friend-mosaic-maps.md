# Friend Mosaic Maps

## Concept

The app can turn opted-in visited regions into a stylized personal map. A user's
map may combine their own regions with optional map layers from friends, creating a
mosaic that is different for every person.

The result is inspired by collaborative canvas apps: the map becomes a living
record of shared places, but it is not a public replay of anyone's movements.

## Per-friend controls

- Each user can disable their contribution from their profile or privacy settings.
- A friend can choose no layer, a coarse region layer, selected regions, or a
  decorative contribution without location meaning.
- A viewer can hide a friend's layer locally without removing the friendship.
- Removing a friend, revoking consent, blocking, or deleting data removes or masks
  that person's layer according to the retention policy.
- Exact trails, timestamps, home areas, and live location are never included in the
  mosaic by default.

## Personal uniqueness

The map is generated from the viewer's own layer and the friend layers they are
allowed to see. Two people see the same composition only when their friend
relationships, consent settings, selected layers, and relevant region data are the
same. Having the same friends does not necessarily mean having the same map.

## Visual and social behavior

Regions can have colors, patterns, badges, memories, cache markers, or
creator-designed styles. The app should label contributions by pseudonymous
nickname only when the contributor allows it. Empty regions are normal; the map
should never pressure users to reveal more location data to fill them.

## Privacy boundaries

The backend should deliver coarse, privacy-filtered region tiles or a
privacy-preserving composite, not raw friend trails. A friend mosaic must not
reveal that a person visited a sensitive location, and public maps must not expose
a person's friend graph. The feature is optional and separate from emergency
search sharing.

---
*Deferred feature — not in the September MVP. See [§1.5](./introduction-and-goals.md#15-mvp-requirements-overview).
Part of [§5 Building Block View](./building-block-view.md).*

# Movement Trails and Search Corridors

## Purpose

Users may choose trusted people who can access a recent movement trail or activate
a search view if the user is missing or fails a planned check-in. The feature gives
people a place to start looking; it does not predict a person's exact location.

## Consent and sharing

- Movement history is opt-in and separate from ordinary live location sharing.
- The user selects recipients, data types, retention period, and activation
  conditions.
- A share may include recent points, direction of travel, approximate speed,
  likely route, and a confidence corridor.
- Recipients see timestamps, accuracy, data gaps, and uncertainty on every
  estimate.
- The user can revoke access, pause collection, or delete the trail unless a
  clearly disclosed safety incident hold applies.

## Estimation behavior

The system can use recent points, timestamps, heading, speed, route networks,
transport context, and known data gaps to calculate possible travel corridors. It
should present multiple plausible paths when evidence is weak and avoid a single
authoritative pin. No AI is required; a transparent rules-based model is preferable
for the first version.

## Missing-person workflow

**Plan creation:** The user creates a safety plan and pre-authorizes specific people.

**Check-in definition:** The user may define check-ins, an expected destination, and an escalation delay.

**Activation:** After a missed check-in or a trusted person's confirmed concern, an authorized
person activates the search view.

**Display:** The app shows the last verified point, movement trail, estimated corridor, data
freshness, and uncertainty.

**Guidance:** The app provides emergency-service guidance and an exportable incident summary
without claiming that the estimate is fact.

**Expiry:** The share expires automatically and records who activated and accessed it.

## Boundaries

This feature must not be available to random groups, ordinary followers, or people
who only know a username. It is not a substitute for emergency services,
professional search teams, or a missing-person report. Stalking prevention,
authentication, access logs, rate limits, and immediate revocation are mandatory.

## Visited regions map

The same opt-in trail can produce a private map of regions the user has visited.
The map starts blank and gradually fills with coarse tiles or areas rather than
exposing a replayable exact route. It can be shown to the user, or shared as a
deliberately selected region summary with trusted people.

The visited map should support uncertainty, timestamps, deletion, and privacy
thresholds. A brief or inaccurate location sample should not automatically mark a
whole area as visited.

## Location-locked content

A cache creator can define a geographic range. The service checks whether the
visitor's eligible visited regions intersect that range, or whether the visitor has
physically entered it after the cache was created. The result can unlock a message,
photo, audio clip, file, or other content without revealing the visitor's complete
movement trail to the creator.

Public caches can be discoverable as hidden content on the map. They may require
the visitor to enter the area and solve a riddle before opening. Public caches need
moderation, reporting, expiry, creator controls, content scanning, and protection
against exact-location harassment. A riddle is an additional game layer, not a
replacement for the location permission check. See
[Cache and Permission Model](./cache-and-permission-model.md).

---
*Deferred feature — not in the September MVP. See [§1.5](./introduction-and-goals.md#15-mvp-requirements-overview).
Part of [§5 Building Block View](./building-block-view.md).*

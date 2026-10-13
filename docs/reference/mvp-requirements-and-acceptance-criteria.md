# MVP Requirements and Acceptance Criteria

## REQ-01 Account and profile privacy

A user can create a pseudonymous account with a username and optional
nickname. Non-friends receive an empty profile response.

**Acceptance:** profile reads differ correctly for self, friend, group
member, non-friend, and blocked user; no location or activity is exposed
through the profile.

## REQ-02 Mutual friendship

Users can send, accept, decline, revoke, and block friendship requests.
Friendship does not automatically grant location access.

**Acceptance:** relationship state changes are authorized, auditable, and do
not expose hidden profiles.

## REQ-03 Directional location sharing

A user can share current location with a selected friend without receiving
that friend's location. The owner chooses exact or approximate precision,
update mode, and expiry.

**Acceptance:** only the authorized recipient can read current location;
revoked, expired, non-friend, and IDOR requests fail.

## REQ-04 Battery-aware updates

The app supports presets and an advanced update interval. It adapts
frequency or precision under low battery, weak connectivity, or OS
restrictions.

**Acceptance:** low-battery and offline states are visible; the app does not
claim current data when no update has arrived.

## REQ-05 Check-ins

The user can create manual, scheduled, and trip check-ins. Missed check-ins
send reminders before any configured escalation.

**Acceptance:** reminders, grace periods, cancellation, completion, and
failure states are observable and testable.

## REQ-06 Stale-state display

Trusted recipients see last update time, last-known battery, accuracy, and a
clear stale/dead state after extended signal loss.

**Acceptance:** stale data is never displayed with a live indicator; a dead
device does not claim to report current battery.

## REQ-07 Map and navigation

The user can view authorized friends on an OpenStreetMap-based map and
navigate using the built-in route option or an external maps app.

**Acceptance:** attribution appears, unauthorized friends are absent, and
external navigation links do not include unnecessary private data.

## REQ-08 Security baseline

The MVP validates inputs, uses parameterized database access, applies
authorization server-side, protects sessions/tokens, rate-limits sensitive
endpoints, handles errors safely, encrypts transport/storage, and records
security events.

**Acceptance:** the security control register records the problem,
relevance, implementation, test, and limitation for every selected control.

## REQ-09 Documentation evidence

The project maintains architecture, stack, data model, threat model,
security controls, incident response, testing, and deferred-scope
documentation.

**Acceptance:** mentor review can identify what is implemented, simulated,
deferred, and still risky.

## Explicit non-goals

The MVP does not include public/random discovery, sponsored prompts,
messaging, calls, trails, search corridors, traversal maps, caches, friend
mosaics, or advanced media moderation. Those remain documented extension
phases.

---
*See [Requirements Traceability Matrix](./requirements-traceability-matrix.md)
for how each REQ maps to UI, API, database, security control, tests, and
evidence.*

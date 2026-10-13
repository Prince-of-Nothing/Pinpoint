# 0004. Web client for the MVP demonstration; Flutter app deferred

Date: 2026-09-29

## Status

Accepted. Defers the "Mobile client: Flutter" row of the frozen MVP stack to
October-December; the stack choice itself is unchanged.

## Context

The MVP must demonstrate the safety flow (friends, directional sharing,
map with freshness, check-ins) and its security controls. A Flutter client
with background location, platform channels and store-ready permission
handling needs real-device testing on Android and iOS. The API had to be
built first, and the remaining time did not allow both.

## Decision

We will ship a small same-origin web client (plain HTML/JavaScript, no build
step) served by the API, using Leaflet with OpenStreetMap tiles. It exercises
every MVP endpoint, reports location from the browser or by clicking the map
(for demos), applies battery-aware update presets through the Battery Status
API where available, and follows strict web security practice: strict CSP
(`script-src 'self'`, Leaflet vendored), text-only rendering, access token in
memory, refresh token in an HttpOnly cookie with CSRF protection.

Alternatives considered: *Flutter now* (rejected for time and device-testing
risk); *API documentation only (Swagger)* (rejected: cannot demonstrate the
map, freshness display or CSP/XSS controls).

## Consequences

- Positive: a complete, demonstrable end-to-end flow; web security controls
  (CSP, CSRF, cookie flags, XSS) are real rather than hypothetical.
- Negative: browsers cannot report location in the background, so the web
  client is not a real safety tool; REQ-04 battery behaviour is only
  partially demonstrated. The API already supports native clients
  (`token_delivery: "body"`), so the Flutter app needs no API redesign.

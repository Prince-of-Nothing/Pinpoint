# 0015. Throttled, opt-out launch-time update check

Date: 2026-11-15

## Status

Accepted

## Context

Pinpoint 1.3.2 removed the automatic GitHub request that ran when About was
opened, because it disclosed app usage without consent. The update check then
became user-initiated from About → Check for updates.

Issue #34 showed the cost of that decision: a user running 1.3.10 on Android had
no way to learn that 1.3.12 existed unless they opened About and pressed the
button. The app has no store listing or auto-updater, so silent staleness is a
security and support problem: old APKs miss fixes.

The check is a single GET of the public `releases.atom` feed. It sends no
account, location, friend or device identifier; the User-Agent is
`Pinpoint/<version>`. GitHub necessarily sees the request IP, as it would for any
HTTPS request.

## Decision

Check GitHub releases automatically at most once per 24 hours, on the first
frame after launch/unlock:

- The result is shown as a non-blocking banner on every screen except About,
  with **View update** and **Later** (dismisses that release tag).
- Users can disable the automatic check in **Profile & settings → Updates**;
  **About → Check for updates** always remains available on demand.
- The timestamp of the last check and the dismissed tag live in
  `<app_data_dir>/prefs.json` (on-device only).
- No release is downloaded or installed automatically. The action is a link to
  the public release page.
- The check is skipped while the app lock is showing, so cached locations are
  not displayed before unlock.

Alternatives considered:

- **Keep it fully manual:** rejected; issue #34 showed users never discover
  updates.
- **Check on every launch:** rejected; unnecessary traffic and a larger
  disclosure than needed for a stable release feed.
- **Use a Pinpoint-hosted update endpoint:** deferred; it would hide the GitHub
  IP disclosure but requires the backend deployment still tracked in #27.

## Consequences

- Stale clients learn about new releases within a day of their next launch.
- The app makes one background request per day to GitHub, which is disclosed in
  the About screen, README and privacy document and can be turned off.
- A failed launch check does not retry until the next day; the manual button
  remains the immediate path.
- The wasm stubs are unchanged and never make the request (ADR-0007).

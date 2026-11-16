# 0018. One update check after installing a new app version

Date: 2026-11-16

## Status

Accepted

## Context

ADR-0015 throttles the automatic GitHub releases check to at most once per
24 hours. The timestamp lives in `prefs.json`, which survives an app update. A
user who launched 1.4.0, then installed 1.4.1 within a day, would not learn
about 1.4.1 until the 24-hour window expired. The manual **About → Check for
updates** button remained, but the automatic check looked broken after an
upgrade.

## Decision

A newly installed app version performs one launch check immediately; the
ADR-0015 24-hour throttle then applies to that version:

- Store `last_update_check_version` beside `last_update_check_unix` in
  `prefs.json`.
- A launch check is due when the stored version is empty, differs from
  `CARGO_PKG_VERSION`, or the 24-hour window has elapsed.
- The request is unchanged: one GET of the public releases feed, User-Agent
  `Pinpoint/<version>`, no account, location, friend or device data.
- The manual button remains always available, and the Profile & settings
  opt-out remains.

## Consequences

- An upgraded client can learn about the release it just installed, and may
  make two checks in one day: one before the upgrade and one after it.
- The extra request happens only after an install or update, not on every
  launch.
- ADR-0015 remains the throttle decision; this ADR records the version-change
  exception.

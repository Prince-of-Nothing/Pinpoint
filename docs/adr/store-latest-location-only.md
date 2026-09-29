# 0005. Store only the latest location and derive freshness and check-in states on read

Date: 2026-09-29

## Status

Accepted.

## Context

Location history is the most sensitive data the system could hold, and the
MVP features (current location, stale/dead display, check-ins) do not need
it. The stack also planned Celery workers for stale-state transitions and
check-in reminders, which adds an operational component before it is needed.

## Decision

- We will store one `latest_locations` row per user (overwritten on update,
  deletable by the owner). No location history is kept in the MVP.
- Freshness (`live`/`stale`/`dead`/`unknown`) and check-in states
  (`pending`/`due_soon`/`overdue`/`missed`) are computed from timestamps on
  every read instead of by background jobs.
- Approximate shares are produced by snapping to a fixed grid (about 1.1 km)
  at read time; the exact point never leaves the server for such shares.

Alternatives considered: history table with retention jobs (rejected: no MVP
feature needs it and it increases breach impact); Celery-driven state
transitions (deferred until push notifications need a worker).

## Consequences

- Positive: a database compromise reveals at most one point per user;
  no worker to operate; states are always consistent with the clock.
- Negative: no push reminders for missed check-ins yet (visible in the
  client only); movement trails and search corridors will need a new,
  separately governed store with its own ADR.

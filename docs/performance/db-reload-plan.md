# Plan: per-request database reload and the global store lock

Status: **plan only, not implemented.** Nothing here has been measured yet; the
first phase exists to get the numbers that decide the later phases.

## What happens today

Every request to `/api/` goes through the convergence middleware
(`src/server/mod.rs:232-248`):

1. `state.refresh_from_db()` runs before the handler. It builds a new `Store`
   from Postgres (`src/server/state/mod.rs:452-476`): all users, all
   friendships, the whole locations domain (shares, last locations, check-ins,
   emergencies), diagnostics, and messages when the service owns them. The
   result replaces the shared store under one `std::sync::Mutex`.
2. The handler runs against that in-memory copy.
3. For every non-GET request, `state.persist()` runs after the handler
   (`src/server/state/mod.rs:478-530`). It opens a transaction, takes
   `pg_advisory_xact_lock` for the service, clones the whole store with
   `snapshot()`, and writes the users, friendships and locations domains.
   The `All` service also rewrites the entire `pinpoint_state` JSON document on
   every mutation.

Other costs in the same path:

- `converge` prints one `eprintln!` line per request, including the URI
  (`src/server/mod.rs:238`). That is noisy in production logs.
- The shared store is touched in 89 places (`store.lock()`), so one slow
  handler can hold up every other request, and the store is rebuilt on each
  request, so a handler's view can change mid-flight.

## Why it matters

- Cost per request grows with the total data, not with what the request
  touches. A read of one user's friends loads every user and every message.
- Two replicas agree only because each request reloads everything. That works,
  but it is the reason the reload cannot be removed without a replacement for
  read-after-write.
- The whole-store snapshot on each write makes write latency grow with the
  database, and it holds the advisory lock for that time.

## Goals

- A request reads and writes only the rows it touches.
- Read-after-write holds across replicas without a full reload.
- Writes are per-row upserts inside one transaction; no whole-store snapshot.
- The lock protects one domain at a time, never the whole store across an
  `.await`.
- Per-request logging is off by default.

## Phases

**Phase 0: measure (no behaviour change).**
Seed a staging database with realistic volume (for example 10,000 users,
50,000 messages, 5,000 locations). Time `load_normalized`, `persist`, and a
handful of representative requests (login, friends list, send message, publish
location). Record p50 and p95 per request and the rows read per request. Gate
the `CONVERGE` log line behind an environment flag. Decide the targets from
these numbers, not from this plan.

**Phase 1: scope the reload.**
Each handler declares the domains it reads (users, friendships, locations,
messages). The middleware loads only those. Cross-domain reads that need
another domain (for example a friend's location needs friendships) load that
domain too, and nothing else. This is a small change that cuts most of the
work for the common reads.

**Phase 2: read from Postgres for the hot paths.**
For the endpoints that are read-only and hot (friend list, friend locations,
message list), query Postgres directly with the same column grants the service
role already has, and stop depending on the in-memory copy for them. This keeps
read-after-write without a reload, because the read sees the committed row.

**Phase 3: per-row persistence.**
Replace `persist()` with the writes the handler actually made, in one
transaction, keyed by the changed rows. Retire the `pinpoint_state` snapshot
once no reader needs it. Keep the advisory lock, but scope its key to the table
or the row range being written, not to the whole service.

**Phase 4: narrow the lock.**
Replace the single store mutex with per-domain locks, and never hold one across
an `.await`. The 89 call sites are the work list; most become one-line changes
once Phase 1 has split the store by domain.

## Risks and checks

- Two replicas must not diverge. Each phase needs a test that runs two
  `AppState` instances against one database and checks read-after-write.
- The in-memory mode (no `DATABASE_URL`) must keep working; it is the test
  default.
- Column grants: Phase 2 reads must use only the columns each service role is
  granted (`db/02-grants.sql`).

## Out of scope for this plan

- Changing the data model or the migration set.
- Moving to a different database.

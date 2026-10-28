# Project Description — Always Together

## In one sentence
A location-sharing system where **friendship ≠ permission**: every share of your location is
explicit, granular, time-boxed, revocable, and enforced server-side.

## Problem
Existing location-sharing products conflate social relationships with data access. Being
"friends" usually means blanket visibility, which enables accidental over-sharing, stalking,
unauthorized surveillance, and long-lived forgotten grants. For safety-critical use cases
(family check-ins, solo travel, vulnerable users), the stakes are physical: leaked location
data can facilitate real-world harm.

## Solution principles
1. **Directional consent** — each (sharer → recipient) pair is its own authorization record
   with precision (`exact` / `approximate` / `city_only`), purpose, and optional expiry.
2. **Server-side enforcement** — the client is untrusted; every read checks consent state,
   friendship status, and blocks at request time.
3. **Honest freshness** — the UI never implies certainty: last-known location is always shown
   with age/accuracy and explicit **stale/dead** states. No false safety confidence.
4. **Security baseline everywhere** — auth with session invalidation, CSRF double-submit,
   rate limiting, bcrypt hashing, secure-by-default cookies, minimal error leakage.
5. **Privacy by default** — pseudonymous accounts, no location stored beyond the current
   publish, immediate and complete revocation.

## Scope (MVP)
- Pseudonymous accounts & profiles; admin-provisioned access
- Mutual friendship workflow (request → accept/reject)
- Directional sharing permissions (create/pause/resume/revoke, precision levels)
- Current-location publishing (manual or device geolocation)
- Map view (OpenStreetMap tiles) with friend pins + freshness badges
- Navigation export (deep links to external map apps)
- Stale-state detection & display
- Rate limits, audit-friendly logs, secure errors
- Web client (SPA) + Go API + Dockerized deployment

## Explicitly deferred (extension phases)
Movement trails & search corridors, visited-region maps, friend mosaics, caching layers,
public/sponsored prompts, messaging, calls, random groups, advanced moderation. Each is
documented in `ROADMAP.md` with its threat implications, but none inflate MVP deliverables.

## Success criteria
- All MVP user stories implemented with passing tests (`go test ./...`)
- Zero cross-tenant data leakage in authorization tests (IDOR attempts fail closed)
- One-command local deployment (`docker compose up --build`) with seeded demo data
- Thesis documentation traceability: requirement → implementation → test evidence

## Non-goals
- General-purpose social network features (feeds, likes, comments)
- Advertising or third-party data monetization — ever
- Covert monitoring capabilities (the product protects the sharer, not the watcher)

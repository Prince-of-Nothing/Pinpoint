# 0016. Read-only service status and integration discovery

Date: 2026-11-15

## Status

Accepted

## Context

ADR-0008 splits the API into independently runnable binaries behind an nginx
gateway. Each binary has `/health`, and `/api/v1/sync-status` reports database
mode and persistence freshness, but there is no single place that answers:

- which service instance answered;
- whether it uses Postgres or the in-memory store;
- which component path prefixes exist; and
- which OSM/tile domains the deployment currently uses.

Issue #33 asked for more modular deployment and for a page that exposes each
component's status and the current integration domains. Clients resolve the
gateway origin themselves, so the status page can use relative paths instead of
hardcoding a public hostname.

## Decision

Mount two read-only routes in every service binary and in the combined server:

- `GET /status` — a small HTML page with the version, service kind, database
  mode, component prefixes and integration endpoints.
- `GET /api/v1/services` — the same data as JSON for scripts and future clients.

Rules:

- No authentication is required; the page must be reachable during an incident.
- No account, location, friend, session or audit data is read.
- Endpoints are stripped of userinfo, query strings and fragments, so tokens
  and credentials never appear. Relative paths are used for gateway components.
- The page is same-origin, has no JavaScript and reuses the strict CSP/security
  headers of the static web UI.
- The map binary reports `map` and uses `ServiceKind::MapOnly`; the other
  binaries report their own kind.

Alternatives considered:

- **Client-only About card:** rejected as the only surface; operators need a
  browser-accessible page even when the desktop app is not running.
- **Admin-only page:** rejected; status must be readable while auth/admin
  services are degraded.
- **Probe every service from one aggregator:** deferred. It needs explicit
  per-service URLs, timeouts and SSRF protection. This slice provides the
  stable discovery surface that such an aggregator can build on.

## Consequences

Easier:

- One URL shows what the deployment is and which integrations it uses.
- Split services expose the same contract without sharing a database read.
- Scripts can consume `/api/v1/services` instead of parsing HTML.

Harder / risks:

- A public page reveals sanitized integration hosts. Deployments that use
  internal hostnames should restrict `/status` at the gateway or firewall.
- The page reports configuration, not live upstream health; `/health` remains
  the liveness/readiness check.
- Future per-service URLs still require a domain/config decision and outbound
  probes (tracked as follow-up work, not part of this ADR).

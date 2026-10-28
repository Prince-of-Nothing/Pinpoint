# 0003. Structure the backend around object-oriented design patterns

Date: 2026-10-28

## Status

Accepted

## Context

The backend had grown as a single-layer script style: routers queried the ORM
directly, authentication logic was duplicated across endpoints, the security
helpers were half-wired (rate-limit keys generated but never used, cookies set
on a discarded `Response` object, lockouts that never expired), and three of
the four routers imported by `app/main.py` did not exist, so the application
could not start.

At the same time, the security documentation
([Security Control Register](../../reference/security-control-register.md),
[API Security Implementation](../../reference/api-security-implementation.md))
promised controls — complete mediation of every location read, anti-
enumeration login, token revocation, CSRF, rate limiting — that a flat
structure cannot express or test in isolation. The team also needs the
codebase to serve as university-assessment evidence for object-oriented
design competence ("TMPS": SOLID plus the classic GoF pattern set).

Requirements:

1. Security behaviour must live in named, individually testable units, not
   scattered `if` statements.
2. Each documented control must map to exactly one implementation site.
3. The structure must not outgrow the MVP: no distributed-system ceremony
   for a four-router API.

## Decision

We will structure the backend in four explicit layers and apply a small,
documented set of object-oriented design patterns — recorded in
[Design Patterns and Principles](../../reference/design-patterns.md) — with
each pattern chosen for a security purpose:

| Layer | Patterns | Purpose |
|---|---|---|
| API (routers + `deps.py`) | Dependency Injection, Decorator-style dependencies | Complete mediation: one `get_auth_context()` path for all credentials; rate limits and CSRF compose as dependencies |
| Services | Facade (`AuthService`, `TokenService`), Strategy + Registry (policies, precision reducers), Chain of Responsibility (password policy), Template Method (password change), Null Object (anti-enumeration dummy hash), Observer (audit logger) | Testable, swappable security logic |
| Repositories | Repository | Single place for queries; transaction boundary owned by services |
| Core/Models/Middleware | Singleton (settings, engine), Factory (app factory, rate-limiter selection), Decorator (security-headers middleware) | Consistent configuration and cross-cutting hardening |

Supporting decisions:

- **PyJWT instead of python-jose** (actively maintained; no known unfixed
  algorithm-handling CVEs), with a fixed algorithm allowlist and required
  claims.
- **Password policy as a rule chain** rather than a monolithic function, so
  rules (length, classes, deny-list) can be added or reordered without
  touching callers.
- **Pseudonymised audit storage**: login attempts store keyed hashes of
  emails, not raw addresses.
- **bcrypt pinned to 4.0.x** until passlib supports ≥ 4.1.

## Consequences

- Every control in the register now has a single implementation site and a
  regression test in `backend/tests/test_security_flows.py` (23 tests).
- Adding a new visibility rule means writing one policy class and registering
  it — routers stay untouched (Open/Closed).
- The pattern catalogue becomes part of the assessment evidence; new
  contributors must follow it (enforced by review, not tooling).
- Slight indirection cost: a request now traverses router → dependency →
  service → repository. Accepted for a safety-critical API; the alternative
  (inline checks) proved untestable in practice.
- The transaction boundary moved to the service layer; repositories must
  never call `commit()` — a rule documented in
  [Design Patterns and Principles](../../reference/design-patterns.md).

---
*See [Design Patterns and Principles](../../reference/design-patterns.md)
for the full catalogue, and the [Security Control Register](../../reference/security-control-register.md)
for the control-to-code mapping.*

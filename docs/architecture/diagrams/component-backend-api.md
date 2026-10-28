# Component View — Backend API (C4 Level 3)

Internals of the **API Service** container as implemented in
[`backend/app/`](../../backend/app). Pattern names in italics refer to the
[Design Patterns and Principles](../../reference/design-patterns.md)
catalogue. Corresponds to [§5.2 Component Breakdown](../building-block-view.md).

```mermaid
C4Component
    title Always Together — API Service internals

    Container_Ext(client, "Mobile Client / Web Companion", "Bearer JWT or session cookie + CSRF")

    System_Boundary(api, "API Service (FastAPI)")
        Component(mw, "Middleware stack", "TrustedHost → Security Headers (Decorator) → CORS", "Host allowlist, HSTS/CSP/nosniff, per-origin CORS")
        Component(auth_router, "Auth Router", "/api/v1/auth/*", "Login (rate-limited), logout, password change, force change")
        Component(user_router, "User Router", "/api/v1/users/*", "Own profile; relationship-filtered profiles (SC-03)")
        Component(fr_router, "Friendship Router", "/api/v1/friendships/*", "Request/accept/reject/revoke/block (rate-limited)")
        Component(loc_router, "Location Router", "/api/v1/locations/*", "Consent records, latest-location upsert, authorized reads")
        Component(deps, "Dependencies (DI)", "get_auth_context, rate_limit, csrf_protect, require_fresh_password", "The single credential path + composable gates")
        Component(auth_svc, "AuthService (Facade)", "login/logout/_apply_password_change (Template Method)", "Anti-enumeration (Null Object), lockout with auto-unlock")
        Component(token_svc, "TokenService (Facade)", "JWT issue/validate", "Algorithm allowlist; type/ver/sid/jti required")
        Component(pol, "Authorization policies (Strategy)", "LocationAccessPolicy, ProfileVisibilityPolicy, PrecisionReducers", "Complete mediation; unknown precision fails closed")
        Component(pw_policy, "PasswordPolicy (Chain)", "length/classes/deny-list rules", "Fail-closed validation")
        Component(rl, "RateLimiter (Strategy/Adapter)", "InMemory / Redis", "Per-IP sliding/fixed windows → 429 + Retry-After")
        Component(audit, "AuditLogger (Observer)", "StructuredLog + Database handlers", "PII-free security events (SC-08)")
        Component(repos, "Repositories (Repository)", "User, Session, Friendship, LocationShare, LocationUpdate, LoginAttempt", "Parameterised queries only; no commits")
        ComponentDb(db, "PostgreSQL", "", "Hashed session tokens, consent records, latest location only, audit events")
    End System_Boundary

    Rel(client, mw, "HTTPS")
    Rel(mw, auth_router, "")
    Rel(mw, user_router, "")
    Rel(mw, fr_router, "")
    Rel(mw, loc_router, "")
    Rel(auth_router, deps, "Depends()")
    Rel(user_router, deps, "Depends()")
    Rel(fr_router, deps, "Depends()")
    Rel(loc_router, deps, "Depends()")
    Rel(deps, token_svc, "validate JWT")
    Rel(deps, auth_svc, "compose facade")
    Rel(auth_router, auth_svc, "login / password ops")
    Rel(auth_svc, pw_policy, "validate")
    Rel(auth_svc, rl, "consume budget")
    Rel(loc_router, pol, "decide access")
    Rel(user_router, pol, "filter profile")
    Rel(fr_router, rl, "consume budget")
    Rel(auth_svc, audit, "record event")
    Rel(pol, audit, "record event")
    Rel(deps, repos, "resolve session/user")
    Rel(auth_svc, repos, "read/write")
    Rel(loc_router, repos, "read/write")
    Rel(fr_router, repos, "read/write")
    Rel(repos, db, "SQL (parameterised)")
```

## Request path invariants

1. **One credential path** — `get_auth_context()` (component `deps`) accepts
   Bearer JWT or session cookie, and applies signature, token-type, token-
   version, session revocation, absolute expiry, and idle timeout to both.
2. **One transaction boundary** — repositories never commit; services and
   routers commit once per operation.
3. **One visibility decider** — no router compares relationships inline; it
   asks a policy.
4. **Everything gated** — `rate_limit`, `csrf_protect`, and
   `require_fresh_password` are declared per endpoint in one line each.

---
*Runtime behaviour: [Sequence — Login](./sequence-login.md),
[Sequence — Authorized Location Read](./sequence-authorized-location-read.md),
[Sequence — Friendship & Consent](./sequence-friendship-consent.md).
Structure of the domain classes: [Class Diagram — Domain Model](./class-domain-model.md).*

# 05 — Sequence: Authentication (login → session + CSRF) 📋

Matches current Go handlers (`internal/handlers/auth.go`, `internal/security`).

```mermaid
sequenceDiagram
    autonumber
    participant C as Client (web/app)
    participant M as Middleware chain
    participant A as AuthHandler
    participant S as Store
    participant R as Redis (T06, planned)

    C->>M: POST /api/v1/auth/login {email, password}
    M->>M: RateLimit check (IP+email bucket)
    alt limit exceeded
        M-->>C: 429 Too Many Requests
    end
    M->>A: forward
    A->>S: GetUserByEmail(email)
    S-->>A: user{PasswordHash, Locked, FailedAttempts}
    alt locked or bad password
        A->>S: IncrementFailedAttempts / LockUser at threshold
        A-->>C: 401 invalid credentials (generic message)
    else ok
        A->>S: ResetFailedAttempts, TouchLastLogin
        A->>A: Issue session token (JWT) + store session (T11)
        A-->>C: 200 Set-Cookie: session=... HttpOnly SameSite=Lax<br/>body: {csrf_token, must_change_password}
    end

    Note over C: client keeps csrf_token in memory,<br/>sends it as X-CSRF-Token on every mutation

    C->>M: GET /api/v1/me (Cookie: session)
    M->>M: verify cookie/session valid & not revoked
    M-->>C: 200 profile | 401 if revoked (logout invalidates server-side T11)

    C->>M: POST /api/v1/locations (Cookie + X-CSRF-Token)
    M->>M: CSRF double-submit compare
    alt mismatch
        M-->>C: 403 CSRF failure
    end
    M-->>C: handler runs → 201
```

Forced password change: when `must_change_password=true`, middleware blocks all non-`/auth/change-password` routes with 403 `PASSWORD_CHANGE_REQUIRED`.

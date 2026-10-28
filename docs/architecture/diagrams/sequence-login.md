# Sequence — Login (with rate limiting, lockout, anti-enumeration)

Runtime behaviour of `POST /api/v1/auth/login` as implemented in
[`api/auth.py`](../../backend/app/api/auth.py) →
[`AuthService.login()`](../../backend/app/services/auth_service.py).
Patterns: Facade, Null Object (timing equalisation), Observer (audit).

```mermaid
sequenceDiagram
    autonumber
    actor U as User
    participant MW as Middleware<br/>(TrustedHost, Headers)
    participant API as Auth Router
    participant RL as RateLimiter<br/>(Strategy)
    participant AS as AuthService<br/>(Facade)
    participant DB as PostgreSQL
    participant AUD as AuditLogger<br/>(Observer)

    U->>MW: POST /api/v1/auth/login {email, password}
    MW->>API: forwarded (host allowlisted)
    API->>RL: hit("login:<ip>", 10, 60s)
    alt budget exceeded
        RL-->>API: denied
        API-->>U: 429 RATE_LIMIT_EXCEEDED + Retry-After
    end

    API->>AS: login(email, password, ip, user_agent)
    AS->>DB: lookup user by normalised email
    alt unknown or inactive account
        DB-->>AS: none
        AS->>AS: verify against throwaway hash<br/>(Null Object — equalises timing)
        AS->>AUD: login_failed(reason=unknown_or_inactive)
        AS-->>API: AuthenticationError (generic)
        API-->>U: 401 AUTHENTICATION_FAILED
    else account exists
        DB-->>AS: user
        alt account locked
            AS->>AS: lockout expired?
            alt lockout elapsed
                AS->>AS: auto-unlock, reset counters
            else still locked
                AS->>AS: verify against throwaway hash
                AS-->>API: AuthenticationError (same generic 401)
            end
        else password check
            AS->>AS: bcrypt verify
            alt bad password
                AS->>AS: failed_login_attempts += 1<br/>lock at threshold (5)
                AS->>DB: persist counters + LoginAttempt(email_hash)
                AS->>AUD: login_failed / account_locked
                AS-->>API: AuthenticationError (same generic 401)
            else good password
                AS->>DB: create Session(token_hash=SHA-256(raw))
                AS->>AS: JWT {sub, sid, ver, type=access, jti, exp=15m}
                AS->>AUD: login_succeeded
                AS->>DB: commit
                AS-->>API: LoginResult(raw session token, access token)
                API-->>U: 200 + Set-Cookie session_id (HttpOnly)<br/>+ Set-Cookie csrf_token (readable)
            end
        end
    end
```

## Design notes

- **Every failure is the same 401** (`AUTHENTICATION_FAILED`): account
  existence, lockout state, and password correctness are never disclosed.
- **Timing equalisation**: the bcrypt work happens for unknown accounts too,
  so response time cannot enumerate users (Null Object pattern).
- **Lockout auto-expires** (`LOCKOUT_DURATION_MINUTES`), unlike the previous
  implementation which locked accounts forever.
- **Session tokens are stored hashed**; the raw value exists only in the
  client's HttpOnly cookie.
- The access token is 15 minutes; the session is 3 hours absolute + 30
  minutes idle, checked on every authenticated request.

---
*See [Security Control Register SC-01](../../reference/security-control-register.md#sc-01-authentication).*

# Sequence — Friendship & Directional Consent Lifecycle

The full consent chain as implemented in
[`api/friendships.py`](../../backend/app/api/friendships.py) and
[`api/locations.py`](../../backend/app/api/locations.py): request → accept →
grant consent → publish → read → revoke → block.

```mermaid
sequenceDiagram
    autonumber
    actor A as Alice
    actor B as Bob
    participant APIA as API (Alice's session)
    participant APIB as API (Bob's session)
    participant POL as Policies<br/>(Strategy)
    participant DB as PostgreSQL
    participant AUD as AuditLogger

    Note over A,APIA: 1. Friendship — both parties must opt in
    A->>APIA: POST /friendships/ {receiver_email: bob}
    APIA->>APIA: rate limit 10/h, CSRF (cookie auth), freshness gate
    APIA->>DB: no existing pair row? no block?
    APIA->>DB: Friendship(A→B, pending)
    APIA->>AUD: friendship.requested
    APIA-->>A: 201 {friendship_id}

    B->>APIB: POST /friendships/{id}/accept
    APIB->>APIB: receiver-only ownership check
    APIB->>DB: status = accepted
    APIB->>AUD: friendship.accepted
    APIB-->>B: 200 {status: accepted}

    Note over A,DB: 2. Consent — only the sharer grants, never on behalf of others
    A->>APIA: POST /locations/shares<br/>{friendship_id, to_friend, precision, purpose}
    APIA->>DB: friendship must be accepted; caller must be a party
    APIA->>APIA: direction != to_friend → 400<br/>("mutual" needs each side's own record)
    APIA->>APIA: expiry bounded: now + ≤24h (fail-safe default)
    APIA->>DB: revoke any previous active share; create LocationShare
    APIA->>AUD: location_share.created(owner, recipient, precision, purpose)
    APIA-->>A: 201 {share_id, expires_at}

    Note over A,B: 3. Publish & read
    A->>APIA: POST /locations/current {lat, lng, accuracy, battery}
    APIA->>DB: upsert latest LocationUpdate (no history)
    B->>APIB: GET /locations/friends/latest
    APIB->>POL: check(Bob, Alice, share)
    POL-->>APIB: allowed, precision
    APIB->>DB: Alice's latest point
    APIB->>POL: reduce(lat, lng)
    APIB-->>B: 200 [{friend: Alice, reduced coords, freshness}]

    Note over A,B: 4. Revocation — immediate
    A->>APIA: DELETE /locations/shares/{id} (owner only)
    APIA->>DB: is_active=false, revoked_at=now
    APIA->>AUD: location_share.revoked(owner_request)
    B->>APIB: GET /locations/friends/latest
    APIB-->>B: Alice absent (policy denies revoked share)

    Note over B,DB: 5. Blocking wins over everything
    B->>APIB: POST /friendships/{id}/block
    APIB->>DB: friendship.status = blocked
    APIB->>DB: revoke every share between the pair
    APIB->>AUD: user.blocked(shares_revoked=n)
    APIB-->>B: 200 {status: blocked}
```

## Invariants illustrated

- **Consent is directional and personal**: a share is created by its owner
  only; `mutual` requires two records so nobody consents on another's behalf.
- **Bounded by default**: forgetting `expires_at` yields a 24-hour share, not
  an eternal one.
- **Revoke and block are O(1) safety actions**: both flip server-side state
  that the read policy consults on every access — no client caching of
  permission.
- **Rate limits** (10/h friend requests, 20/h share creations) blunt
  harassment and scraping.

---
*See [Location Sharing Lifecycle](../location-sharing-lifecycle.md) and
[Security Control Register SC-02](../../reference/security-control-register.md#sc-02-directional-authorization).*

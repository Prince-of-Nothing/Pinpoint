# MVP Demo Script

A 15-minute demonstration of the Always Together MVP for Task 9: architecture,
main functionality and implemented security measures. Setup is in
[How to Deploy for University Demonstration](./deploy-for-demonstration.md).

Before starting: seed the data, flush Redis, and open two browser windows:

- window A: normal, logged in as `demo_parent`
- window B: private, logged in as `demo_child`

Also open a terminal in the repository.

## 0. Opening (1 min)

> "Always Together lets trusted friends share their location for safety,
> without friendship implying location access. Every share is one-way,
> chosen precision, time-limited and revocable. Recipients always see how
> fresh the data is."

Show the [Implementation Status](./implementation-plan.md) table: tasks 1-9.

## 1. Architecture (2 min)

Show [Building Block View (as built)](../architecture/building-block-view.md#51a-as-built-september-mvp):

- Modular monolith: FastAPI API, PostgreSQL, Redis; web client served from
  the same origin.
- Mention the three scope decisions and why: ADRs 0003 (password auth now,
  OIDC later), 0004 (web client now, Flutter later) and 0005 (latest
  location only).
- `docker compose ps`: the API, database and Redis. Only the API port is
  exposed. The API runs as a database role that cannot change the schema.

## 2. Main functionality (5 min)

**Window A (Maria, `demo_parent`), Map tab:**

1. Three friends on the map.
   - Alex: green, live, exact, low battery.
   - Sam: orange and dashed, stale 20 minutes, approximate (circle of about 1 km).
   - Jordan: grey, no signal for 3 hours, last-known battery 5%.
   > "Stale data is never shown as live. That matters in an emergency."
2. Open Jordan's popup: navigation links carry only the coordinate.
3. Check-ins tab: Jordan's check-in is **missed**; Alex's is on track.

**Window B (Alex, `demo_child`):**

4. Friends tab: pending request from Sam. Accept it, and point out that
   Sam still sees **nothing** of Alex's location.
5. Sharing tab: Alex shares with Maria (exact, until revoked). Maria shares
   with Alex only approximately for 1 hour: **each direction is separate**.
6. Map tab → Reporting → Manual → click a new position. In window A, the
   marker moves on the next refresh (or reload).
7. Check-ins tab: tap **I'm OK**. Window A shows it as completed.
8. Sharing tab: **Stop sharing** with Maria. In window A, Alex disappears
   from the map and the list.

**Account tab (either window):** active sessions with **Revoke** and
**Log out everywhere**.

## 3. Security measures (5 min)

Run in the terminal:

```bash
sh backend/scripts/demo_security.sh
```

Narrate each step as it prints:

| Step | Control | What to say |
|---|---|---|
| 1 | SC-02 | Recipients only get what was explicitly shared, re-checked on every request |
| 2 | SC-03 | A blocked user cannot tell the blocker exists |
| 3 | SC-02 | IDOR: another user's record gives the same 404 as a missing one, and the attempt is audited |
| 4 | SC-04 | Unknown fields are rejected, so a user cannot make themselves admin |
| 5 | RBAC | Admin endpoints require the admin role, which is read from the database |
| 6 | SC-04 | Coordinates are range-checked; NaN/Infinity rejected |
| 7 | SC-05 | Refresh-token rotation: replaying a stolen token kills the whole session |
| 8 | SC-05 | Logout ends the session server-side, so the JWT stops working at once |
| 9 | SC-01 | Brute force hits the rate limit (429) and account lockout |

Then show the evidence:

- As admin (`ops`): `GET /api/v1/admin/audit-events?event_type=authz.denied`
  (or the `/docs` page) shows the IDOR attempt from step 3.
- Browser dev tools → Application → Cookies: `at_refresh` is `HttpOnly`,
  `SameSite=Strict`, path `/api/v1/auth`. Network tab: response headers
  include `Content-Security-Policy`.
- Optional: set a display name to `<img src=x onerror=alert(1)>`; it shows
  as plain text for friends (XSS prevented by rendering and CSP).
- Open [Security Test Report](../evidence/security-test-report.md): 140
  tests grouped by control, 97% coverage, bandit and pip-audit clean. Run
  `cd backend && pytest -q` live if time allows (about 7 seconds).

## 4. Limitations and next steps (2 min)

Be explicit (from the [register](../reference/security-control-register.md)):

- The web client cannot share location in the background: the Flutter app
  is the October priority.
- No MFA or passkeys yet; no account recovery; HS256 shared-secret tokens.
- Encryption at rest and backups depend on the hosting environment, which
  is not set up yet.
- Abuse reporting and moderation are not built yet.

Close with the [October-December plan](./development-plan-october-december.md):
native app and identity; then safety features and infrastructure; then
abuse handling, privacy rights and an ASVS review.

## Presentation slide outline

1. Title: Always Together, a privacy-first safety map
2. Problem: location sharing without consent enables stalking; stale data misleads
3. Scope: MVP requirements REQ-01..09 and what was deferred (and why)
4. Architecture: modular monolith diagram; data minimisation decisions
5. Directional consent model: friendship ≠ access; precision, expiry, revocation
6. Demo (live)
7. Security controls: the Task 6 list mapped to SC-01..SC-06
8. Testing: test pyramid, IDOR matrix, report numbers, CI pipeline
9. Limitations (honest list)
10. October-December plan
11. Questions

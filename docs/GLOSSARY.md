# Glossary

**Directional consent / SharePermission** — an explicit grant from one specific user (sharer)
to one specific recipient to view their location, carrying precision, purpose, expiry, and
state. Friendship is a precondition, never a substitute.

**Precision** — how coarse shared coordinates are: `exact` (as published), `approximate`
(server-jittered by a few km), `city_only` (centroid/label only). Downgrade happens
server-side at read time.

**Purpose** — free-text reason attached to a grant ("safety", "meetup"…), surfaced to both
parties to prevent scope drift.

**Current publish** — the single latest location a user has chosen to record. MVP stores no
history; each publish overwrites the previous.

**Freshness** — age + reported accuracy of a publish, shown honestly in clients.

**Stale / Dead state** — classification when a publish is too old to trust (stale) or far too
old / device presumed offline (dead). Prevents false safety confidence.

**Battery-aware updates** — client policy adapting publish frequency/precision to device
battery state (planned; scheduler in Kotlin app).

**Check-in** — deliberate "I'm here / I'm okay" publish. Variants planned: manual, scheduled,
trip-based, plus missed-check-in alerts.

**Movement trail** — historical path recording. *Deferred extension*; highest privacy risk in
the backlog (habits, home/work inference).

**Search corridor** — region around a planned route used to match nearby friends. Deferred.

**Visited-region map** — aggregated places a user has been. Deferred; requires k-anonymity.

**Friend mosaic** — combined multi-friend view. Deferred; allowed only with unanimous
per-member consent.

**Session** — server-side authentication state bound to an HttpOnly cookie; destroyed on logout.

**CSRF token (double-submit)** — non-HttpOnly token the client must echo in `X-CSRF-Token`
on state-changing requests when using cookie auth.

**Rate limiting** — per-IP/per-user throttling on sensitive endpoints (auth especially) to
blunt brute force and abuse.

**IDOR** — Insecure Direct Object Reference: attacking another user's resources by swapping
IDs. The system must fail closed with generic errors.

**Fail closed** — default-deny posture: any doubt (missing grant, malformed input, expired
token) results in rejection, not partial access.

**Admin-provisioned account** — user created by an admin with `must_change_password` set;
locked out of normal functionality until they rotate the temporary password.

**weba / webk** — planned companion clients: `weba` = admin console SPA; `webk` = lightweight
kiosk/quick check-in web view.

**MVP** — the nine-requirement thesis scope (see docs/PROJECT.md); everything else is an
extension phase gated by threat analysis.

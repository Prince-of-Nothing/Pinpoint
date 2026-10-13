# Requirements Traceability Matrix (RTM)

This matrix maps every MVP requirement to its UI implementation, API
endpoint, database model, security controls, test coverage, and evidence.
The chain runs: **REQ → UI → API → DB → security control → test →
evidence**.

| REQ | UI / Screen | API Endpoint(s) | Database Table(s) | Security Control(s) | Tests | Evidence |
|---|---|---|---|---|---|---|
| **REQ-01** Profile privacy | Profile screen; views for self/friend/non-friend/blocked | `GET /api/v1/users/{id}`; filtered responses | `users`, `profiles` | SC-03 Private-by-default profiles; SC-04 Input/API security | Profile views from all 5 actor contexts (self, friend, group member, non-friend, blocked); verify no location/activity exposed to non-friends | Demo screenshot of each viewer type; test report |
| **REQ-02** Mutual friendship | Friends screen; /friendships/* operations | `POST/GET/DELETE /api/v1/friendships/{id}`; status endpoint | `friendships` | SC-02 Directional authorization; SC-04 Input/API security; SC-05 Session/transport security | Accept/decline/revoke/block state changes auditable; friendship does not auto-grant location access; IDOR tests on friendship endpoints | Demo video of friendship flow; test report with audit log verification |
| **REQ-03** Directional location sharing | Share Location screen; location card on map | `POST /api/v1/location-shares/`; `GET /api/v1/location-shares/{id}`; `DELETE /api/v1/location-shares/{id}` | `location_shares` | SC-02 Directional authorization (recipient-owned); SC-04 Input/API security; SC-06 Data protection | Recipient-only access; non-friend denial; revoked-share denial; expired-share denial; IDOR attempts with another user's ID; cross-account tests | Demo screenshot of share creation/revocation; test report verifying directional consent |
| **REQ-04** Battery-aware updates | Location settings screen; battery presets | `GET /api/v1/users/me`; location update flow | `users`, `user_status`, `latest_locations` | SC-07 Battery and stale-state safety; SC-04 Input/API security | Battery profiles tested; offline periods; OS-paused updates; low-battery states; stale UI behavior (no live indicator when stale); clock differences | Battery test log; stale-state UI verification video |
| **REQ-05** Check-ins | Check-in screen; create/scheduled/trip check-ins | `POST /api/v1/check-ins/`; `GET /api/v1/check-ins/{id}/status`; `POST /api/v1/check-ins/{id}/complete`; `POST /api/v1/check-ins/{id}/cancel`; `POST /api/v1/check-ins/{id}/acknowledge` | `check_ins`, `check_in_events` | SC-07 Battery/stale-state safety; SC-04 Input/API security; SC-09 Incident response | Reminders sent; grace period observed; cancellation; completion state; failure/missed state; escalation flow; reminders before escalation | Demo of full check-in lifecycle; test report with reminder/grace/complete/fail verification |
| **REQ-06** Stale-state display | Friend map; location card | `GET /api/v1/latest_locations`; WebSocket location updates | `latest_locations` | SC-07 Battery and stale-state safety; SC-04 Input/API security; SC-02 Directional authorization | Stale data never displayed with live indicator; dead device does not claim current battery; last-update timestamp shown; accuracy and battery info visible; clear stale/dead state after extended signal loss | UI verification: stale state vs live state comparison; test report on stale/dead transitions |
| **REQ-07** Map and navigation | Map screen; friends display; navigation links | `GET /api/v1/map/friends`; `GET /api/v1/map/navigation/deep-link` | `users`, `location_shares`; no location history stored in map API | SC-02 Directional authorization (unauthorized friends absent); SC-04 Input/API security; SC-06 Data protection (no private data in links) | Attribution appears on map; unauthorized friends absent from `/map/friends`; external navigation links contain only coordinate + label (no friendship/sharing/history data); IDOR test on map endpoint | Map screenshot with attribution; deep link URL inspected (no private params); test report on navigation privacy |
| **REQ-08** Security baseline | N/A (cross-cutting) | All endpoints | All relevant tables | SC-01 Authentication; SC-04 Input/API security; SC-05 Session/transport security; SC-06 Data protection; SC-09 Incident response | Input validation/parameterized queries on all endpoints; session/token protection; rate limiting on sensitive endpoints; error responses safe (no stack traces); security events recorded for authz/IDOR/rate limit/input | Security control register entries for SC-01 through SC-09; penetration test summary; dependency/vulnerability scan |
| **REQ-09** Documentation evidence | N/A (documentation screen) | N/A | N/A | All relevant security controls | RTM itself is the evidence artifact; mentor review of architecture, stack, data model, security, testing, and deferred-scope docs | This RTM document; architecture proposal sign-off; mentor review checklist |

## How to read the matrix

- **REQ** – The requirement number from REQ-01 through REQ-09 (see
  [MVP Requirements and Acceptance Criteria](./mvp-requirements-and-acceptance-criteria.md)).
- **UI / Screen** – The mobile screen or web page where the requirement is
  visible/interacted with.
- **API Endpoint(s)** – The versioned API contract endpoint(s) that
  implement the requirement (see [API Contract](./api-contract.md)).
- **Database Table(s)** – The PostgreSQL tables that store the relevant data
  (see [Domain Data Model](./domain-data-model.md)).
- **Security Control(s)** – The SC from the
  [Security Control Register](./security-control-register.md) that applies.
- **Tests** – The key test scenarios that validate the requirement
  end-to-end.
- **Evidence** – What can be shown/presented to the mentor as proof of
  implementation.

## Status tracking (not shown in table)

All rows default to **Planned** → **In Progress** → **Implemented** →
**Verified** as work is completed. The rightmost column (Evidence) is the
primary artifact that moves a row from "planned" to "implemented" for
mentor review.

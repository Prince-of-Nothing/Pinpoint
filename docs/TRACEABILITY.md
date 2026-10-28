# Requirements Traceability Matrix (RTM)

Thesis evidence chain: **user story → implementation → test**. Keep this table updated in
every PR that touches MVP requirements (see CONTRIBUTING.md rule 4).

Req IDs follow the nine MVP requirements in `thesis_content.md`.

| #  | Requirement (short)                | User story                                                          | Implementation                                       | Test evidence                          | Status |
|----|------------------------------------|---------------------------------------------------------------------|------------------------------------------------------|----------------------------------------|--------|
| R1 | Account privacy / pseudonymity     | As a user, I have a pseudonymous account with minimal PII           | `internal/handlers/auth.go`, `users.go`; bcrypt hashes | `TestAuth*`, `TestUsers*`              | ✅     |
| R2 | Mutual friendship                  | As a user, we both must accept before any sharing is even possible  | `friendships.go` (request/accept/reject)             | `TestFriendship*`                      | ✅     |
| R3 | Directional sharing permissions    | As a sharer, I grant per-recipient precision/purpose/expiry         | `shares.go`, `models.SharePermission`                | `TestShare*` incl. pause/resume/expiry | ✅     |
| R4 | Current-location updates           | As a sharer, I publish my current location on demand                | `locations.go` POST                                  | `TestLocationPublish*`                 | ✅     |
| R5 | Battery-aware frequency            | As a phone user, updates adapt to battery state                     | field plumbed (`battery_pct`); scheduler deferred    | — (client logic, Phase 2 Kotlin)       | 📋     |
| R6 | Check-in workflows                 | As a contact, I'm alerted if a scheduled check-in is missed         | manual only in MVP; scheduled/trip deferred          | manual publish covered by R4           | 📋     |
| R7 | Stale-state display                | As a viewer, I always see age/accuracy and stale/dead flags         | freshness computed server-side; SPA badges           | `TestFreshness*`                        | ✅     |
| R8 | Map + navigation export            | As a viewer, I see friends on OSM and can open external nav         | `web/public/app.js` (tiles, deep links)              | manual E2E (Playwright planned)        | ✅     |
| R9 | Security baseline                  | As all parties, authz/CSRF/rate-limit/secure-errors enforced        | `internal/security/*`, middleware                    | security test suite (all green)        | ✅     |
| R10| Documentation                      | As examiners, requirements trace to code and tests                  | this repo's `docs/` + thesis doc                     | n/a                                    | ✅     |

## Deferred-feature register (with threat note)
| Feature            | Phase | Blocking risk to resolve first                                |
|--------------------|-------|---------------------------------------------------------------|
| Movement trails    | P3    | Habit inference ⇒ home/work exposure; needs opt-in + expiry + E2E design |
| Visited regions    | P3    | Aggregation leaks; k-anonymity threshold required             |
| Friend mosaics     | P3    | Group consent semantics; unanimous grants only                |
| Messaging/calls    | P4    | New abuse surface; moderation prerequisite                    |
| OAuth/OIDC + PKCE  | P4    | Replace short-JWT mitigation; device/session management       |

## How to extend
New MVP-scope requirement → add row (R-id, story, file(s), test func name(s)).
Test not yet written → status 📋 and a linked issue; never mark ✅ without executable proof.

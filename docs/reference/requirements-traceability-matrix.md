# Requirements Traceability Matrix (RTM)

This matrix maps each MVP requirement to its UI surface, API endpoint, data
model, security control, automated tests and evidence. The chain runs:
**REQ → UI → API → DB → security control → test → evidence**. It describes the
**as-built Rust system**; the detailed field rules are in the
[API contract](./api-contract.md) and the models in
[system models](../analysis/system-models.md).

| REQ | UI surface | API endpoint(s) | Data | Security control(s) | Tests | Evidence |
|---|---|---|---|---|---|---|
| **REQ-01** Account and profile privacy | Web/desktop sign-in, register, Account; native login/register | `POST /auth/register`, `/auth/login`, `/auth/anonymous`, `/auth/anonymous/recover`, `GET/PATCH /auth/me`, `/users/:id`, `/users/me` | `pinpoint_users`, `pinpoint_sessions` | SC-01 Authentication; SC-03 Private-by-default; SC-05 Session/transport | `tests/router_probe.rs`, `tests/auth_password.rs`, `tests/oauth.rs`, `scripts/smoke.ps1` | Security docs; live smoke (email bidi 400, Unicode 201) |
| **REQ-02** Mutual friendship | Friends screen (web drawer, native Friends) | `GET/POST /friendships/`, `POST /friendships/:id/respond`, `DELETE /friendships/:id` | `pinpoint_friendships` | SC-02 Directional authorization; SC-04 Input/API | `tests/router_probe.rs`, `scripts/smoke.ps1` (reject → no revive → fresh request → accept) | Smoke output |
| **REQ-03** Directional location sharing | Sharing screen; map share sheet | `POST/PATCH/DELETE /locations/shares`, `GET /locations/shares` | `pinpoint_shares` | SC-02 Directional authorization; SC-04 Input/API | `tests/location_consent.rs`, `scripts/smoke.ps1` (label bidi/newline) | Test report; smoke |
| **REQ-04** Publish location and freshness | Map publish form; native publish | `POST/GET /locations/me`, `GET /locations/friends` | `pinpoint_locations` | SC-02 consent on read; SC-04 validation; SC-07 latest-only + timestamp | `tests/location_consent.rs` (accuracy bounds, consent, precision), `scripts/smoke.ps1` | Smoke (`accuracy_neg=400`, `accuracy_high=400`, `accuracy_ok=200`) |
| **REQ-05** Private trail, notes and check-ins | Today screen (web/native) | `GET /locations/history`, `GET/POST/DELETE /locations/notes`, `/locations/check-ins` | `pinpoint_location_history`, `pinpoint_place_notes`, `pinpoint_checkins` | SC-03 owner-only; SC-06 encryption at rest | `tests/today.rs`, `tests/note_encryption.rs`, `scripts/smoke.ps1` | Tests; smoke (note/check-in bounds) |
| **REQ-06** Map, search and routing | Map screen; offline catalog | `GET /map/tiles/:z/:x/:y`, `/map/search`, `/map/reverse`, `/map/route`, `/map/capabilities`, `/map/attribution`; `/offline/packs/*` | No private data; OSM proxy | SC-04 validation; SC-07 availability; OSM policy (ADR-0011/0021) | `tests/map_route_e2e.rs`, `tests/map_offline_e2e.rs`, `map-qa.yml`, `scripts/smoke.ps1` (search 120/121) | Map QA workflow; smoke |
| **REQ-07** End-to-end encrypted messages | Chats (web/native); safety number sheet | `PUT /messages/key`, `GET /messages/key/:id`, `POST/GET /messages`, `/messages/conversations`, `/messages/unread`, `/messages/read` | `pinpoint_messages`, `pinpoint_message_keys` (ciphertext only) | SC-06 data protection; ADR-0023 | `tests/contract.rs`, `tests/router_probe.rs`, message vectors (`tests/message-vector.json`, `web/app/test/message-js-vector.json`) | Vector tests |
| **REQ-08** Security baseline | Cross-cutting | All authenticated routes | All tables | SC-01…SC-09 | `cargo clippy -D warnings`, `cargo test`, `cargo audit`, CodeQL, ZAP baseline, smoke | CI; [DAST report](../evidence/zap-baseline.html); control register |
| **REQ-09** Documentation and evidence | N/A | N/A | N/A | All controls | Documentation review | This RTM; [security architecture](../security-architecture.md); [system models](../analysis/system-models.md); [evidence](../evidence/); ADRs |

## Implementation status (as built)

| REQ | Status | Implemented by | Automated tests |
|---|---|---|---|
| REQ-01 Account and profile privacy | **Implemented** — email/password + TOTP, anonymous + recovery, Google OAuth; GitHub OAuth implemented, PR pending | `src/server/auth_handlers.rs`, `src/security/password.rs`, `src/security/oauth.rs` | `tests/router_probe.rs`, `tests/auth_password.rs`, `tests/oauth.rs` |
| REQ-02 Mutual friendship | **Implemented** — request/accept/reject/remove; a rejected request needs a fresh one | `src/server/users_handlers.rs` | `tests/router_probe.rs`, `scripts/smoke.ps1` |
| REQ-03 Directional sharing | **Implemented** — `exact`/`approximate`/`city_only`, pause, expiry (24 h default, 168 h cap), revoke | `src/server/locations_handlers.rs` | `tests/location_consent.rs` |
| REQ-04 Publish and freshness | **Implemented** — coordinates/accuracy validated, latest-only row with `updated_at`; no battery sampling | `src/server/locations_handlers.rs`, `src/models.rs` | `tests/location_consent.rs` |
| REQ-05 Private activity | **Implemented** — owner-only 24 h trail (7-day retention pruned hourly), notes/photos, check-ins (`manual`/`scheduled`/`trip` labels) | `src/server/locations_handlers.rs` | `tests/today.rs`, `tests/note_encryption.rs` |
| REQ-06 Map and routing | **Implemented** — MapLibre + PMTiles, opt-in OpenFreeMap, OSRM proxy, search/reverse, attribution | `src/server/mod.rs`, `web/app/map.js`, `src/osm.rs` | `tests/map_route_e2e.rs`, `tests/map_offline_e2e.rs`, `map-qa.yml` |
| REQ-07 E2E messages | **Implemented** — ECIES (X25519 + HKDF-SHA256 + AES-256-GCM + Ed25519), ciphertext-only server, safety number, read receipts | `src/security/e2e.rs`, `src/server/messages_handlers.rs` | Message vectors, `tests/contract.rs` |
| REQ-08 Security baseline | **Implemented** for the MVP scope; SC-06 (location rows), SC-07 (no battery/push) and SC-08 (no SIEM) are partial | all endpoints | CI + CodeQL + ZAP + smoke |
| REQ-09 Documentation and evidence | **Implemented** | this repository | This RTM, docs index, evidence folder |

## Deferred / not in the MVP

- Battery and charging telemetry, scheduled reminders and push notifications.
- Passkeys/magic links, explicit OAuth provider linking, GitHub OAuth merge.
- Database-level encryption for location rows and an automated key-rotation
  tool; the encrypted backup wrapper covers off-host copies.
- Multi-replica shared rate limiting and session pending-state storage.

## How to read the matrix

- **UI surface** names the screen in the web client and, where it exists, the
  native egui screen.
- **API endpoint(s)** are the as-built paths; field rules and status codes are
  in the [API contract](./api-contract.md).
- **Tests** are runnable: `cargo test --locked`, `scripts/smoke.ps1`, and the
  web checks under `web/app/test/`.
- **Evidence** points to a report, workflow or command output; the control
  register explains the problem, relevance, implementation and limitations of
  each SC item.

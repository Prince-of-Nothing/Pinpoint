# Development Roadmap and Evidence (Weeks 7–15)

This page is the working plan behind the course milestones and the evidence
that backs each claim. It describes the **as-built** Rust system, not the
historical prototypes.

## Where the project stands

| Area | State | Evidence |
|---|---|---|
| Accounts and sessions | Email/password (Argon2id), TOTP MFA, anonymous + recovery code, opaque session cookies, CSRF, sliding and absolute expiry, revocation | `docs/security-architecture.md` §4, ADR-0009, ADR-0012, `tests/router_probe.rs` |
| Authorization | Directional shares, precision downgrade, pause/expiry, owner-only notes/trail/bookmarks, RBAC (`resident`/`admin`/`administrator`) | `tests/location_consent.rs`, `tests/router_probe.rs`, `docs/security-architecture.md` §10 |
| Input and API security | Server-side validation, stable error envelope, sqlx bound queries, per-client and per-account rate limits | `src/security/validation.rs`, `src/security/rate_limit.rs`, `tests/` |
| Transport and client | HTTPS-only client policy, HSTS, CSP, secure cookie attributes, CSRF double-submit | ADR-0006, `src/server/mod.rs`, `src/api.rs` |
| Messaging | E2E ECIES (X25519 + HKDF-SHA256 + AES-256-GCM + Ed25519), ciphertext-only server, safety number | ADR-0023, `docs/messaging.md`, `tests/message-*.json` |
| Maps | MapLibre + PMTiles, opt-in OpenFreeMap basemap, OSM proxy caps, attribution | ADR-0020, ADR-0021, `docs/OSM_POLICY.md`, `map-qa.yml` |
| Threat model | STRIDE identification, impact × likelihood scoring, prioritised table, attack tree | `docs/security-architecture.md` §2 |
| Verification | 373 lib tests, integration suites, 29-check live smoke, ZAP baseline run, CI gate | `docs/TESTING.md`, `scripts/smoke.ps1`, `docs/evidence/zap-baseline.*` |

## Milestones

### Weeks 7–8 — consolidation

- Finish the requirements set and link every security requirement to the asset
  it protects and the threat it answers (traceability matrix update).
- Complete the UML set: use case, sequence for sign-in/share/message, activity
  for share creation, state machine for a share and a session, ER for the data
  model, component and deployment views with network zones.
- Validate assumptions with target users (questionnaire/interviews) and record
  any pivoting decision with its effect on requirements and threat model.

### Weeks 9–11 — defence priorities

The threat model ranks what to build first. The current order:

1. consent and IDOR checks (T-01, T-02) — implemented, keep regression tests;
2. session and CSRF hardening (T-03, T-04) — implemented, add rotation tests;
3. injection and dependency integrity (T-10, T-12) — implemented, add SAST;
4. at-rest protection and backups (T-13) — partially deployed;
5. log and diagnostics hygiene (T-14), device storage (T-08), admin
   escalation (T-15) — implemented, keep under review.

### Weeks 12–13 — remaining defences and security testing

- Encrypt note/photo blobs and database backups; document key custody.
- Automate SAST in CI (CodeQL or Semgrep) next to the existing Clippy gate.
- Automate the ZAP baseline as a scheduled DAST run against a seeded instance.
- Add Dependabot (Cargo, npm, GitHub Actions) to complement `cargo audit`.
- Server-side CAPTCHA or proof-of-work for registration if abuse appears;
  rate limits cover the current MVP.

### Weeks 14–15 — Midterm 2

- Live demo: registration, consent, share expiry, trail, notes, messages,
  admin console, and the security tests.
- Report v2.0, slides, individual corrections, remaining-risk list.

### Exam session

- Final application, public repository, README with run and test commands,
  final report, presentation.

## Open decisions

| Decision | Options | Recommendation |
|---|---|---|
| OAuth 2.0 login | (a) implement authorization-code + PKCE for Google/GitHub; (b) keep first-party sessions and document the choice | (a) removes the only named course requirement not met; PKCE scaffolding exists in `src/security/oauth.rs`. Needs provider client IDs and secrets outside the repository |
| Encryption at rest | (a) volume encryption only; (b) application-level encryption for notes/photos; (c) both | (b) protects against a DB or backup leak, which the threat model scores High (T-13) |
| Rate limiting | (a) per replica (current); (b) shared store | (b) before any multi-replica deployment |

## Evidence index

- Gate: `cargo fmt`, `cargo clippy -D warnings`, `cargo build --locked`,
  `cargo test --locked`, `cargo audit`.
- Web checks: `node web/app/test/{sharing-ui,admin-ui,messages-poll,password-length,navigation-ui}.mjs`.
- Live smoke: `scripts/smoke.ps1` (29 checks).
- DAST: `docs/evidence/zap-baseline.html` and `.json`.
- Threat model and control register: `docs/security-architecture.md` §2,
  `docs/reference/security-control-register.md`.

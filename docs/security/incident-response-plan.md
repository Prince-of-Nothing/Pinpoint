# Pinpoint — Incident Response Plan (DSA)

**Owner:** team lead (see `docs/SUPPORT_CONTACTS.md`). **Linked:** `docs/security/threat-model.md` (mitigations), `docs/SECURITY.md` (reporting, 72h ack / 30d fix).

## 1. Severity & triage (first 24h)

| Sev | Example (Pinpoint) | Response clock | Notify |
|---|---|---|---|
| P1 | Live location disclosure / account takeover in progress | Stop-ship; revoke ALL sessions; rotate `DATABASE_URL`/secrets | Team + supervisor, users if personal data affected (72h GDPR) |
| P2 | Auth bypass on demo only, DoS on gateway | Patch + deploy within 7 days | Team + supervisor |
| P3 | Header hygiene (Z-1/Z-3), Dependabot moderate | Next sprint | Weekly review |

## 2. Detection sources

- Audit logs (`login failure` spikes, `CSRF_FAILED`, IDOR probe audit) — grep stdout/file sink.
- Gateway `access.log` (rate-limit 429 counts).
- Healthchecks (`/health`, `/map/health`, `pg_isready`) + Docker logs.
- ZAP baseline scan (Task 3) + `cargo-audit` Dependabot.

## 3. Playbooks

| Signal | Immediate actions | Follow-up |
|---|---|---|
| Session token leak | Password change (revokes all sessions); rotate `SECRET_KEY`; force re-login | Review auth code; add rotation test |
| DB breach (`pinpoint_state`) | Halt gateway; snapshot logs; assume latest locations exposed; notify affected users | Normalise tables; least-privilege DB user; E2E design note |
| Location over-sharing complaint | Pause/delete share on request; audit the grant; check expiry logic | UX copy fix; expiry default review |
| Device lost (bookmarks) | Bookmarks are on-device AES-GCM (if key protected); remote account deactivation | OS-keychain migration |
| OSMF ban / Nominatim 429 | Throttle confirmed; switch to self-hosted tile cache plan | Reduce poll interval; cache POI |

## 4. Recovery & lessons

- **Backups:** Postgres named volume `pinpoint-db-data`; restore via `docker compose` re-init from snapshot JSONB.
- **Drill:** reproduce P1 with dedicated test accounts quarterly; measure time-to-revoke.
- **Post-mortem:** within 5 working days — timeline, root cause (map to STRIDE ID), fixes, retest evidence (`cargo test` + ZAP).

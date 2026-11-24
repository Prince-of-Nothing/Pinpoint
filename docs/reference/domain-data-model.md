# Domain Data Model

The as-built model is relational (PostgreSQL 16 in production, the same structs
in memory for the demo). Tables are prefixed `pinpoint_`; identifiers are
`BIGINT` and timestamps are ISO-8601 text or `TIMESTAMPTZ`. The ER diagram and
the field-level rules live in
[system models](../analysis/system-models.md) and the
[API contract](./api-contract.md).

## Core entities

| Table | Purpose | Key rules |
|---|---|---|
| `pinpoint_users` | Accounts | Unique email; `role` (`resident`/`admin`/`administrator`); `auth_method` (`email`/`anonymous`/`oauth`); `identity_subject` unique when set; Argon2id hash; TOTP secret; lockout counters |
| `pinpoint_sessions` | Opaque sessions | Session id stored as a SHA-256 hash; CSRF token; `auth_method`; `last_activity`; `mfa_pending` |
| `pinpoint_friendships` | Friend graph | `pending`/`accepted`/`rejected`; a rejected request is final and a new request is required |
| `pinpoint_shares` | Directional consent | One row per friendship + direction; `precision_level` (`exact`/`approximate`/`city_only`); `is_active` (pause); `expires_at` (24 h default, 168 h cap); label |
| `pinpoint_locations` | Latest location | One row per user; coordinates + accuracy + `updated_at`; no history in this table |
| `pinpoint_location_history` | Owner-only trail | 24 h window returned, 7-day retention pruned hourly; never shared |
| `pinpoint_place_notes` | Private notes | Owner-only; label/message/photo encrypted at rest when `DATA_ENCRYPTION_KEY` is set |
| `pinpoint_checkins` | Check-ins | `manual`/`scheduled`/`trip`; note bounded; owner-only |
| `pinpoint_emergencies` | Emergency alerts | Public-to-recipients alert; resolved flag; does not override a paused share |
| `pinpoint_messages` | E2E envelopes | Ciphertext only (`ciphertext_to`/`ciphertext_from`), nonces, salts, Ed25519 signature, read timestamp; unique signature prevents replay |
| `pinpoint_message_keys` | Public bundles | X25519 + Ed25519 public keys per user, replaced on rotation |
| `pinpoint_diagnostic_reports` | Diagnostics | Minimal category/message, no location/IP/device id; capped ingestion |
| `pinpoint_admin_audit` | Admin audit | Actor, action, target, timestamp for user creation, reset, deactivation and rotation changes |
| `pinpoint_schema_migrations` | Migration ledger | Applied version + name + timestamp |

## Identity and visibility

- A public handle is derived from the user id (`username_for`); emails are
  never searchable by non-admins.
- Profile reads are scoped: self, accepted friend, pending request, otherwise
  `404`; no location or activity is exposed through the profile.
- A friendship alone grants no location access; an active, unexpired,
  unpaused share naming the caller as recipient is required on every read.
- Notes, the trail, bookmarks and check-ins are owner-only by query filter.
- Account deletion cascades through the tables above and tombstones the id so
  a stale replica cannot resurrect it.

## Retention and encryption

- The trail is pruned hourly to seven days; the API only returns 24 hours.
- Messages are ciphertext on the server; only the participants hold plaintext.
- Passwords are Argon2id PHC hashes; legacy PBKDF2 hashes are rehashed on login.
- Note label/message/photo use AES-256-GCM (`enc:v1:`), with legacy plaintext
  rows still readable.
- Location rows, friendships and profile fields are plaintext; the database
  role and volume/backup controls are the boundary (residual risk in the
  [control register](./security-control-register.md), SC-06).

## Not in the model

Groups, prompts, challenges, scoreboards, battery/charging telemetry,
WebSocket events and push notifications are **not** implemented. They were
part of the earlier target design and are listed as deferred work; do not
assume their tables exist.

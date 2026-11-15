# 9. Architecture Decisions

This section is an index into the [Architecture Decision Records](../adr) (ADRs). Do
**not** duplicate decision rationale here — link to the ADRs, which are the durable
source of truth for *why* a decision was made.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](../adr/record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](../adr/0002-adopt-pinpoint-product-direction.md) | Adopt Pinpoint as the product direction | Accepted |
| [0003](../adr/password-auth-with-rotating-refresh-tokens.md) | Password authentication with session-bound JWTs and rotating refresh tokens for the MVP | Accepted |
| [0004](../adr/web-client-for-mvp-demonstration.md) | Web client for the MVP demonstration; Flutter app deferred | Accepted |
| [0005](../adr/store-latest-location-only.md) | Store only the latest location; derive freshness and check-in states on read | Accepted |
| [0006](../adr/0006-rust-client-transport-and-session-contract.md) | Rust desktop client transport policy (HTTPS off-loopback) and session/CSRF contract | Accepted |
| [0007](../adr/0007-rust-multiplatform.md) | Rust multiplatform as the sole application codebase | Accepted |
| [0008](../adr/0008-modular-rust-services.md) | Split Rust API into independently scalable services behind a gateway | Accepted |
| [0009](../adr/0009-dual-auth-anonymous-and-identity.md) | Dual authentication models — anonymous device-bound and identity-linked accounts | Accepted (partial) |
| [0010](../adr/0010-normalized-encrypted-database.md) | Normalized, encrypted, modular database schema | Accepted (design target) |
| [0011](../adr/0011-self-hosted-osm-services.md) | Self-hosted OpenStreetMap services | Accepted (config + docs) |
| [0012](../adr/0012-opaque-server-side-sessions.md) | Opaque server-side sessions instead of JWT + rotating refresh tokens | Accepted |
| [0013](../adr/0013-android-native-activity-jni.md) | Use the live Android activity for JNI and accept cold-start share/deep-link delivery | Accepted |
| [0014](../adr/0014-remove-action-send-target.md) | Remove the ACTION_SEND target until an onNewIntent bridge exists | Accepted |
| [0015](../adr/0015-launch-time-update-check.md) | Throttled, opt-out launch-time update check | Accepted |
| [0016](../adr/0016-service-status-discovery.md) | Read-only service status and integration discovery | Accepted |
| [0017](../adr/0017-android-java-bridge.md) | Minimal Java Android bridge for share-in, biometrics and background location | Accepted |

Add a row here whenever a new ADR is created. See [`adr/template.md`](../adr/template.md)
for the format used to write new ADRs, and [`adr/README.md`](../adr/README.md) for the
process.

---
*Part of the [architecture documentation](./README.md) (arc42 §9).*

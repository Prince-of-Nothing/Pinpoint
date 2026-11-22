# 0023. End-to-end encrypted 1:1 messages (ECIES, not Signal Protocol)

- Status: Accepted
- Date: 2026-11-22
- Related: FEAT messaging, ADR-0010, ADR-0012

## Context

Pinpoint needs 1:1 text messages without turning the map app into a
server-readable chat. A full Signal Protocol implementation (X3DH + Double
Ratchet, prekeys, multi-device, sealed sender) is a separate product. Signal's
`libsignal` is AGPL-3.0, while this repository is MIT, so linking it would
change the licence of the combined work. Rolling a Double Ratchet by hand is
not acceptable.

## Decision

Use an ECIES construction over audited primitives:

- Each account has a long-lived X25519 key and an Ed25519 signing key.
- The sender generates an ephemeral X25519 key per message, derives two
  AES-256-GCM keys with HKDF-SHA256 (recipient copy and sender copy), and signs
  the whole envelope with Ed25519.
- The server stores the opaque envelope and the public bundles only. It
  enforces that the sender and recipient are accepted friends and rate-limits
  sends.
- The client shows a safety fingerprint; the recipient verifies the sender's
  Ed25519 signature before decrypting.
- The server never sees plaintext and cannot forge a message that verifies
  under the sender's key.

## Explicit limits

- **No double ratchet.** A stolen long-lived private key can decrypt past
  messages; this is not Signal-grade forward secrecy.
- **One key bundle per account.** Signing in on a new device rotates the key;
  older messages remain readable only on the old device. Multi-device fan-out
  is future work.
- **Text only.** No groups, attachments, calls, push notifications, typing
  indicators, or disappearing messages.
- **Metadata is visible to the server:** sender, recipient, timestamp, size.
- **Web and native are not Signal-compatible.** The protocol is internal to
  Pinpoint; there is no interoperability with Signal clients.

## Rejected alternatives

- **`libsignal` / Signal Protocol:** AGPL-3.0 conflicts with the MIT licence;
  full Signal server and multi-device support are out of scope.
- **Server-readable messages:** contradicts the privacy model.
- **Home-grown Double Ratchet:** unacceptable crypto risk.

## Consequences

- Messaging can ship under the MIT licence with RustCrypto primitives.
- The browser client needs a JS/WASM X25519 + Ed25519 + AES-GCM implementation;
  native Rust uses `x25519-dalek`, `ed25519-dalek`, `hkdf` and `aes-gcm`.
- A later Signal-grade upgrade is a protocol replacement, not a UI change.
- Account deletion must remove message keys and both message directions.

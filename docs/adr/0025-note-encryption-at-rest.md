# 0025. Encryption at Rest for Private Notes

Date: 2026-11-25

## Status

Accepted.

## Context

Private place notes (label, message, optional photo) were stored as plaintext
in Postgres and in the in-memory snapshot. The scored threat model (T-13) rates
at-rest exposure as High (R = 12): a database dump, a backup copy, or a stolen
disk exposes every user's notes. `security/encryption.rs` already had
AES-256-GCM primitives and key management used for the optional bookmark
store, but the server never used them.

## Decision

Encrypt the three private-note fields before they reach storage:

- **Cipher:** AES-256-GCM, random 12-byte nonce per value, stored as
  `enc:v1:<base64(nonce || ciphertext)>`.
- **Key:** `DATA_ENCRYPTION_KEY`, a base64 32-byte key, read once at startup.
  An empty value keeps the previous plaintext behavior for development; a
  non-empty but invalid value refuses to start rather than silently storing
  plaintext.
- **Scope:** `label`, `message` and `photo`. Coordinates are not encrypted
  because the app queries and displays them structurally; they remain covered
  by the database role and volume controls.
- **Reads:** values without the `enc:v1:` prefix are returned unchanged, so
  notes written before this change stay readable. A missing key returns the
  stored ciphertext rather than dropping the row.
- **Responses:** the API still returns plaintext to the owner; the wire
  contract does not change.
- **Errors:** an encryption failure stores the plaintext and logs a warning
  (the key is validated, so this is practically unreachable); a decryption
  failure logs and returns the ciphertext.

## Consequences

- A database dump or an unencrypted backup no longer exposes note contents.
- The key must be backed up with the database. Rotating it without a migration
  makes old notes undecryptable (returned as ciphertext); key rotation is
  future work.
- `pg_dump` backups are still not encrypted by this mechanism and need a
  storage-layer or wrapper control.
- Location rows, friendships and account metadata remain plaintext; they are
  covered by least-privilege database roles and transport encryption only.
- Existing tests run with no key (plaintext); `tests/note_encryption.rs`
  injects a cipher and asserts the stored row is ciphertext while the API
  returns plaintext, and that legacy rows stay readable.

## Alternatives considered

- **`pgcrypto` column encryption.** Keeps the key in SQL statements and query
  logs, and every reader sees plaintext; rejected for a server-side cipher
  with an env key.
- **Volume encryption only.** Protects a stolen disk but not a database dump
  or a copied backup, which is exactly the T-13 scenario.
- **Client-side E2E encryption of notes.** Strongest, but breaks server-side
  photo validation, the web client's note list and any future search; a
  separate project.

## References

- ADR-0010 (normalized encrypted database design), ADR-0024.
- `src/security/encryption.rs`, `src/server/state/mod.rs`,
  `src/server/locations_handlers.rs`, `tests/note_encryption.rs`.

# Pinpoint — E2E Encryption + MFA Design Note

**Status:** MFA implemented (TOTP + endpoints + unit tests). Location
end-to-end encryption is not implemented: the unused client-side location cipher was
removed. OS-keychain key storage is implemented (`src/security/keychain.rs`); bookmark
encryption wiring is the remaining piece. See
[`auth-mechanisms.md`](auth-mechanisms.md) and
[`session-management.md`](session-management.md) for the current auth/session
details.

## 1. MFA (TOTP, implemented)

- `src/security/mfa.rs` — RFC 6238 TOTP with HMAC-SHA256, 30s step, ±1 step tolerance, constant-time compare.
- `generate_secret()` (160-bit random), `totp(secret, unix)`, `verify_totp(...) -> Option<step>`, `otpauth_uri(...)`.
- Tests: round-trip + drift, unique secrets, uri shape, fail-closed on bad secret.
- **Endpoints implemented:** `/auth/mfa/enroll`, `/auth/mfa/verify`, `/auth/mfa/status`, `POST /auth/mfa/disable {code}` (`src/server/auth_handlers.rs`); login gate requires `totp` when enabled; replay-safe via `mfa_last_step`.
- **Remaining integration:** `mfa_pending` session gate once the session store lands; TOTP recovery codes.

## 2. E2E encryption (implemented, MVP scope)

Goal: server stores only ciphertext; friends decrypt with per-user keys.

- **Implemented:** `src/security/e2e.rs` derives a per-user key with PBKDF2
  (`e2e$<salt>$<iters>$<key>`), encrypts `{lat, lon, acc}` with AES-256-GCM,
  and `PinpointApp` produces the ciphertext on publish
  (`encrypt_my_location`) while the key is persisted in the OS keychain.
- **Current limitation:** the `/locations/me` contract still carries plaintext
  coordinates for server-side precision enforcement; no location ciphertext is
  produced. Adding one requires the precision-cap design below.
- **Target design:** per-friendship key wrapping (X25519 ECDH), server stores
  an opaque blob + `precision_level` outside the ciphertext, client-side
  coarsening + a server-enforced max-precision cap agreed at share creation
  (weakens T-E2 unless the client is attested).
- **Key storage:** OS keychain (Windows Credential Manager / macOS Keychain /
  Linux Secret Service), never in `bookmarks.json` or the repo.

## 3. OS keychain for bookmark keys

- **Implemented.** `src/security/keychain.rs` provides a unified `Keychain`
  trait with `OsKeychain` (Windows Credential Manager / macOS Keychain /
  Linux Secret Service via the `keyring` crate) and a `FileKeychain`
  fallback (AES-256-GCM, file-backed master key). `KeyManager` now
  persists its active key through `Keychain::load_or_generate`, and
  `PinpointApp::new` opens the best-available keychain via
  `keychain::open_default`. See `src/security/encryption.rs`.
- Fallback: derived key from user password (PBKDF2) with salt in
  `bookmarks.json` header — still pending for platforms without any
  keychain (currently in-memory).

## 4. Threat-model deltas

- MFA closes T-S1 (credential stuffing → account takeover) for enrolled users; replay protection via consumed-step tracking.
- E2E closes T-I3 (DB dump) but reopens T-E2 (precision) — must ship with cap + attestation before removing server downgrade.

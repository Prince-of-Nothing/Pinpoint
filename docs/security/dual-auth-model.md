# Dual Authentication Model — Design Note (anonymous vs identity-linked)

Companion to [ADR-0009](../adr/0009-dual-auth-anonymous-and-identity.md).
Status: **Implemented** for anonymous device-bound accounts and email/password;
magic-link, OAuth and passkey login remain future work.
See also [`session-management.md`](session-management.md) and
[`auth-mechanisms.md`](auth-mechanisms.md).

## 1. The two models

| | **Model A — Anonymous** | **Model B — Identity-linked** |
|---|---|---|
| Sign-up | One tap, automatic | Email+password (temporary password when admin-provisioned); magic link/OAuth/passkey future |
| PII stored | **None** (random ID + public key) | Email or IdP subject |
| Credential | Device key pair (Ed25519) in OS keychain | Password hash / IdP / passkey |
| Recovery | Optional recovery code / backup (user-held) | Admin-issued temporary password; no email self-service yet |
| Cross-device | Export/import key or recovery code | Seamless (sign in again) |
| Abuse risk | High (Sybil) → needs controls | Lower |
| Privacy | **Maximum** | Trade-off (IdP linkage) |

Both models produce **one user row**; identity is optional metadata.

## 2. Flows

### 2.1 Anonymous sign-up (Model A)

```
Client                                   Server
  │  first launch, no session              │
  │  generate Ed25519 keypair              │
  │  store private key in OS keychain      │
  │  POST /api/v1/auth/anonymous           │
  │    { public_key, attestation? }        │
  │ ─────────────────────────────────────► │  create user { id: anon_…, public_key }
  │                                        │  issue session cookie + CSRF
  │ ◄───────────────────────────────────── │  { user, session }
  │  (optional) show recovery code once    │
```

- The server never sees the private key.
- The public key doubles as the account identifier (or is mapped to a random
  `anon_…` ID).
- Optional device attestation (App Attest / Play Integrity) proves the
  request comes from a genuine app install.

### 2.2 Anonymous sign-in (same device)

- The client signs a server challenge with the private key
  (`POST /api/v1/auth/anonymous/verify` → `{ challenge_id, signature }`).
- The server verifies with the stored public key and issues a session.

### 2.3 Anonymous recovery (new device)

```
Client                                   Server
  │  "Restore with recovery code"          │
  │  POST /api/v1/auth/anonymous/recover   │
  │    { recovery_code }                   │
  │ ─────────────────────────────────────► │  look up user by hashed code
  │                                        │  issue session; client generates
  │ ◄───────────────────────────────────── │  new keypair and links it
```

- The recovery code is shown **once** at account creation and stored only by
  the user (like a backup phrase). The server stores a hash of it.
- Without the code, the account is unrecoverable **by design** — this must be
  stated in the UI.

### 2.4 Identity-linked sign-up/in (Model B)

- **Email + password** (existing, ADR-0003).
- **Email magic link** (passwordless): `POST /api/v1/auth/magic-link` → email
  with a signed, expiring link → `POST /api/v1/auth/magic-link/verify`.
- **OAuth/OIDC** (Google/Apple): `POST /api/v1/auth/oauth/{provider}` →
  redirect → callback → session. Store the IdP `sub` + email.
- **Passkeys** (WebAuthn): registration + authentication ceremonies; the
  credential is device-bound but syncable via the platform (iCloud Keychain /
  Google Password Manager).

### 2.5 Upgrade path (anonymous → identity)

```
Client                                   Server
  │  authenticated anonymous session       │
  │  POST /api/v1/auth/link                │
  │    { email | oauth | passkey proof }   │
  │ ─────────────────────────────────────► │  attach identity to the same user row
  │ ◄───────────────────────────────────── │  session preserved
```

- Preserves the anonymous history (friends, shares, bookmarks).
- Gives the user recovery + cross-device sign-in.
- Reversible: the identity can be unlinked (returning to anonymous) if the
  user prefers, with the same trade-offs.

## 3. Data model changes

`User` (in `src/models.rs` + server store) gains:

| Field | Type | Notes |
|---|---|---|
| `auth_method` | enum | `anonymous` \| `email` \| `oauth` \| `passkey` |
| `identity_subject` | Option<String> | email or IdP `sub`; null for anonymous |
| `public_key` | Option<String> | device public key / key id for anonymous |
| `recovery_code_hash` | Option<String> | SHA-256 of the 160-bit one-time recovery code |

Existing accounts default to `auth_method = email` with `identity_subject`
set. **Status:** these fields are landed in `src/models.rs`; migration 14 adds
the columns and the `pinpoint_sessions` table.

## 3.1 Session integration

Both auth models use the **same session machinery** (see
[`session-management.md`](session-management.md)):

- On successful auth (password, recovery code, device key) the server creates
  a `pinpoint_sessions` row and issues the `HttpOnly` session cookie +
  `X-CSRF-Token` (opaque sessions, no JWT — ADR-0012).
- The session row records `auth_method`, so downstream services and the audit
  log know whether the user is anonymous or identity-linked.
- Anonymous users can re-authenticate with the recovery code
  (`/auth/anonymous/recover`), which rotates the code on every use; the same
  session machinery handles revocation.
- `mfa_pending` applies only to identity sessions (anonymous accounts have no
  TOTP by default; MFA can be added later if an identity is linked).

## 3.2 Public identity: generated usernames

Every account — anonymous or identity-linked — gets a stable public handle
derived from its numeric id, e.g. `calm-otter-42`
(`src/server/usernames.rs`). The id suffix guarantees uniqueness without a
schema change, and the handle is calculated on read, so existing rows need no
backfill.

| Visibility | Fields | Who can see it |
|---|---|---|
| Public card | `username`, `display_name` (= username), `auth_method` | Any signed-in user who knows the exact handle |
| Friend profile | public card + `full_name`, `email` | Accepted friends only |
| Private record | + `id`, `role`, `is_active`, `must_change_password` | The account itself and admins |

Rules enforced server-side:

- **Exact-match lookup only.** `GET /api/v1/users/lookup?username=` matches the
  whole handle, case-insensitively; there is no substring, real-name or email
  search for non-admins. `/api/v1/users/search` is admin-only.
- **Numeric IDs are not walkable.** `GET /api/v1/users/{id}` returns 401 when
  signed out, the private record for self/admin, the friend profile for
  accepted friends, the public card for a pending request, and 404 otherwise.
- **Pending requests carry handles, not legal names** (`requester_name` /
  `receiver_name` in the friendship rows), so accepting is what reveals the
  real name and email.
- **Friend requests accept `receiver_username`.** The legacy exact
  `receiver_email` path remains for the desktop client; both resolve to the
  same account and never echo the email back to the requester.

## 4. API surface

| Method | Path | Model | Status |
|---|---|---|---|
| POST | `/api/v1/auth/anonymous` | A | ✅ implemented (returns one-time recovery code) |
| POST | `/api/v1/auth/anonymous/verify` | A | ⬜ pending (device challenge-response) |
| POST | `/api/v1/auth/anonymous/recover` | A | ✅ implemented (rotates the code) |
| GET | `/api/v1/users/lookup?username=` | A/B | ✅ implemented (exact handle → public card) |
| POST | `/api/v1/friendships/` | A/B | ✅ implemented (`receiver_username`, legacy email) |
| POST | `/api/v1/auth/link` | A→B | ⬜ pending |
| POST | `/api/v1/auth/unlink` | B→A | ⬜ pending (optional) |
| POST | `/api/v1/auth/magic-link` | B | ⬜ pending |
| POST | `/api/v1/auth/magic-link/verify` | B | ⬜ pending |
| POST | `/api/v1/auth/oauth/{provider}` | B | ⬜ pending |
| GET | `/api/v1/auth/oauth/{provider}/callback` | B | ⬜ pending |
| POST | `/api/v1/auth/passkey/register` | B | ⬜ pending |
| POST | `/api/v1/auth/passkey/verify` | B | ⬜ pending |

## 5. Client changes

- **Onboarding choice:** "Continue anonymously" vs "Sign in" (email/Google/
  Apple/passkey). Clear explanation of the trade-offs.
- **Key generation:** Ed25519 via `src/security/`; private key in
  `keychain.rs` (`OsKeychain`/`FileKeychain`).
- **Recovery code UI:** show once, allow export (QR / file), import on a new
  device.
- **Account settings:** show current model, link/unlink identity, reveal
  recovery hint.

## 6. Abuse / Sybil control

Anonymous accounts are cheap to create. Mitigations (choose per deployment):

1. **Rate limiting (implemented):** anonymous creation and recovery share the
   per-client HMAC throttle (20 requests per 10 minutes); per-IP and
   per-device controls beyond that remain deployment tuning.
2. **Device attestation:** Apple App Attest / Android Play Integrity to prove
   a genuine app install (future work).
3. **Invite codes:** optional, for closed beta.
4. **Proof of work:** lightweight client puzzle (optional).
5. **Behavioural limits:** anonymous accounts start with tighter share/quota
   limits until they link an identity or build trust.

## 7. What similar apps do

| App | Anonymous? | Identity required | Recovery |
|---|---|---|---|
| Google Maps LS | No | Google account | Account recovery |
| Apple Find My | No | Apple ID | Apple ID recovery |
| Life360 | No | Email/phone | Account recovery |
| Signal | No | Phone number | PIN / backup |
| Session | **Yes** | None (blockchain ID) | Backup phrase |
| Sign in with Apple | Partial | Apple ID (relay email) | Apple ID |
| Passkeys | Partial | Platform account | Platform sync |

Pinpoint would be the only location-sharing app offering an **explicit choice**
between anonymous and identity-linked accounts, plus an **upgrade path**.

## 8. Effort & recommendation

| Phase | Scope | Effort |
|---|---|---|
| 0 | Document (this note + ADR) | ✅ done |
| 1 | Session store + middleware + real login/register/logout (replace mock) | ✅ done |
| 2 | Anonymous model: `/auth/anonymous` + one-time recovery code + rotation | ✅ done (device challenge-response remains a follow-up) |
| 3 | Refresh rotation (migration 15) + client refresh-on-401 | ❌ superseded by ADR-0012 (opaque sessions) |
| 4 | Identity-linked: magic link + OAuth + passkeys | Large — pending |
| 5 | Upgrade path + abuse controls | Medium — pending |

**Recommendation:** adopt as a **design contribution** for the thesis; Phases 1–2
are implemented, so the demo shows the two-model story with working code.
OAuth/passkeys stay out of the MVP demo (Phase 4).

**Implementation status:** Phases 1–2 **landed** — real sessions, anonymous
creation, one-time recovery code with rotation, session list/revoke and the web
UI. Anonymous creation/recovery is rate-limited per client; device
challenge-response (`/auth/anonymous/verify`) remains a follow-up. See
`session-management.md` §10 for the checklist.

## 9. Risks

- **Account loss** for anonymous users without a backup — must be explicit.
- **Abuse:** anonymous creation is rate-limited per client, but device
  attestation remains future work.
- **Scope creep** — keep OAuth/passkeys out of the MVP demo.
- **Key management** — the OS keychain is the trust root; losing it loses the
  account (same as Model B losing the password, minus the reset path).

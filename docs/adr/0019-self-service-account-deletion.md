# 0019. Self-service account deletion

Date: 2026-11-16

## Status

Accepted

## Context

The privacy policy promises erasure, but the only deletion path was an
admin-only `DELETE /users/{id}`. A public launch needs a user-facing erasure
path that also revokes sessions and removes local key material.

## Decision

- Add `DELETE /api/v1/users/me` with
  `{ "confirm": "DELETE", "current_password": "..." }`.
- Every account must type `DELETE`. Email accounts must also prove the current
  password; anonymous accounts have no password and use the confirmation phrase
  only.
- The server removes the profile, friendships, shares owned by the user, current
  location, check-ins, emergencies and all sessions, then clears the session
  cookie.
- Android and the web client clear the persisted session and local E2E key and
  return to sign-in.
- The admin `DELETE /users/{id}` remains for moderation but rejects self-deletion
  and points to the self-service endpoint.

## Consequences

- Users can exercise erasure without contacting support.
- A deleted account cannot sign in and its sessions are revoked immediately.
- The action is irreversible; the UI requires a typed confirmation and, for
  email accounts, the current password.
- Audit logging records `user.self_delete` for operator visibility.

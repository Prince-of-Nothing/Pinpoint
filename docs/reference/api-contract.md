# MVP API Contract

Version: `api/v1`
Base URL: `https://api.always-together.local/api/v1`

All responses use JSON. Error responses follow Problem Details (RFC 7807).

## Authentication & Identity

### OIDC/PKCE Initiation
- `GET /api/v1/oauth/pkce/challenge` — Initiate PKCE flow; returns
  `code_challenge` and `code_challenge_method`. The mobile client generates a
  verifier and sends the challenge request.

### Token Exchange
- `POST /api/v1/oauth/token` — Exchange authorization code for tokens.
  - Body: `code`, `code_verifier`, `grant_type`="authorization_code"
  - Returns: `access_token`, `refresh_token`, `expires_in`, `token_type`,
    `scope`
  - Access tokens are short-lived (15m); refresh tokens are rotated on use.

### Revoke Token
- `POST /api/v1/oauth/revoke` — Revoke an access or refresh token.

### Get Current User
- `GET /api/v1/users/me` — Get the authenticated user's profile. Response is
  filtered by the viewer's relationship (self only).

## Users & Profiles

### Get User Profile
- `GET /api/v1/users/{id}` — Get a user's profile. Response data varies
  based on the viewer's relationship to the subject:
  - **Self**: full profile including image, nickname, status, location
    visibility
  - **Friend**: profile without precise location or recent activity
  - **Non-friend/Group member/Blocked**: empty profile (no image, no status,
    no location)

## Friendships

### Send Friendship Request
- `POST /api/v1/friendships/` — Send a friendship request to another user.
  - Body: `target_user_id`
  - Returns: friendship request ID, status, created_at

### Get Friendship Status
- `GET /api/v1/friendships/{id}/status` — Check the status of a friendship
  request.
  - Returns: `state` (pending, accepted, declined, revoked, blocked),
    `created_at`, `updated_at`

### Accept Friendship
- `POST /api/v1/friendships/{id}/accept` — Accept a pending friendship
  request.
  - Returns: updated friendship state

### Decline Friendship
- `POST /api/v1/friendships/{id}/decline` — Decline a pending friendship
  request.

### Revoke Friendship Request
- `POST /api/v1/friendships/{id}/revoke` — Revoke a sent friendship request.

### Block User
- `POST /api/v1/friendships/{id}/block` — Block a user. Prevents all future
  friendship requests and location sharing.

## Directional Location Sharing

### Share Location
- `POST /api/v1/location-shares/` — Share current location with a selected
  friend.
  - Body: `recipient_user_id`, `precision` (exact|approximate), `purpose`,
    `expires_at`
  - Returns: location share ID, recipient, precision, created_at, expiry

### Get Location Share
- `GET /api/v1/location-shares/{id}` — Get the status of a location share
  record.
  - Returns: owner, recipient, precision, purpose, expiry, is_active,
    last_update

### Revoke Location Share
- `DELETE /api/v1/location-shares/{id}` — Revoke an active location share.
  The recipient immediately loses access to the location.

## Check-Ins

### Create Check-In
- `POST /api/v1/check-ins/` — Create a manual, scheduled, or trip check-in.
  - Body: `type` (manual|scheduled|trip), `scheduled_at` (for scheduled),
    `trip_id` (for trip), `metadata`
  - Returns: check-in ID, type, status, created_at

### Get Check-In Status
- `GET /api/v1/check-ins/{id}/status` — Get the current status of a
  check-in.
  - Returns: `state` (pending, reminder_sent, grace_period, completed,
    cancelled, missed), `time_remaining`, `notes`

### Complete Check-In
- `POST /api/v1/check-ins/{id}/complete` — Manually complete a check-in
  before the timer expires.

### Cancel Check-In
- `POST /api/v1/check-ins/{id}/cancel` — Cancel a scheduled or trip
  check-in.

### Acknowledge Reminder
- `POST /api/v1/check-ins/{id}/acknowledge` — Acknowledge a reminder
  notification for a check-in.

## Stale-State & Latest Locations

### Get Friends' Latest Locations
- `GET /api/v1/latest_locations` — Get the latest known locations of all
  friends the authenticated user is authorized to see. Each entry includes:
  friend user ID, last-known location (lat/lng), accuracy, battery level,
  last-update timestamp, stale/dead status.

## Map & Navigation

### Get Friends for Map
- `GET /api/v1/map/friends` — Get friends the authenticated user is
  authorized to display on the map. Each entry includes: user ID, display
  name, approximate location, precision label. Unauthorized friends are
  excluded.

### Generate Navigation Deep Link
- `GET /api/v1/map/navigation/deep-link?recipient={user_id}&destination={lat,lng}`
  — Generate a deep link for an external maps application (Apple Maps,
  Google Maps, Waze). The link includes only the target coordinate and a
  minimal label; no friendship information, sharing status, location
  history, or other private data is included in the link query parameters.

## Error Responses (Problem Details)

All error responses follow RFC 7807:

```json
{
  "type": "about://errors/invalid-request",
  "title": "Bad Request",
  "status": 400,
  "detail": "Specific explanation",
  "instance": "/api/v1/users/me"
}
```

HTTP status codes used:

- **400** — Invalid input (malformed coordinates, bad PKCE, validation
  failure)
- **401** — Missing or invalid authentication/token
- **403** — Authorization denied (not a friend, share revoked, blocked, IDOR
  attempt)
- **404** — Resource not found (invalid user ID, expired share)
- **429** — Rate limit exceeded
- **500** — Internal server error (never expose stack traces)

---
*See also [Domain Data Model](./domain-data-model.md) for the underlying entities and
[Security Architecture](../architecture/security-architecture-and-data-protection.md)
for the authorization sequence applied before every response above.*

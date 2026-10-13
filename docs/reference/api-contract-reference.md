# API Contract Reference

**Version:** `api/v1`  
**Base URL:** `https://api.always-together.local/api/v1`  
**WebSocket URL:** `wss://api.always-together.local/ws`  

All HTTP responses use JSON. Error responses follow Problem Details format (RFC 7807).

## Authentication Endpoints

### POST /oauth/token

Exchange authorization code for access and refresh tokens using OAuth 2.0 with PKCE.

**Request:**
```json
{
  "grant_type": "authorization_code",
  "code": "auth_code_abc123",
  "code_verifier": "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
  "client_id": "mobile-app-client"
}
```

**Response (200 OK):**
```json
{
  "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 900,
  "refresh_token": "rt_9f8e7d6c5b4a3210fedcba9876543210",
  "scope": "profile:read location:write checkin:write"
}
```

**Errors:**
- `400 Bad Request` — Invalid code_verifier, expired authorization code
- `401 Unauthorized` — Invalid client credentials

---

### POST /oauth/revoke

Revoke an access or refresh token.

**Request:**
```json
{
  "token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type_hint": "access_token"
}
```

**Response (200 OK):**
```json
{
  "revoked": true,
  "timestamp": "2024-11-15T10:30:00Z"
}
```

---

## User Endpoints

### GET /users/me

Get the authenticated user's full profile.

**Response (200 OK):**
```json
{
  "id": "usr_7f8d9e2a1b3c4d5e",
  "username": "traveler_alex",
  "email": "alex@example.com",
  "profile": {
    "nickname": "Alex",
    "image_url": "https://storage.example.com/profiles/usr_7f8d9e2a1b3c4d5e/avatar.jpg",
    "status": {
      "text": "Exploring downtown!",
      "visibility": "friends",
      "expires_at": "2024-11-15T23:59:59Z"
    }
  },
  "location_sharing": {
    "is_sharing": true,
    "active_recipients": 3,
    "default_precision": "approximate"
  },
  "safety_status": {
    "check_in_active": false,
    "last_check_in_completed": "2024-11-14T18:30:00Z"
  },
  "created_at": "2024-09-01T12:00:00Z",
  "updated_at": "2024-11-15T09:00:00Z"
}
```

---

### GET /users/{id}

Get another user's profile. Response varies by relationship:

**Self View (200 OK):**
```json
{
  "id": "usr_7f8d9e2a1b3c4d5e",
  "username": "traveler_alex",
  "profile": {
    "nickname": "Alex",
    "image_url": "https://storage.../avatar.jpg",
    "status": {"text": "Exploring!", "visibility": "friends"}
  },
  "location_visibility": {
    "sharing_enabled": true,
    "default_precision": "approximate",
    "active_recipients": ["usr_abc123", "usr_def456"]
  }
}
```

**Friend View (200 OK):**
```json
{
  "id": "usr_7f8d9e2a1b3c4d5e",
  "username": "traveler_alex",
  "profile": {
    "nickname": "Alex",
    "image_url": "https://storage.../avatar.jpg",
    "status": {"text": "Exploring!", "visibility": "friends"}
  },
  "location_visibility": {
    "sharing_enabled": false
  }
}
```

**Non-Friend View (200 OK):**
```json
{
  "id": "usr_redacted",
  "username": "",
  "profile": {
    "nickname": "",
    "image_url": null,
    "status": null
  }
}
```

---

## Friendship Endpoints

### POST /friendships/

Send a friendship request.

**Request:**
```json
{
  "target_user_id": "usr_abc123def456"
}
```

**Response (201 Created):**
```json
{
  "friendship_id": "frnd_1a2b3c4d5e6f",
  "requester_id": "usr_7f8d9e2a1b3c4d5e",
  "target_id": "usr_abc123def456",
  "state": "pending",
  "created_at": "2024-11-15T10:30:00Z",
  "updated_at": "2024-11-15T10:30:00Z"
}
```

**Errors:**
- `400 Bad Request` — Invalid target_user_id
- `403 Forbidden` — Target user is blocked
- `409 Conflict` — Friendship already exists or pending
- `429 Too Many Requests` — Rate limit exceeded (max 10 requests/hour)

---

### GET /friendships/{id}/status

Check friendship status.

**Response (200 OK):**
```json
{
  "friendship_id": "frnd_1a2b3c4d5e6f",
  "requester_id": "usr_7f8d9e2a1b3c4d5e",
  "target_id": "usr_abc123def456",
  "state": "accepted",
  "created_at": "2024-11-15T10:30:00Z",
  "updated_at": "2024-11-15T11:00:00Z"
}
```

**States:** `pending`, `accepted`, `declined`, `revoked`, `blocked`

---

### POST /friendships/{id}/accept

Accept a pending friendship request.

**Response (200 OK):**
```json
{
  "friendship_id": "frnd_1a2b3c4d5e6f",
  "state": "accepted",
  "updated_at": "2024-11-15T11:00:00Z"
}
```

---

### POST /friendships/{id}/block

Block a user. Prevents all future interactions.

**Response (200 OK):**
```json
{
  "friendship_id": "frnd_1a2b3c4d5e6f",
  "state": "blocked",
  "updated_at": "2024-11-15T11:30:00Z"
}
```

---

## Location Sharing Endpoints

### POST /location-shares/

Create a directional location share.

**Request:**
```json
{
  "recipient_user_id": "usr_abc123def456",
  "precision": "approximate",
  "purpose": "safety",
  "expires_at": "2024-11-15T22:00:00Z"
}
```

**Response (201 Created):**
```json
{
  "share_id": "loc_share_9z8y7x6w",
  "owner_id": "usr_7f8d9e2a1b3c4d5e",
  "recipient_id": "usr_abc123def456",
  "precision": "approximate",
  "purpose": "safety",
  "is_active": true,
  "created_at": "2024-11-15T10:30:00Z",
  "expires_at": "2024-11-15T22:00:00Z"
}
```

**Validation Rules:**
- `precision`: Must be `exact` or `approximate`
- `purpose`: Must be one of `safety`, `meetup`, `navigation`, `check-in`
- `expires_at`: Maximum 24 hours from creation

**Errors:**
- `400 Bad Request` — Invalid precision, purpose, or expiry
- `403 Forbidden` — Not mutual friends
- `429 Too Many Requests` — Rate limit exceeded

---

### GET /location-shares/{id}

Get location share status.

**Response (200 OK):**
```json
{
  "share_id": "loc_share_9z8y7x6w",
  "owner_id": "usr_7f8d9e2a1b3c4d5e",
  "recipient_id": "usr_abc123def456",
  "precision": "approximate",
  "purpose": "safety",
  "is_active": true,
  "created_at": "2024-11-15T10:30:00Z",
  "expires_at": "2024-11-15T22:00:00Z",
  "last_update": "2024-11-15T10:25:00Z"
}
```

---

### DELETE /location-shares/{id}

Revoke an active location share.

**Response (200 OK):**
```json
{
  "share_id": "loc_share_9z8y7x6w",
  "is_active": false,
  "revoked_at": "2024-11-15T11:00:00Z",
  "reason": "user_revoked"
}
```

---

### GET /latest_locations

Get latest locations of all authorized friends.

**Response (200 OK):**
```json
{
  "friends_locations": [
    {
      "friend_id": "usr_abc123",
      "friend_username": "sam_explorer",
      "location": {
        "latitude": 40.7128,
        "longitude": -74.0060,
        "accuracy_meters": 15,
        "precision": "approximate"
      },
      "freshness": {
        "last_update": "2024-11-15T10:25:00Z",
        "battery_level": 78,
        "is_stale": false,
        "staleness_reason": null
      }
    },
    {
      "friend_id": "usr_def456",
      "friend_username": "jamie_hiker",
      "location": {
        "latitude": 40.7580,
        "longitude": -73.9855,
        "accuracy_meters": 25,
        "precision": "exact"
      },
      "freshness": {
        "last_update": "2024-11-15T09:50:00Z",
        "battery_level": 45,
        "is_stale": true,
        "staleness_reason": "old_update"
      }
    }
  ]
}
```

**Staleness Threshold:** 30 minutes since last update

---

## Check-In Endpoints

### POST /check-ins/

Create a safety check-in.

**Request (Scheduled):**
```json
{
  "type": "scheduled",
  "scheduled_at": "2024-11-15T20:00:00Z",
  "metadata": {
    "activity": "Late night study session",
    "expected_location": "Library"
  }
}
```

**Request (Trip):**
```json
{
  "type": "trip",
  "destination": {
    "latitude": 40.7580,
    "longitude": -73.9855,
    "label": "Times Square"
  },
  "eta_minutes": 45,
  "metadata": {
    "transport_mode": "walking"
  }
}
```

**Response (201 Created):**
```json
{
  "check_in_id": "chk_5v4w3x2y1z",
  "type": "scheduled",
  "status": "pending",
  "scheduled_at": "2024-11-15T20:00:00Z",
  "grace_period_minutes": 15,
  "created_at": "2024-11-15T10:30:00Z"
}
```

---

### GET /check-ins/{id}/status

Get check-in status.

**Response (200 OK):**
```json
{
  "check_in_id": "chk_5v4w3x2y1z",
  "type": "scheduled",
  "state": "reminder_sent",
  "time_remaining_seconds": 1800,
  "notes": null
}
```

**States:** `pending`, `reminder_sent`, `grace_period`, `completed`, `cancelled`, `missed`

---

### POST /check-ins/{id}/complete

Manually complete a check-in.

**Response (200 OK):**
```json
{
  "check_in_id": "chk_5v4w3x2y1z",
  "state": "completed",
  "completed_at": "2024-11-15T19:45:00Z"
}
```

---

### POST /check-ins/{id}/cancel

Cancel a scheduled check-in.

**Response (200 OK):**
```json
{
  "check_in_id": "chk_5v4w3x2y1z",
  "state": "cancelled",
  "cancelled_at": "2024-11-15T19:30:00Z",
  "reason": "user_cancelled"
}
```

---

## Map Endpoints

### GET /map/friends

Get friends authorized for map display.

**Response (200 OK):**
```json
{
  "friends": [
    {
      "user_id": "usr_abc123",
      "display_name": "Sam",
      "approximate_location": {
        "latitude": 40.71,
        "longitude": -74.01
      },
      "precision_label": "approximate",
      "last_update": "2024-11-15T10:25:00Z"
    }
  ]
}
```

---

### GET /map/navigation/deep-link

Generate external navigation app deep link.

**Request:**
```
GET /map/navigation/deep-link?recipient=usr_abc123&destination=40.7580,-73.9855
```

**Response (200 OK):**
```json
{
  "deep_link": "comgooglemaps://?daddr=40.7580,-73.9855&directionsmode=walking",
  "provider": "google_maps",
  "destination": {
    "latitude": 40.7580,
    "longitude": -73.9855
  }
}
```

**Supported Providers:** `google_maps`, `apple_maps`, `waze`

---

## WebSocket Events

### Connection

Connect to WebSocket endpoint:
```
wss://api.always-together.local/ws?token=<access_token>
```

### Server → Client Events

**location_updated:**
```json
{
  "event": "location_updated",
  "data": {
    "friend_id": "usr_abc123",
    "location": {
      "latitude": 40.7128,
      "longitude": -74.0060,
      "accuracy_meters": 15
    },
    "timestamp": "2024-11-15T10:30:00Z"
  }
}
```

**check_in_reminder:**
```json
{
  "event": "check_in_reminder",
  "data": {
    "check_in_id": "chk_5v4w3x2y1z",
    "time_remaining_seconds": 900,
    "message": "Don't forget to check in!"
  }
}
```

**share_revoked:**
```json
{
  "event": "share_revoked",
  "data": {
    "share_id": "loc_share_9z8y7x6w",
    "revoked_by": "owner",
    "timestamp": "2024-11-15T11:00:00Z"
  }
}
```

### Client → Server Events

**update_location:**
```json
{
  "event": "update_location",
  "data": {
    "latitude": 40.7128,
    "longitude": -74.0060,
    "accuracy_meters": 10,
    "battery_level": 78,
    "timestamp": "2024-11-15T10:30:00Z"
  }
}
```

---

## Error Response Format

All errors follow RFC 7807 Problem Details:

```json
{
  "type": "about://errors/invalid-request",
  "title": "Bad Request",
  "status": 400,
  "detail": "Specific explanation of what went wrong",
  "instance": "/api/v1/users/me",
  "errors": [
    {
      "field": "email",
      "message": "Invalid email format"
    }
  ]
}
```

### Error Types

| Type | Status | Description |
|------|--------|-------------|
| `about://errors/invalid-request` | 400 | Malformed request or validation failure |
| `about://errors/unauthorized` | 401 | Missing or invalid authentication |
| `about://errors/forbidden` | 403 | Authorization denied (not friend, blocked, etc.) |
| `about://errors/not-found` | 404 | Resource does not exist |
| `about://errors/rate-limit-exceeded` | 429 | Too many requests |
| `about://errors/internal-error` | 500 | Internal server error |

---

## Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/oauth/token` | 10 requests | per minute per IP |
| `/friendships/` | 10 requests | per hour per user |
| `/location-shares/` | 20 requests | per hour per user |
| `/check-ins/` | 50 requests | per hour per user |
| All other endpoints | 100 requests | per minute per user |

Rate limit headers included in responses:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1699900800
```

---

*For security implementation details, see [API Security Implementation](./api-security-implementation.md).*  
*For data model definitions, see [Domain Data Model](./domain-data-model.md).*

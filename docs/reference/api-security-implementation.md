# API Security Implementation Guide

This document provides detailed security implementation guidance for all API endpoints. Every endpoint must implement the full authorization sequence before returning data.

## Authentication Flow

### OAuth 2.0 with PKCE (RFC 7636)

```
┌─────────────┐    1. Generate    ┌─────────────┐
│   Mobile    │───code_verifier──▶│   Client    │
│    App      │                   │   (Flutter) │
└─────────────┘                   └──────┬──────┘
                                         │
         2. code_challenge = SHA256(code_verifier)
                                         │
                                         ▼
┌─────────────┐    3. GET /oauth/pkce/challenge
│   Backend   │◀──────────────────────────┤
│   Server    │                           │
└──────┬──────┘                           │
       │                                  │
       │  4. Return code_challenge        │
       │     code_challenge_method="S256" │
       ▼                                  │
┌─────────────┐                           │
│   Backend   │───────────────────────────┘
│   Server    │
└─────────────┘

5. User authorizes in browser/webview
6. Authorization code returned to app

┌─────────────┐    7. POST /oauth/token
│   Mobile    │───code + code_verifier──▶┌─────────────┐
│    App      │                          │   Backend   │
└─────────────┘                          │   Server    │
                                         └──────┬──────┘
                                                │
                                   8. Verify: code_verifier
                                      SHA256(code_verifier) == code_challenge
                                                │
                                   9. Issue: access_token (15m)
                                      refresh_token (rotating)
                                                │
                                         ┌──────▼──────┐
                                         │   Mobile    │
                                         │    App      │
                                         │ (store in   │
                                         │ secure      │
                                         │ enclave/    │
                                         │ Keychain)   │
                                         └─────────────┘
```

### Token Structure

**Access Token (JWT):**
```json
{
  "sub": "usr_7f8d9e2a1b3c4d5e",
  "iss": "https://api.always-together.local",
  "aud": "always-together-mobile",
  "exp": 1699900800,
  "iat": 1699900000,
  "scope": "profile:read location:write checkin:write",
  "device_id": "dev_a1b2c3d4",
  "session_id": "ses_x9y8z7w6"
}
```

**Refresh Token:**
- Opaque string stored server-side
- Rotated on every use
- Bound to device_id and session_id
- Revoked on logout or suspicious activity

## Endpoint Security Details

### POST /api/v1/oauth/token

**Security Controls:**
- Rate limit: 10 requests per minute per IP
- code_verifier validation: SHA256(code_verifier) must match stored code_challenge
- Authorization code: single-use, expires in 10 minutes
- Client authentication: public client (PKCE required)

**Request Validation:**
```python
def validate_token_request(request):
    # Validate required fields
    assert 'code' in request.body
    assert 'code_verifier' in request.body
    assert request.body['grant_type'] == 'authorization_code'
    
    # Validate code_verifier length (43-128 chars)
    verifier = request.body['code_verifier']
    assert 43 <= len(verifier) <= 128
    assert re.match(r'^[a-zA-Z0-9._~-]+$', verifier)
    
    # Retrieve authorization code from database
    auth_code = db.get_authorization_code(request.body['code'])
    assert auth_code is not None
    assert not auth_code.is_used
    assert auth_code.expires_at > now()
    
    # Verify PKCE
    computed_challenge = sha256(verifier)
    assert computed_challenge == auth_code.code_challenge
    
    return True
```

**Response:**
```json
{
  "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6...",
  "token_type": "Bearer",
  "expires_in": 900,
  "refresh_token": "rt_9f8e7d6c5b4a3210...",
  "scope": "profile:read location:write checkin:write"
}
```

**Error Responses:**
```json
// Invalid code_verifier
{
  "type": "about://errors/invalid-grant",
  "title": "Invalid Grant",
  "status": 400,
  "detail": "code_verifier does not match code_challenge",
  "instance": "/api/v1/oauth/token"
}

// Expired authorization code
{
  "type": "about://errors/invalid-grant",
  "title": "Invalid Grant",
  "status": 400,
  "detail": "Authorization code has expired",
  "instance": "/api/v1/oauth/token"
}
```

### GET /api/v1/users/me

**Security Controls:**
- Requires valid access token with `profile:read` scope
- Token expiry checked on every request
- Session validity verified

**Authorization Sequence:**
1. Extract Bearer token from Authorization header
2. Validate JWT signature using public key
3. Check token expiry (`exp` claim)
4. Verify session exists and is not revoked
5. Check device is not blocked
6. Load user profile from database
7. Return self-profile (no relationship filtering needed)

**Response:**
```json
{
  "id": "usr_7f8d9e2a1b3c4d5e",
  "username": "traveler_alex",
  "profile": {
    "nickname": "Alex",
    "image_url": "https://storage.../profiles/usr_.../avatar.jpg",
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
  }
}
```

### GET /api/v1/users/{id}

**Security Controls:**
- Relationship-based response filtering
- No information leakage about existence of non-visible users
- Block checking before any data return

**Authorization Sequence:**
```python
def get_user_profile(viewer_id, target_id):
    # Step 1: Check blocks
    if is_blocked(viewer_id, target_id) or is_blocked(target_id, viewer_id):
        return empty_profile_response()
    
    # Step 2: Determine relationship
    relationship = get_relationship(viewer_id, target_id)
    
    # Step 3: Filter response based on relationship
    if relationship == 'self':
        return full_profile_with_location_settings(target_id)
    elif relationship == 'friend':
        return profile_without_precise_location(target_id)
    elif relationship == 'group_member':
        return minimal_profile(target_id)
    else:
        # Non-friend, non-group-member
        return empty_profile_response()

def empty_profile_response():
    """Return indistinguishable response for non-visible users"""
    return {
        "id": "usr_redacted",
        "username": "",
        "profile": {
            "nickname": "",
            "image_url": null,
            "status": null
        }
    }
```

**Response Variations:**

*Self view:*
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

*Friend view:*
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
    "sharing_enabled": false  // Precise location not included
  }
}
```

*Non-friend view:*
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

### POST /api/v1/friendships/

**Security Controls:**
- Rate limiting to prevent mass friend requests
- Target user existence validation without revealing
- Duplicate request prevention
- Block checking

**Request Validation:**
```python
def send_friendship_request(requester_id, target_user_id):
    # Check rate limit
    recent_requests = count_recent_friend_requests(requester_id, window_minutes=60)
    if recent_requests >= 10:
        raise RateLimitExceeded("Too many friend requests")
    
    # Check if already friends or pending
    existing = get_friendship_status(requester_id, target_user_id)
    if existing and existing.state in ['pending', 'accepted']:
        raise ConflictException("Friendship already exists or pending")
    
    # Check blocks
    if is_blocked(requester_id, target_user_id):
        raise ForbiddenException("Cannot send request to blocked user")
    
    # Create friendship record
    friendship = Friendship.create(
        requester_id=requester_id,
        target_id=target_user_id,
        state='pending',
        created_at=now()
    )
    
    # Send notification to target
    notify_friendship_request(target_user_id, requester_id)
    
    # Audit event
    audit_log('friendship_requested', {
        'requester_id': requester_id,
        'target_id': target_user_id,
        'timestamp': now().isoformat()
    })
    
    return friendship.to_response()
```

**Response:**
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

### POST /api/v1/location-shares/

**Security Controls:**
- Mutual friendship verification
- Explicit consent capture
- Precision and purpose binding
- Expiry enforcement
- Recipient notification

**Authorization Sequence:**
```python
def create_location_share(owner_id, recipient_id, precision, purpose, expires_at):
    # Step 1: Verify mutual friendship
    friendship = get_mutual_friendship(owner_id, recipient_id)
    if not friendship or friendship.state != 'accepted':
        raise ForbiddenException("Mutual friendship required")
    
    # Step 2: Validate precision
    if precision not in ['exact', 'approximate']:
        raise BadRequestException("Invalid precision value")
    
    # Step 3: Validate purpose
    valid_purposes = ['safety', 'meetup', 'navigation', 'check-in']
    if purpose not in valid_purposes:
        raise BadRequestException("Invalid purpose")
    
    # Step 4: Validate expiry (max 24 hours for MVP)
    max_expiry = now() + timedelta(hours=24)
    if expires_at > max_expiry:
        raise BadRequestException("Maximum sharing duration is 24 hours")
    
    if expires_at <= now():
        raise BadRequestException("Expiry must be in the future")
    
    # Step 5: Check existing share
    existing = get_active_share(owner_id, recipient_id)
    if existing:
        # Revoke existing share first
        revoke_location_share(existing.id, reason='replaced')
    
    # Step 6: Create share record
    share = LocationShare.create(
        owner_id=owner_id,
        recipient_id=recipient_id,
        precision=precision,
        purpose=purpose,
        expires_at=expires_at,
        is_active=True,
        created_at=now()
    )
    
    # Step 7: Notify recipient
    notify_location_share_started(recipient_id, owner_id, precision)
    
    # Step 8: Audit event
    audit_log('location_share_created', {
        'owner_id': owner_id,
        'recipient_id': recipient_id,
        'precision': precision,
        'purpose': purpose,
        'expires_at': expires_at.isoformat(),
        'share_id': share.id
    })
    
    return share.to_response()
```

**Request:**
```json
{
  "recipient_user_id": "usr_abc123def456",
  "precision": "approximate",
  "purpose": "safety",
  "expires_at": "2024-11-15T22:00:00Z"
}
```

**Response:**
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

**Precision Transformation:**

When `precision=approximate`, the backend transforms coordinates:
```python
def apply_approximate_precision(lat, lng, precision_level='approximate'):
    """
    Add random noise to coordinates for approximate location.
    Approximate = ~1km uncertainty (0.01 degrees)
    """
    if precision_level == 'approximate':
        # Add random offset up to 0.01 degrees (~1.1km)
        noise_lat = random.uniform(-0.01, 0.01)
        noise_lng = random.uniform(-0.01, 0.01)
        return lat + noise_lat, lng + noise_lng
    elif precision_level == 'coarse':
        # Add random offset up to 0.1 degrees (~11km)
        noise_lat = random.uniform(-0.1, 0.1)
        noise_lng = random.uniform(-0.1, 0.1)
        return lat + noise_lat, lng + noise_lng
    else:
        return lat, lng  # exact
```

### GET /api/v1/latest_locations

**Security Controls:**
- Per-friend authorization check
- Consent expiry validation
- Stale-state detection
- Precision transformation per share settings

**Authorization Sequence:**
```python
def get_friends_latest_locations(viewer_id):
    # Step 1: Get all friends
    friends = get_friends_list(viewer_id)
    
    results = []
    for friend in friends:
        # Step 2: Check active location share
        share = get_active_share(friend.id, viewer_id)
        
        if not share or not share.is_active:
            continue  # No authorized access
        
        # Step 3: Check expiry
        if share.expires_at < now():
            continue  # Share expired
        
        # Step 4: Get latest location update
        location_update = get_latest_location_update(friend.id)
        
        if not location_update:
            # No location data available
            results.append({
                'friend_id': friend.id,
                'status': 'no_location_data',
                'last_update': None
            })
            continue
        
        # Step 5: Check staleness (older than 30 minutes = stale)
        staleness_threshold = now() - timedelta(minutes=30)
        is_stale = location_update.timestamp < staleness_threshold
        
        # Step 6: Apply precision transformation
        if share.precision == 'approximate':
            lat, lng = apply_approximate_precision(
                location_update.latitude,
                location_update.longitude,
                'approximate'
            )
            precision_label = 'approximate'
        else:
            lat, lng = location_update.latitude, location_update.longitude
            precision_label = 'exact'
        
        # Step 7: Build response
        results.append({
            'friend_id': friend.id,
            'friend_username': friend.username,
            'location': {
                'latitude': lat,
                'longitude': lng,
                'accuracy_meters': location_update.accuracy,
                'precision': precision_label
            },
            'freshness': {
                'last_update': location_update.timestamp.isoformat(),
                'battery_level': location_update.battery_level,
                'is_stale': is_stale,
                'staleness_reason': 'old_update' if is_stale else None
            }
        })
    
    # Step 8: Audit access
    audit_log('locations_accessed', {
        'viewer_id': viewer_id,
        'friends_count': len(results),
        'timestamp': now().isoformat()
    })
    
    return {'friends_locations': results}
```

**Response:**
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
        "is_stale": false
      }
    },
    {
      "friend_id": "usr_def456",
      "friend_username": "jordan_travels",
      "location": {
        "latitude": 40.7580,
        "longitude": -73.9855,
        "accuracy_meters": 25,
        "precision": "exact"
      },
      "freshness": {
        "last_update": "2024-11-15T09:15:00Z",
        "battery_level": 12,
        "is_stale": true,
        "staleness_reason": "old_update"
      }
    }
  ]
}
```

## Rate Limiting

### Rate Limit Configuration

| Endpoint | Limit | Window | Action on Exceed |
|----------|-------|--------|------------------|
| POST /oauth/token | 10 | 1 minute | 429 Too Many Requests |
| POST /friendships/ | 10 | 1 hour | 429 Too Many Requests |
| POST /location-shares/ | 20 | 1 hour | 429 Too Many Requests |
| GET /latest_locations | 60 | 1 minute | 429 Too Many Requests |
| POST /check-ins/ | 30 | 1 hour | 429 Too Many Requests |
| All other endpoints | 100 | 1 minute | 429 Too Many Requests |

### Rate Limit Response

```json
{
  "type": "about://errors/rate-limit-exceeded",
  "title": "Rate Limit Exceeded",
  "status": 429,
  "detail": "Too many requests. Please retry after 60 seconds.",
  "instance": "/api/v1/latest_locations",
  "retry_after": 60
}
```

**Headers:**
```
HTTP/1.1 429 Too Many Requests
Retry-After: 60
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1699900860
```

## Input Validation

### Coordinate Validation

```python
def validate_coordinates(latitude, longitude):
    """Validate latitude and longitude values."""
    if not isinstance(latitude, (int, float)):
        raise BadRequestException("Latitude must be a number")
    if not isinstance(longitude, (int, float)):
        raise BadRequestException("Longitude must be a number")
    
    if not -90 <= latitude <= 90:
        raise BadRequestException("Latitude must be between -90 and 90")
    if not -180 <= longitude <= 180:
        raise BadRequestException("Longitude must be between -180 and 180")
    
    # Check for impossible locations (middle of ocean, etc.) if needed
    # Check for impossible movement speed if previous location exists
    
    return True
```

### Username Validation

```python
def validate_username(username):
    """Validate username format and availability."""
    if not isinstance(username, str):
        raise BadRequestException("Username must be a string")
    
    if len(username) < 3 or len(username) > 30:
        raise BadRequestException("Username must be 3-30 characters")
    
    if not re.match(r'^[a-zA-Z][a-zA-Z0-9_]*$', username):
        raise BadRequestException("Username must start with a letter and contain only letters, numbers, and underscores")
    
    # Check reserved usernames
    reserved = ['admin', 'moderator', 'support', 'always_together', 'system']
    if username.lower() in reserved:
        raise BadRequestException("This username is reserved")
    
    return True
```

## Error Handling

### Standard Error Response Format

All errors follow RFC 7807 (Problem Details for HTTP APIs):

```json
{
  "type": "about://errors/error-type",
  "title": "Human-readable title",
  "status": 400,
  "detail": "Specific explanation of what went wrong",
  "instance": "/api/v1/endpoint",
  "error_code": "SPECIFIC_ERROR_CODE",
  "timestamp": "2024-11-15T10:30:00Z",
  "request_id": "req_abc123def456"
}
```

### Error Types

| Error Type | HTTP Status | Description |
|------------|-------------|-------------|
| `about://errors/invalid-request` | 400 | Malformed request, validation failure |
| `about://errors/unauthorized` | 401 | Missing or invalid authentication |
| `about://errors/forbidden` | 403 | Authorization denied |
| `about://errors/not-found` | 404 | Resource does not exist |
| `about://errors/conflict` | 409 | Resource conflict (duplicate, etc.) |
| `about://errors/rate-limit-exceeded` | 429 | Too many requests |
| `about://errors/internal-error` | 500 | Internal server error |

### Security-Specific Error Messages

**DO:**
- "Authorization denied" (generic)
- "Invalid authentication token"
- "Resource not found"
- "Request validation failed"

**DON'T:**
- "User does not exist" (reveals existence)
- "You are not friends with this user" (reveals relationship)
- "Location share was revoked by owner" (reveals revocation vs never-existed)
- Stack traces or internal error details

## Audit Logging

### Events to Log

| Event | Data to Log | Retention |
|-------|-------------|-----------|
| Authentication success | user_id, device_id, ip_hash, timestamp | 90 days |
| Authentication failure | device_id, ip_hash, timestamp, reason | 90 days |
| Location share created | owner_id, recipient_id, precision, purpose, expiry | 2 years |
| Location share revoked | share_id, revoked_by, reason, timestamp | 2 years |
| Location accessed | viewer_id, friend_id, timestamp, share_id | 1 year |
| Profile viewed (non-friend) | viewer_id, target_id, timestamp | 90 days |
| Friendship requested | requester_id, target_id, timestamp | 1 year |
| Block created | blocker_id, blocked_id, timestamp | 2 years |
| Check-in created | user_id, type, scheduled_time, timestamp | 1 year |
| Check-in missed | checkin_id, user_id, timestamp | 1 year |
| Report submitted | reporter_id, reported_id, category, timestamp | 2 years |
| Moderation action | moderator_id, target_id, action, reason | 3 years |

### Audit Log Entry Format

```json
{
  "event_id": "evt_1a2b3c4d5e6f",
  "event_type": "location_share_created",
  "timestamp": "2024-11-15T10:30:00Z",
  "actor_id": "usr_7f8d9e2a1b3c4d5e",
  "target_id": "usr_abc123def456",
  "resource_type": "location_share",
  "resource_id": "loc_share_9z8y7x6w",
  "action": "create",
  "context": {
    "precision": "approximate",
    "purpose": "safety",
    "expires_at": "2024-11-15T22:00:00Z"
  },
  "metadata": {
    "ip_hash": "sha256:abc123...",
    "user_agent_hash": "sha256:def456...",
    "device_id": "dev_a1b2c3d4"
  }
}
```

---

*Related documents:*
- *[Security Architecture and Data Protection](../architecture/security-architecture-and-data-protection.md)*
- *[Security Control Register](./security-control-register.md)*
- *[Security Threats and Controls](../architecture/security-threats-and-controls.md)*
- *[Domain Data Model](./domain-data-model.md)*

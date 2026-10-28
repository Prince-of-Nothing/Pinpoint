"""
Pydantic schemas for request/response validation.
Defines data transfer objects for API endpoints.

Validation is the first security control (SC-04): every field is bounded and
typed before business logic runs; error responses never echo internals.
"""

from datetime import datetime
from enum import Enum
from typing import List, Optional

from pydantic import BaseModel, ConfigDict, EmailStr, Field, field_validator


class UserRole(str, Enum):
    """User role enumeration for schemas."""
    RESIDENT = "resident"
    ADMINISTRATOR = "administrator"


class FriendshipStatus(str, Enum):
    """Friendship status enumeration."""
    PENDING = "pending"
    ACCEPTED = "accepted"
    REJECTED = "rejected"
    BLOCKED = "blocked"


VALID_PURPOSES = ("safety", "meetup", "navigation", "check_in")
PRECISION_LEVELS = ("exact", "approximate", "city_only")


# ============ Authentication Schemas ============

class LoginRequest(BaseModel):
    """Login request schema."""
    email: EmailStr = Field(..., max_length=255, description="User email address")
    # Length policy is enforced at password *creation*; login must accept any
    # previously stored credential (including temporary passwords).
    password: str = Field(..., min_length=1, max_length=128, description="User password")

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "email": "user@example.com",
                "password": "SecurePassword123!"
            }
        }
    )


class LoginResponse(BaseModel):
    """Login response schema."""
    access_token: str
    token_type: str = "bearer"
    expires_in: int
    user: "UserResponse"


class PasswordChangeRequest(BaseModel):
    """Change user password schema."""
    current_password: str = Field(..., min_length=1, max_length=128)
    new_password: str = Field(..., min_length=10, max_length=128)
    confirm_password: str = Field(..., min_length=10, max_length=128)

    @field_validator('confirm_password')
    @classmethod
    def passwords_match(cls, v, info):
        if 'new_password' in info.data and v != info.data['new_password']:
            raise ValueError('Passwords do not match')
        return v


class ForcePasswordChangeRequest(BaseModel):
    """Force password change request schema (first login)."""
    new_password: str = Field(..., min_length=10, max_length=128)
    confirm_password: str = Field(..., min_length=10, max_length=128)

    @field_validator('confirm_password')
    @classmethod
    def passwords_match(cls, v, info):
        if 'new_password' in info.data and v != info.data['new_password']:
            raise ValueError('Passwords do not match')
        return v


# ============ User Schemas ============

class UserCreate(BaseModel):
    """Schema for creating a new user (admin only)."""
    email: EmailStr = Field(..., max_length=255)
    full_name: str = Field(..., min_length=1, max_length=100)
    role: UserRole = Field(default=UserRole.RESIDENT)
    temporary_password: Optional[str] = Field(None, min_length=10, max_length=128)

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "email": "newuser@example.com",
                "full_name": "John Doe",
                "role": "resident"
            }
        }
    )


class UserResponse(BaseModel):
    """User response schema (excludes sensitive data)."""
    id: int
    email: str
    full_name: str
    role: UserRole
    is_active: bool
    must_change_password: bool
    created_at: datetime
    last_login_at: Optional[datetime] = None

    model_config = ConfigDict(from_attributes=True)


class UserUpdate(BaseModel):
    """Schema for updating user profile."""
    full_name: Optional[str] = Field(None, min_length=1, max_length=100)


class ProfileResponse(BaseModel):
    """
    Viewer-filtered profile projection (SC-03).

    Fields are null/redacted for viewers without a friendship — the backend
    filters, the client only renders.
    """
    id: Optional[int] = None
    display_name: Optional[str] = None
    is_full_profile: bool = False


# ============ Friendship Schemas ============

class FriendshipRequest(BaseModel):
    """Schema for sending a friend request."""
    receiver_email: EmailStr = Field(..., max_length=255)

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "receiver_email": "friend@example.com"
            }
        }
    )


class FriendshipResponse(BaseModel):
    """Friendship response schema."""
    id: int
    requester_id: int
    requester_name: str
    receiver_id: int
    receiver_name: str
    status: FriendshipStatus
    created_at: datetime
    responded_at: Optional[datetime] = None

    model_config = ConfigDict(from_attributes=True)


class FriendshipAction(BaseModel):
    """Schema for accepting/rejecting friend requests."""
    action: str = Field(..., pattern="^(accept|reject)$")


# ============ Location Share Schemas ============

class LocationShareCreate(BaseModel):
    """Schema for creating a directional location-share consent record."""
    friendship_id: int
    direction: str = Field("to_friend", pattern="^(to_friend|from_friend|mutual)$")
    precision_level: Optional[str] = Field("exact", pattern="^(exact|approximate|city_only)$")
    purpose: str = Field("safety", pattern="^(safety|meetup|navigation|check_in)$")
    expires_at: Optional[datetime] = None

    model_config = ConfigDict(
        json_schema_extra={
            "example": {
                "friendship_id": 1,
                "direction": "to_friend",
                "precision_level": "approximate",
                "purpose": "safety",
                "expires_at": "2026-01-01T22:00:00Z"
            }
        }
    )


class LocationShareResponse(BaseModel):
    """Location share response schema."""
    id: int
    friendship_id: int
    owner_id: int
    direction: str
    is_active: bool
    precision_level: str
    purpose: str
    expires_at: Optional[datetime] = None
    created_at: datetime
    revoked_at: Optional[datetime] = None
    last_location_update_at: Optional[datetime] = None

    model_config = ConfigDict(from_attributes=True)


class LocationUpdateRequest(BaseModel):
    """Schema for updating the caller's current location."""
    latitude: float = Field(..., ge=-90, le=90)
    longitude: float = Field(..., ge=-180, le=180)
    accuracy: Optional[float] = Field(None, ge=0, le=100_000)
    battery_level: Optional[int] = Field(None, ge=0, le=100)
    timestamp: Optional[datetime] = None


class FriendLocationResponse(BaseModel):
    """One friend's authorized, precision-filtered latest location."""
    friend_id: int
    friend_name: str
    latitude: float
    longitude: float
    accuracy: Optional[float] = None
    precision_level: str
    recorded_at: datetime
    is_stale: bool
    battery_level: Optional[int] = None


class LatestLocationsResponse(BaseModel):
    """Collection of authorized friend locations."""
    friends_locations: List[FriendLocationResponse]


# Back-compatible alias (older code referenced LocationUpdate).
LocationUpdate = LocationUpdateRequest


# ============ Error Response Schema ============

class ErrorResponse(BaseModel):
    """Standard error response schema (RFC 7807 Problem Details)."""
    error: dict


class ErrorDetail(BaseModel):
    """Error detail structure."""
    code: str
    message: str
    fields: Optional[dict] = None


# Update forward references
UserResponse.model_rebuild()
FriendshipResponse.model_rebuild()
LoginResponse.model_rebuild()

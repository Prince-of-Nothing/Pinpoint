"""Schemas module initialization."""

from app.schemas.auth import (
    # Auth schemas
    LoginRequest,
    LoginResponse,
    PasswordChangeRequest,
    ForcePasswordChangeRequest,

    # User schemas
    UserCreate,
    UserResponse,
    UserUpdate,
    ProfileResponse,

    # Friendship schemas
    FriendshipRequest,
    FriendshipResponse,
    FriendshipAction,

    # Location schemas
    LocationShareCreate,
    LocationShareResponse,
    LocationUpdate,
    LocationUpdateRequest,
    FriendLocationResponse,
    LatestLocationsResponse,

    # Error schemas
    ErrorResponse,
    ErrorDetail,

    # Enums
    UserRole,
    FriendshipStatus
)

__all__ = [
    "LoginRequest",
    "LoginResponse",
    "PasswordChangeRequest",
    "ForcePasswordChangeRequest",
    "UserCreate",
    "UserResponse",
    "UserUpdate",
    "ProfileResponse",
    "FriendshipRequest",
    "FriendshipResponse",
    "FriendshipAction",
    "LocationShareCreate",
    "LocationShareResponse",
    "LocationUpdate",
    "LocationUpdateRequest",
    "FriendLocationResponse",
    "LatestLocationsResponse",
    "ErrorResponse",
    "ErrorDetail",
    "UserRole",
    "FriendshipStatus"
]

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
    
    # Friendship schemas
    FriendshipRequest,
    FriendshipResponse,
    FriendshipAction,
    
    # Location schemas
    LocationShareCreate,
    LocationShareResponse,
    LocationUpdate,
    LocationResponse,
    
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
    "FriendshipRequest",
    "FriendshipResponse",
    "FriendshipAction",
    "LocationShareCreate",
    "LocationShareResponse",
    "LocationUpdate",
    "LocationResponse",
    "ErrorResponse",
    "ErrorDetail",
    "UserRole",
    "FriendshipStatus"
]

"""Authentication and account schemas."""

import re
from datetime import datetime
from typing import Literal

from pydantic import EmailStr, Field, field_validator

from app.schemas.common import ApiResponse, StrictRequest

USERNAME_PATTERN = re.compile(r"^[a-z0-9_]{3,30}$")
RESERVED_USERNAMES = frozenset({"admin", "administrator", "root", "system", "support", "me", "api"})


def normalise_username(value: str) -> str:
    value = value.strip().lower()
    if not USERNAME_PATTERN.fullmatch(value):
        raise ValueError("Use 3-30 characters: lowercase letters, digits and underscores.")
    return value


class RegisterRequest(StrictRequest):
    username: str = Field(min_length=3, max_length=30)
    email: EmailStr = Field(max_length=254)
    # Length bounds are enforced by the password policy (clear messages, no echo)
    password: str = Field(min_length=1, max_length=1024)
    display_name: str | None = Field(default=None, min_length=1, max_length=50)

    @field_validator("username")
    @classmethod
    def _username(cls, value: str) -> str:
        value = normalise_username(value)
        if value in RESERVED_USERNAMES:
            raise ValueError("This username is reserved.")
        return value

    @field_validator("email")
    @classmethod
    def _email(cls, value: str) -> str:
        return value.strip().lower()


class LoginRequest(StrictRequest):
    login: str = Field(min_length=1, max_length=254, description="Username or email")
    password: str = Field(min_length=1, max_length=1024)
    token_delivery: Literal["cookie", "body"] = Field(
        default="cookie",
        description="'cookie' (web: HttpOnly refresh cookie + CSRF token) or "
        "'body' (native apps: refresh token returned in the response body)",
    )


class RefreshRequest(StrictRequest):
    refresh_token: str | None = Field(default=None, min_length=16, max_length=128)


class PasswordChangeRequest(StrictRequest):
    current_password: str = Field(min_length=1, max_length=1024)
    new_password: str = Field(min_length=1, max_length=1024)


class UserSelf(ApiResponse):
    id: str
    username: str
    email: str
    display_name: str | None
    bio: str | None
    role: str
    created_at: datetime
    last_login_at: datetime | None


class TokenResponse(ApiResponse):
    access_token: str
    token_type: Literal["bearer"] = "bearer"  # noqa: S105
    expires_in: int
    refresh_token: str | None = None
    csrf_token: str | None = None
    user: UserSelf


class SessionInfo(ApiResponse):
    id: str
    created_at: datetime
    last_seen_at: datetime
    expires_at: datetime
    ip_address: str | None
    user_agent: str | None
    current: bool = False
